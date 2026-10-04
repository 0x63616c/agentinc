//! Build the macOS-only GPUI application, then stage portable signing input.
//! No credentials needed. The Linux release job signs every nested Mach-O.
use super::{checked, create_tar_gz, hex, inventory, output, parse_args, usage_error};
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};

/// The executable name the bundle shipped before the package was renamed. The already-installed
/// 0.1.0 updater launches this path after replacement, so the bundle keeps it as a symlink.
pub const LEGACY_EXECUTABLE: &str = "agentinc-os";

const PG_VERSION: &str = "16.15.0";
const PG_SHA256: &str = "46f6382024d9b633d1f4b4903ffef8c2404e00ae83098d628c00591048cb0512";

/// `shutil.which`: the first executable regular file named `binary` on PATH.
fn which(binary: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(binary))
        .find(|path| {
            fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// `fnmatch` for the three patterns `copytree` ignores: a literal or a `*suffix`.
fn ignored(name: &str) -> bool {
    name == "pgxs" || name.ends_with(".a") || name.ends_with(".pc")
}

/// `shutil.copytree(symlinks=True)`, optionally ignoring the lib patterns above.
pub(super) fn copytree(source: &Path, destination: &Path, ignore: bool) -> Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if ignore && ignored(&name.to_string_lossy()) {
            continue;
        }
        let target = destination.join(&name);
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            symlink(fs::read_link(entry.path())?, &target)?;
        } else if kind.is_dir() {
            copytree(&entry.path(), &target, ignore)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    fs::set_permissions(destination, fs::metadata(source)?.permissions())?;
    Ok(())
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `plistlib.dumps` for a flat dict of strings and booleans: XML, keys sorted.
fn plist(entries: &[(&str, Value)]) -> String {
    let mut sorted: Vec<_> = entries.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
    );
    for (key, value) in sorted {
        out.push_str(&format!("\t<key>{}</key>\n", escape(key)));
        match value {
            Value::Bool(true) => out.push_str("\t<true/>\n"),
            Value::Bool(false) => out.push_str("\t<false/>\n"),
            Value::String(s) => out.push_str(&format!("\t<string>{}</string>\n", escape(&s))),
            other => unreachable!("unsupported plist value {other}"),
        }
    }
    out.push_str("</dict>\n</plist>\n");
    out
}

/// Every Mach-O in the bundle may link only system, `@loader_path` or `@rpath` libraries, and
/// every `@loader_path` library must exist inside the bundle.
fn audit(bundle: &Path) -> Result<()> {
    fn files(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let linked = fs::symlink_metadata(&path)?.file_type().is_symlink();
            if metadata.is_dir() {
                if !linked {
                    files(&path, found)?;
                }
            } else if metadata.is_file() {
                found.push(path);
            }
        }
        Ok(())
    }
    let canonical = fs::canonicalize(bundle)?;
    let mut all = Vec::new();
    files(bundle, &mut all)?;
    for binary in all {
        if !output("file", &["-b", &text(&binary)])?.starts_with("Mach-O") {
            continue;
        }
        let listing = output("otool", &["-L", &text(&binary)])?;
        // Universal Sparkle binaries include a header for each architecture.
        for line in listing.lines().filter(|line| line.starts_with('\t')) {
            let line = line.trim();
            let dependency = line.split(" (compatibility").next().unwrap_or(line);
            if !["/usr/lib/", "/System/", "@loader_path/", "@rpath/"]
                .iter()
                .any(|prefix| dependency.starts_with(prefix))
            {
                bail!(
                    "nonportable dependency {dependency} in {}",
                    binary.display()
                );
            }
            if let Some(rest) = dependency.strip_prefix("@loader_path/") {
                let wanted = binary.parent().unwrap().join(rest);
                let resolved = fs::canonicalize(&wanted).ok();
                if !resolved
                    .as_ref()
                    .is_some_and(|path| path.starts_with(&canonical) && path.is_file())
                {
                    bail!(
                        "unresolved bundle dependency {dependency} in {}",
                        binary.display()
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn cli(root: &Path, raw: &[String]) -> Result<()> {
    let opts = parse_args("release", raw, &["--upload"], &["--profile"]);
    if !opts.positional.is_empty() {
        usage_error(
            "release",
            &format!("unrecognized arguments: {}", opts.positional.join(" ")),
        );
    }
    let profile = opts.one("--profile").unwrap_or("release");
    if !["debug", "release"].contains(&profile) {
        usage_error(
            "release",
            &format!(
                "argument --profile: invalid choice: '{profile}' (choose from 'debug', 'release')"
            ),
        );
    }
    run(root, profile, opts.flag("--upload"))
}

fn run(root: &Path, profile: &str, upload: bool) -> Result<()> {
    if env::consts::OS != "macos" {
        bail!("GPUI/Metal requires the macOS SDK: run cargo xtask release on a Mac.");
    }
    let root = fs::canonicalize(root)?;
    env::set_current_dir(&root)?;
    if !output("git", &["status", "--porcelain", "--untracked-files=no"])?.is_empty() {
        bail!("Commit tracked changes before preparing a release handoff");
    }
    let metadata: Value = serde_json::from_str(&output(
        "cargo",
        &["metadata", "--no-deps", "--format-version=1"],
    )?)?;
    let product_version = metadata["packages"]
        .as_array()
        .and_then(|all| all.iter().find(|p| p["name"] == "ainc-release"))
        .and_then(|p| p["version"].as_str())
        .ok_or_else(|| anyhow!("ainc-release is not in cargo metadata"))?
        .to_string();
    let test_key = env::var("AINC_UPGRADE_TEST_PUBLIC_KEY").ok();
    let test_key_set = test_key.as_deref().is_some_and(|key| !key.is_empty());
    let test_version = env::var("AINC_UPGRADE_TEST_VERSION").ok();
    if test_version.as_deref().is_some_and(|v| !v.is_empty()) && !test_key_set {
        bail!("test version requires an upgrade-test public key");
    }
    let newer_fixture = test_version.as_deref().is_some_and(|v| !v.is_empty());
    let version = test_version.unwrap_or(product_version);
    let commit = output("git", &["rev-parse", "HEAD"])?;
    let build = output("git", &["rev-list", "--count", "HEAD"])?;
    // Sparkle compares CFBundleVersion, not the display version. Both fixtures
    // come from one commit, so give the explicitly newer fixture the next build.
    let build = if newer_fixture {
        (build.parse::<u64>()? + 1).to_string()
    } else {
        build
    };
    let mut command = crate::spawn::cargo();
    command
        .args([
            "build",
            "--locked",
            "-p",
            "ainc-mac",
            "-p",
            "ainc-daemon",
            "-p",
            "ainc-release",
            "-p",
            "ainc-cli",
            "--bins",
        ])
        .env("AINC_BUILD_ID", &build)
        .env("AINC_CHANNEL", "production")
        .env("AINC_COMMIT", &commit)
        .env("CARGO_INCREMENTAL", "0");
    if profile == "release" {
        command.arg("--release");
    }
    checked(&mut command)?;
    let out = root.join(".local/release").join(&commit);
    fs::create_dir_all(&out)?;
    let bundle = out.join("AgentInc.app");
    if bundle.exists() {
        fs::remove_dir_all(&bundle)?;
    }
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir(&resources)?;
    let target = Path::new(
        metadata["target_directory"]
            .as_str()
            .context("target_directory")?,
    )
    .join(profile);
    for binary in ["AgentInc", "aincd", "ainc-update", "ainc"] {
        fs::copy(target.join(binary), macos.join(binary))?;
    }
    symlink("AgentInc", macos.join(LEGACY_EXECUTABLE))?;
    fs::copy(
        root.join("crates/ainc-mac/assets/AppIcon.icns"),
        resources.join("AppIcon.icns"),
    )?;
    checked(
        Command::new("sh")
            .args(["crates/ainc-mac/scripts/stage-ghostty.sh", profile])
            .arg(&bundle),
    )?;
    super::sparkle::stage(&root, &bundle)?;
    let runtime = resources.join("runtime");
    fs::create_dir(&runtime)?;
    // Portable by construction; never relocate a developer's Homebrew install.
    if env::consts::ARCH != "aarch64" {
        bail!("This release currently supports Apple Silicon only");
    }
    let pg_name = format!("postgresql-{PG_VERSION}-aarch64-apple-darwin");
    let cache = root.join(".local/release-inputs");
    fs::create_dir_all(&cache)?;
    let pg_archive = cache.join(format!("{pg_name}.tar.gz"));
    if !pg_archive.exists() {
        checked(Command::new("curl").args(["--fail", "--location", "--retry", "3", "--retry-all-errors", "--output"]).arg(&pg_archive).arg(format!(
            "https://github.com/theseus-rs/postgresql-binaries/releases/download/{PG_VERSION}/{pg_name}.tar.gz"
        )))?;
    }
    if hex(&Sha256::digest(fs::read(&pg_archive)?)) != PG_SHA256 {
        bail!("portable Postgres checksum mismatch");
    }
    {
        let unpack = tempfile::Builder::new().tempdir_in(&cache)?;
        // The archive is pinned by SHA-256 above.
        checked(
            Command::new("tar")
                .arg("-xzf")
                .arg(&pg_archive)
                .arg("-C")
                .arg(unpack.path()),
        )?;
        let pg = unpack.path().join(&pg_name);
        let destination = runtime.join("postgres");
        fs::create_dir_all(destination.join("bin"))?;
        for binary in ["postgres", "initdb"] {
            fs::copy(
                pg.join("bin").join(binary),
                destination.join("bin").join(binary),
            )?;
        }
        copytree(&pg.join("share"), &destination.join("share"), false)?;
        copytree(&pg.join("lib"), &destination.join("lib"), true)?;
        for license in ["LICENSE", "COPYRIGHT", "README.md"] {
            fs::copy(pg.join(license), destination.join(license))?;
        }
    }
    for binary in ["temporal", "codex"] {
        let source = which(binary)
            .ok_or_else(|| anyhow!("{binary} binary required for the self-contained bundle"))?;
        fs::copy(fs::canonicalize(source)?, runtime.join(binary))?;
    }
    // Audit every Mach-O, including dylibs. A packaging regression fails before
    // the artifact reaches signing; no machine-local paths are tolerated.
    audit(&bundle)?;
    let mut identity: Map<String, Value> = serde_json::from_str(&output(
        &text(&target.join("ainc-release-manifest")),
        &["--identity"],
    )?)?;
    identity.insert("commit".into(), json!(commit));
    identity.insert(
        "architecture".into(),
        json!(env::consts::ARCH.replace("arm64", "aarch64")),
    );
    identity.insert("upgrade_test".into(), json!(test_key_set));
    if identity.get("version") != Some(&json!(version))
        || identity.get("build") != Some(&json!(build))
    {
        bail!("compiled product identity differs from Cargo metadata/build input");
    }
    fs::write(
        resources.join("release.json"),
        format!("{}\n", serde_json::to_string_pretty(&identity)?),
    )?;
    let mut info = vec![
        ("CFBundleName", json!("AgentInc")),
        ("CFBundleDisplayName", json!("AgentInc")),
        ("CFBundleIdentifier", json!("co.worldwidewebb.agentinc")),
        ("CFBundleExecutable", json!("AgentInc")),
        ("CFBundleIconFile", json!("AppIcon")),
        ("CFBundlePackageType", json!("APPL")),
        ("CFBundleShortVersionString", json!(version)),
        ("CFBundleVersion", json!(build)),
        (
            "NSHumanReadableCopyright",
            json!("Copyright © 2026 Calum Webb"),
        ),
        ("LSMinimumSystemVersion", json!("15.0")),
        ("NSHighResolutionCapable", json!(true)),
        ("NSPrincipalClass", json!("NSApplication")),
        ("SUFeedURL", json!(super::sparkle::FEED_URL)),
        (
            "SUPublicEDKey",
            json!(
                test_key
                    .as_deref()
                    .filter(|key| !key.is_empty())
                    .unwrap_or(ainc_release::UPDATE_PUBLIC_KEY)
            ),
        ),
        ("SUVerifyUpdateBeforeExtraction", json!(true)),
        ("SURequireSignedFeed", json!(true)),
        ("SUEnableAutomaticChecks", json!(false)),
        ("SUAutomaticallyUpdate", json!(false)),
    ];
    if test_key_set {
        // Both fixture versions share a per-run defaults/cache namespace.
        // Stamp it before signing; acceptance never edits a notarized bundle.
        info.push((
            "SUDefaultsDomain",
            json!(format!(
                "co.worldwidewebb.agentinc.upgrade-test.{}",
                hex(&Sha256::digest(test_key.as_deref().unwrap().as_bytes()))
            )),
        ));
    }
    fs::write(contents.join("Info.plist"), plist(&info))?;
    // The runtime input inventory makes local build provenance reviewable.
    let mut handoff = identity.clone();
    handoff.insert("files".into(), Value::Object(inventory(&bundle)?));
    fs::write(
        out.join("handoff.json"),
        format!("{}\n", serde_json::to_string_pretty(&handoff)?),
    )?;
    let archive = out.join("unsigned.tar.gz");
    // This is a short-lived handoff, not the downloadable release asset. Avoid
    // spending native-runner time on maximum compression for every test rebuild.
    create_tar_gz(
        &archive,
        1,
        &[
            (&bundle, "AgentInc.app"),
            (&out.join("handoff.json"), "handoff.json"),
        ],
    )?;
    println!("{}", archive.display());
    if upload {
        let tag = format!("build-{commit}");
        let existing = Command::new("gh-axi")
            .args(["release", "view", &tag])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !existing.success() {
            checked(Command::new("gh-axi").args([
                "release",
                "create",
                &tag,
                "--draft",
                "--target",
                &commit,
                "--title",
                &format!("AgentInc {version} build input"),
                "--notes",
                "Native signing input. Do not publish.",
            ]))?;
        }
        checked(
            Command::new("gh-axi")
                .args(["release", "upload", &tag])
                .arg(&archive)
                .arg("--clobber"),
        )?;
        let testing = output("git", &["branch", "--show-current"])? != "main";
        checked(Command::new("gh-axi").args([
            "workflow",
            "run",
            "release.yml",
            "--ref",
            "main",
            "-f",
            &format!("commit={commit}"),
            "-f",
            &format!("test={testing}"),
            "-f",
            "build=false",
        ]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_is_sorted_xml_like_plistlib() {
        let out = plist(&[("b", json!("x & y")), ("a", json!(true))]);
        assert!(out.contains("<dict>\n\t<key>a</key>\n\t<true/>\n\t<key>b</key>\n\t<string>x &amp; y</string>\n</dict>"));
    }

    #[test]
    fn copytree_keeps_symlinks_and_skips_ignored_names() {
        let root = tempfile::tempdir().unwrap();
        let (src, dst) = (root.path().join("src"), root.path().join("dst"));
        fs::create_dir_all(src.join("pgxs")).unwrap();
        fs::write(src.join("libpq.dylib"), b"x").unwrap();
        fs::write(src.join("libpq.a"), b"x").unwrap();
        fs::write(src.join("libpq.pc"), b"x").unwrap();
        symlink("libpq.dylib", src.join("libpq.5.dylib")).unwrap();
        copytree(&src, &dst, true).unwrap();
        assert!(dst.join("libpq.dylib").is_file());
        assert!(!dst.join("libpq.a").exists() && !dst.join("pgxs").exists());
        assert_eq!(
            fs::read_link(dst.join("libpq.5.dylib")).unwrap(),
            Path::new("libpq.dylib")
        );
    }
}
