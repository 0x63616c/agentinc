//! Sparkle's pinned native tools generate deltas on macOS. Only the Linux signer
//! receives production keys; Ed25519 signatures use Sparkle's documented wire format.
use super::{checked, extract_tar_gz, hex, inventory, output, parse_args};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

pub const VERSION: &str = "2.9.6";
const SHA256: &str = "52bf9e88cdd972fc0c81501377a880e90d47031bd8ca5462488f843e2609e192";
pub const FEED_URL: &str =
    "https://github.com/0x63616c/agentinc/releases/latest/download/appcast.xml";

/// Return a freshly extracted, checksum-verified official distribution. Never
/// trust an already extracted tool cache (the tarball is the trust boundary).
pub fn tools(root: &Path) -> Result<PathBuf> {
    let cache = root.join(".local/release-inputs");
    fs::create_dir_all(&cache)?;
    let archive = cache.join(format!("Sparkle-{VERSION}.tar.xz"));
    if !archive.exists() {
        let download = tempfile::NamedTempFile::new_in(&cache)?;
        checked(Command::new("curl").args(["--fail", "--location", "--output"]).arg(download.path()).arg(format!(
            "https://github.com/sparkle-project/Sparkle/releases/download/{VERSION}/Sparkle-{VERSION}.tar.xz"
        )))?;
        ensure!(
            hex(&Sha256::digest(fs::read(download.path())?)) == SHA256,
            "Sparkle checksum mismatch"
        );
        download.persist(&archive)?;
    }
    ensure!(
        hex(&Sha256::digest(fs::read(&archive)?)) == SHA256,
        "Sparkle checksum mismatch"
    );
    let destination = cache.join(format!("sparkle-{VERSION}"));
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::create_dir(&destination)?;
    checked(
        Command::new("tar")
            .arg("-xJf")
            .arg(archive)
            .arg("-C")
            .arg(&destination),
    )?;
    Ok(destination)
}

pub fn stage(root: &Path, bundle: &Path) -> Result<()> {
    let distribution = tools(root)?;
    let frameworks = bundle.join("Contents/Frameworks");
    fs::create_dir_all(&frameworks)?;
    // Preserve the entire framework, including its XPC services, Updater.app,
    // Autoupdate, signatures, executable modes and versioned symlinks.
    super::prepare::copytree(
        &distribution.join("Sparkle.framework"),
        &frameworks.join("Sparkle.framework"),
        false,
    )?;
    fs::copy(
        distribution.join("LICENSE"),
        bundle.join("Contents/Resources/Sparkle-LICENSE"),
    )?;
    Ok(())
}

/// Same raw Ed25519 signature as Sparkle sign_update. PEM is never passed in
/// arguments or printed, and temporary key material lives in a private directory.
pub fn sign_bytes(bytes: &[u8], pem: &str) -> Result<String> {
    let private = tempfile::tempdir()?;
    fs::set_permissions(private.path(), fs::Permissions::from_mode(0o700))?;
    let key = private.path().join("key.pem");
    let data = private.path().join("data");
    fs::write(&key, pem)?;
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600))?;
    fs::write(&data, bytes)?;
    let result = Command::new("openssl")
        .args(["pkeyutl", "-sign", "-rawin", "-inkey"])
        .arg(key)
        .arg("-in")
        .arg(data)
        .env_remove("UPDATE_SIGNING_KEY_ED25519_PEM")
        .output()?;
    ensure!(
        result.status.success() && result.stdout.len() == 64,
        "Sparkle Ed25519 signing failed"
    );
    Ok(STANDARD.encode(result.stdout))
}

/// Exact format from Sparkle 2.9.6 common_cli/Signing.swift signAppcast().
/// The footer is outside the signed bytes; embedded notes are inside them.
pub fn sign_feed(xml: &str, pem: &str) -> Result<Vec<u8>> {
    ensure!(
        !xml.contains("<!-- sparkle-signatures:"),
        "feed is already signed"
    );
    let signature = sign_bytes(xml.as_bytes(), pem)?;
    Ok(format!(
        "{xml}<!-- sparkle-signatures:\nedSignature: {signature}\nlength: {}\n-->\n",
        xml.len()
    )
    .into_bytes())
}

/// Generate and actually apply an official v4 delta before accepting it.
pub fn create_delta(root: &Path, before: &Path, after: &Path, delta: &Path) -> Result<()> {
    let distribution = tools(root)?;
    create_delta_with(&distribution.join("bin/BinaryDelta"), before, after, delta)
}

fn create_delta_with(tool: &Path, before: &Path, after: &Path, delta: &Path) -> Result<()> {
    checked(
        Command::new(tool)
            .args(["create", "--version", "4"])
            .arg(before)
            .arg(after)
            .arg(delta),
    )?;
    let check = tempfile::tempdir()?;
    let reconstructed = check.path().join("AgentInc.app");
    checked(
        Command::new(tool)
            .arg("apply")
            .arg(before)
            .arg(&reconstructed)
            .arg(delta),
    )?;
    ensure!(
        inventory(&reconstructed)? == inventory(after)?,
        "Sparkle delta round-trip differs from signed bundle"
    );
    // Native codesign verifies symlinks, nested helpers and executable modes as
    // well as bytes. Never re-sign reconstructed bundles to hide a bad patch.
    verify_bundle(&reconstructed)
}

fn verify_bundle(bundle: &Path) -> Result<()> {
    checked(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle),
    )
}

fn verify_release_bundle(bundle: &Path, manifest: &ainc_release::Manifest) -> Result<()> {
    verify_bundle(bundle)?;
    let plist = bundle.join("Contents/Info.plist");
    for (key, expected) in [
        ("CFBundleVersion", manifest.build.as_str()),
        ("CFBundleShortVersionString", &manifest.version.to_string()),
        ("SUPublicEDKey", ainc_release::UPDATE_PUBLIC_KEY),
        ("SUFeedURL", FEED_URL),
        ("SURequireSignedFeed", "true"),
        ("SUVerifyUpdateBeforeExtraction", "true"),
    ] {
        let value = output(
            "plutil",
            &["-extract", key, "raw", "-o", "-", &plist.to_string_lossy()],
        )?;
        ensure!(
            value == expected,
            "release bundle {key} differs from Sparkle publication contract"
        );
    }
    ensure!(
        bundle
            .join("Contents/Frameworks/Sparkle.framework/Versions/B/Sparkle")
            .is_file(),
        "release is missing Sparkle.framework"
    );
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Delta {
    from_build: String,
    name: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeltaHandoff {
    tag: String,
    archive_sha256: String,
    deltas: Vec<Delta>,
}

fn download(tag: &str, asset: &str, destination: &Path) -> Result<()> {
    checked(
        Command::new("gh")
            .args(["release", "download", tag, "--pattern", asset, "--dir"])
            .arg(destination)
            .arg("--clobber"),
    )
}

fn manifest(directory: &Path, test: bool) -> Result<ainc_release::Manifest> {
    let public = if test {
        fs::read_to_string(directory.join("test-public-key.txt"))?
    } else {
        ainc_release::UPDATE_PUBLIC_KEY.to_string()
    };
    let signed: ainc_release::SignedManifest =
        serde_json::from_slice(&fs::read(directory.join("feed.json"))?)?;
    let manifest = signed.verify(public.trim())?;
    manifest.verify_archive(&fs::read(directory.join("AgentInc.tar.gz"))?)?;
    Ok(manifest)
}

fn fetch_release(tag: &str, directory: &Path, test: bool) -> Result<ainc_release::Manifest> {
    for asset in ["AgentInc.tar.gz", "feed.json"] {
        download(tag, asset, directory)?;
    }
    if test {
        download(tag, "test-public-key.txt", directory)?;
    }
    manifest(directory, test)
}

/// Mac runner, with read-only GitHub credentials and no production signing key.
/// Published release archives remain the durable delta history on GitHub.
pub fn deltas_cli(root: &Path, raw: &[String]) -> Result<()> {
    ensure!(env::consts::OS == "macos", "Sparkle deltas require macOS");
    let opts = parse_args(
        "release-sparkle-deltas",
        raw,
        &["--test"],
        &["--tag", "--out"],
    );
    let tag = opts.required("--tag");
    let out = Path::new(opts.required("--out"));
    fs::create_dir_all(out)?;
    let current = tempfile::tempdir()?;
    let manifest = fetch_release(tag, current.path(), opts.flag("--test"))?;
    extract_tar_gz(&current.path().join("AgentInc.tar.gz"), current.path())?;
    let app = current.path().join("AgentInc.app");
    verify_release_bundle(&app, &manifest)?;
    checked(
        Command::new("xcrun")
            .args(["stapler", "validate"])
            .arg(&app),
    )?;
    let releases: Vec<Value> = serde_json::from_str(&output(
        "gh",
        &[
            "release",
            "list",
            "--limit",
            "100",
            "--json",
            "tagName,isDraft,isPrerelease",
        ],
    )?)?;
    let mut deltas = Vec::new();
    // Three recent production archives give an actual delta path without ever
    // rebuilding a historical source tree (which changes its signed bytes).
    for release in releases
        .iter()
        .filter(|release| release["isDraft"] == false && release["isPrerelease"] == false)
        .take(3)
    {
        let prior_tag = release["tagName"].as_str().context("release tag")?;
        if prior_tag == tag {
            continue;
        }
        let prior = tempfile::tempdir()?;
        let prior_manifest = fetch_release(prior_tag, prior.path(), false)?;
        if prior_manifest.build.parse::<u64>()? >= manifest.build.parse::<u64>()? {
            continue;
        }
        extract_tar_gz(&prior.path().join("AgentInc.tar.gz"), prior.path())?;
        let prior_app = prior.path().join("AgentInc.app");
        if !prior_app
            .join("Contents/Frameworks/Sparkle.framework")
            .exists()
        {
            continue; // Legacy clients use the unchanged full-archive feed.
        }
        verify_bundle(&prior_app)?;
        let name = format!(
            "AgentInc-{}-from-{}.delta",
            manifest.build, prior_manifest.build
        );
        let path = out.join(&name);
        create_delta(root, &prior_app, &app, &path)?;
        if fs::metadata(&path)?.len() >= manifest.archive_bytes {
            fs::remove_file(path)?;
            continue;
        }
        deltas.push(Delta {
            from_build: prior_manifest.build,
            name,
            sha256: hex(&Sha256::digest(fs::read(path)?)),
        });
    }
    fs::write(
        out.join("sparkle-deltas.json"),
        serde_json::to_vec_pretty(&DeltaHandoff {
            tag: tag.into(),
            archive_sha256: manifest.archive_sha256,
            deltas,
        })?,
    )?;
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn enclosure(url: &str, bytes: &[u8], pem: &str, from: Option<&str>) -> Result<String> {
    let from = from
        .map(|build| format!(" sparkle:deltaFrom=\"{}\"", escape(build)))
        .unwrap_or_default();
    Ok(format!(
        "<enclosure url=\"{}\" length=\"{}\" type=\"application/octet-stream\" sparkle:edSignature=\"{}\"{from}/>\n",
        escape(url),
        bytes.len(),
        sign_bytes(bytes, pem)?
    ))
}

fn appcast(
    manifest: &ainc_release::Manifest,
    archive: &[u8],
    handoff: &DeltaHandoff,
    directory: &Path,
    pem: &str,
) -> Result<Vec<u8>> {
    manifest.verify_archive(archive)?;
    ensure!(
        manifest.archive_sha256 == handoff.archive_sha256,
        "delta handoff archive mismatch"
    );
    let prefix = manifest
        .archive_url
        .strip_suffix("AgentInc.tar.gz")
        .context("immutable archive URL")?;
    ensure!(
        prefix.ends_with(&format!("/releases/download/{}/", handoff.tag)),
        "delta handoff release mismatch"
    );
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<rss version=\"2.0\" xmlns:sparkle=\"http://www.andymatuschak.org/xml-namespaces/sparkle\"><channel><title>AgentInc</title><item><title>AgentInc {}</title><sparkle:version>{}</sparkle:version><sparkle:shortVersionString>{}</sparkle:shortVersionString><sparkle:minimumSystemVersion>15.0</sparkle:minimumSystemVersion><sparkle:hardwareRequirements>arm64</sparkle:hardwareRequirements><description sparkle:format=\"markdown\">{}</description>\n",
        manifest.version,
        escape(&manifest.build),
        manifest.version,
        escape(&manifest.changelog)
    );
    xml.push_str(&enclosure(&manifest.archive_url, archive, pem, None)?);
    if !handoff.deltas.is_empty() {
        xml.push_str("<sparkle:deltas>\n");
        for delta in &handoff.deltas {
            ensure!(
                delta.name
                    == format!(
                        "AgentInc-{}-from-{}.delta",
                        manifest.build.parse::<u64>()?,
                        delta.from_build.parse::<u64>()?
                    ),
                "invalid delta filename"
            );
            let bytes = fs::read(directory.join(&delta.name))?;
            ensure!(
                hex(&Sha256::digest(&bytes)) == delta.sha256,
                "delta digest mismatch"
            );
            xml.push_str(&enclosure(
                &format!("{prefix}{}", delta.name),
                &bytes,
                pem,
                Some(&delta.from_build),
            )?);
        }
        xml.push_str("</sparkle:deltas>\n");
    }
    xml.push_str("</item></channel></rss>\n");
    sign_feed(&xml, pem)
}

/// Linux runner only: authenticate the staged legacy archive, sign the exact
/// archive/delta bytes and feed, then upload to the same still-private draft.
pub fn sign_cli(raw: &[String]) -> Result<()> {
    let opts = parse_args(
        "release-sparkle-sign",
        raw,
        &["--test"],
        &["--tag", "--deltas"],
    );
    let tag = opts.required("--tag");
    let test = opts.flag("--test");
    if !test {
        ensure!(
            env::var("GITHUB_REF").as_deref() == Ok("refs/heads/main"),
            "only main may sign a production appcast"
        );
    }
    let directory = Path::new(opts.required("--deltas"));
    let handoff: DeltaHandoff =
        serde_json::from_slice(&fs::read(directory.join("sparkle-deltas.json"))?)?;
    ensure!(handoff.tag == tag, "delta handoff tag mismatch");
    let current = tempfile::tempdir()?;
    let manifest = fetch_release(tag, current.path(), test)?;
    let draft: Value = serde_json::from_str(&output(
        "gh",
        &[
            "release",
            "view",
            tag,
            "--json",
            "isDraft,targetCommitish,assets",
        ],
    )?)?;
    ensure!(
        draft["targetCommitish"] == manifest.commit,
        "Sparkle upload requires matching release commit"
    );
    if draft["isDraft"] == false {
        ensure!(
            draft["assets"]
                .as_array()
                .is_some_and(|assets| assets.iter().any(|asset| asset["name"] == "appcast.xml")),
            "published release is missing its Sparkle appcast"
        );
        println!("Sparkle release already published for this commit");
        return Ok(());
    }
    ensure!(draft["isDraft"] == true, "release draft state is missing");
    let pem = if test {
        let key = current.path().join("test.pem");
        checked(
            Command::new("openssl")
                .args(["genpkey", "-algorithm", "ED25519", "-out"])
                .arg(&key),
        )?;
        fs::read_to_string(key)?
    } else {
        env::var("UPDATE_SIGNING_KEY_ED25519_PEM").context("missing update signing key")?
    };
    // The legacy manifest verifies the production key above; explicitly verify
    // the new signer too, so a misprovisioned secret cannot publish a dead feed.
    let private = tempfile::tempdir()?;
    fs::set_permissions(private.path(), fs::Permissions::from_mode(0o700))?;
    fs::write(private.path().join("key.pem"), &pem)?;
    let public = Command::new("openssl")
        .args(["pkey", "-pubout", "-outform", "DER", "-in"])
        .arg(private.path().join("key.pem"))
        .output()?;
    ensure!(
        public.status.success() && public.stdout.len() == 44,
        "invalid Ed25519 public key"
    );
    let public = STANDARD.encode(&public.stdout[12..]);
    ensure!(
        test || public == ainc_release::UPDATE_PUBLIC_KEY,
        "Sparkle signing key differs from production key"
    );
    let bytes = fs::read(current.path().join("AgentInc.tar.gz"))?;
    let feed = appcast(&manifest, &bytes, &handoff, directory, &pem)?;
    let feed_path = directory.join("appcast.xml");
    fs::write(&feed_path, feed)?;
    let mut upload = Command::new("gh");
    upload.args(["release", "upload", tag]).arg(feed_path);
    for delta in &handoff.deltas {
        upload.arg(directory.join(&delta.name));
    }
    if test {
        let key_path = directory.join("test-sparkle-public-key.txt");
        fs::write(&key_path, public)?;
        upload.arg(key_path);
    }
    checked(upload.arg("--clobber"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(directory: &Path) -> String {
        let path = directory.join("key.pem");
        checked(
            Command::new("openssl")
                .args(["genpkey", "-algorithm", "ED25519", "-out"])
                .arg(&path),
        )
        .unwrap();
        fs::read_to_string(path).unwrap()
    }

    fn verify(bytes: &[u8], signature: &str, directory: &Path) -> bool {
        fs::write(directory.join("data"), bytes).unwrap();
        fs::write(
            directory.join("signature"),
            STANDARD.decode(signature).unwrap(),
        )
        .unwrap();
        Command::new("openssl")
            .args(["pkeyutl", "-verify", "-rawin", "-inkey"])
            .arg(directory.join("key.pem"))
            .arg("-in")
            .arg(directory.join("data"))
            .arg("-sigfile")
            .arg(directory.join("signature"))
            .output()
            .unwrap()
            .status
            .success()
    }

    #[test]
    fn archive_and_feed_signatures_authenticate_exact_bytes() {
        let root = tempfile::tempdir().unwrap();
        let pem = key(root.path());
        let signature = sign_bytes(b"archive", &pem).unwrap();
        assert!(verify(b"archive", &signature, root.path()));
        assert!(!verify(b"tampered archive", &signature, root.path()));
        let xml =
            "<?xml version=\"1.0\"?><rss><channel><title>AgentInc — é</title></channel></rss>\n";
        let feed = String::from_utf8(sign_feed(xml, &pem).unwrap()).unwrap();
        let (content, block) = feed.split_once("<!-- sparkle-signatures:\n").unwrap();
        assert_eq!(content, xml);
        let signature = block
            .lines()
            .next()
            .unwrap()
            .strip_prefix("edSignature: ")
            .unwrap();
        assert!(verify(content.as_bytes(), signature, root.path()));
        assert!(block.contains(&format!("length: {}\n", xml.len())));
        assert!(sign_feed(&feed, &pem).is_err());
    }

    fn fixture_manifest(archive: &[u8]) -> ainc_release::Manifest {
        serde_json::from_value(serde_json::json!({
            "version": "0.6.0", "build": "100", "commit": "a".repeat(40),
            "daemon_version": "0.6.0", "minimum_client": "0.1.0", "api": 1, "schema": 1,
            "architecture": "aarch64", "archive_url": "https://github.com/owner/repo/releases/download/v0.6.0/AgentInc.tar.gz",
            "archive_sha256": hex(&Sha256::digest(archive)), "archive_bytes": archive.len(),
            "notes": "Fix & improve", "changelog": "# AgentInc 0.6.0\n\nFix & improve <updates>",
        })).unwrap()
    }

    #[test]
    fn appcast_binds_full_and_delta_assets_to_immutable_release_and_build() {
        let root = tempfile::tempdir().unwrap();
        let pem = key(root.path());
        let archive = b"full archive";
        let manifest = fixture_manifest(archive);
        let name = "AgentInc-100-from-99.delta";
        fs::write(root.path().join(name), b"delta").unwrap();
        let mut handoff = DeltaHandoff {
            tag: "v0.6.0".into(),
            archive_sha256: manifest.archive_sha256.clone(),
            deltas: vec![Delta {
                from_build: "99".into(),
                name: name.into(),
                sha256: hex(&Sha256::digest(b"delta")),
            }],
        };
        let feed = appcast(&manifest, archive, &handoff, root.path(), &pem).unwrap();
        let feed_path = root.path().join("appcast.xml");
        fs::write(&feed_path, feed).unwrap();
        // Parse XML using a real parser, including the signature footer and
        // escaping of release notes; source text matching cannot satisfy this.
        let parsed = Command::new("ruby").args(["-rrexml/document", "-e", r#"
            doc = REXML::Document.new(File.read(ARGV[0]))
            item = doc.elements['rss/channel/item']
            abort unless item.elements['sparkle:version'].text == '100'
            abort unless item.elements['sparkle:shortVersionString'].text == '0.6.0'
            abort unless item.elements['description'].text.include?('Fix & improve <updates>')
            delta = item.elements['sparkle:deltas/enclosure']
            abort unless delta.attributes['sparkle:deltaFrom'] == '99'
            abort unless delta.attributes['url'] == 'https://github.com/owner/repo/releases/download/v0.6.0/AgentInc-100-from-99.delta'
            puts item.elements['enclosure'].attributes['sparkle:edSignature']
            puts delta.attributes['sparkle:edSignature']
        "#]).arg(&feed_path).output().unwrap();
        assert!(
            parsed.status.success(),
            "{}",
            String::from_utf8_lossy(&parsed.stderr)
        );
        let signatures = String::from_utf8(parsed.stdout).unwrap();
        let signatures: Vec<_> = signatures.lines().collect();
        assert!(verify(archive, signatures[0], root.path()));
        assert!(verify(b"delta", signatures[1], root.path()));
        fs::write(root.path().join(name), b"corrupt").unwrap();
        assert!(appcast(&manifest, archive, &handoff, root.path(), &pem).is_err());
        handoff.deltas.clear();
        assert!(
            appcast(
                &manifest,
                b"re-signed after delta generation",
                &handoff,
                root.path(),
                &pem
            )
            .is_err()
        );
        handoff.tag = "v0.7.0".into();
        assert!(appcast(&manifest, archive, &handoff, root.path(), &pem).is_err());
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "downloads pinned Sparkle tools; run explicitly on a Mac"]
    fn official_sparkle_tools_verify_signatures_and_reconstruct_signed_bundles() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let distribution = tools(&root).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let directory = directory.path();
        let pem = key(directory);
        let der = |public: bool| {
            let mut command = Command::new("openssl");
            command
                .args(["pkey", "-in"])
                .arg(directory.join("key.pem"))
                .args(["-outform", "DER"]);
            if public {
                command.arg("-pubout");
            }
            let result = command.output().unwrap();
            assert!(result.status.success());
            result.stdout
        };
        // Sparkle's supported new export format is the raw 32-byte seed
        // (common_cli/Secret.swift); use only a throwaway key for this test.
        let private = der(false);
        let public = der(true);
        assert_eq!((private.len(), public.len()), (48, 44));
        let sparkle_key = directory.join("sparkle.key");
        fs::write(&sparkle_key, STANDARD.encode(&private[16..])).unwrap();
        let archive = directory.join("archive.tar.gz");
        fs::write(&archive, b"archive bytes").unwrap();
        let signature = sign_bytes(b"archive bytes", &pem).unwrap();
        checked(
            Command::new(distribution.join("bin/sign_update"))
                .arg("--ed-key-file")
                .arg(&sparkle_key)
                .arg("--verify")
                .arg(&archive)
                .arg(signature),
        )
        .unwrap();
        let feed = directory.join("appcast.xml");
        fs::write(
            &feed,
            sign_feed("<?xml version=\"1.0\"?><rss><channel/></rss>\n", &pem).unwrap(),
        )
        .unwrap();
        checked(
            Command::new(distribution.join("bin/sign_update"))
                .arg("--ed-key-file")
                .arg(&sparkle_key)
                .arg("--verify")
                .arg(&feed),
        )
        .unwrap();

        let source = directory.join("main.c");
        fs::write(&source, "int main(void) { return 0; }\n").unwrap();
        let before = directory.join("before/AgentInc.app");
        fs::create_dir_all(before.join("Contents/MacOS")).unwrap();
        fs::write(before.join("Contents/Info.plist"), "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>test.sparkle</string><key>CFBundleExecutable</key><string>AgentInc</string><key>CFBundleVersion</key><string>1</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>").unwrap();
        checked(
            Command::new("clang")
                .arg(&source)
                .arg("-o")
                .arg(before.join("Contents/MacOS/AgentInc")),
        )
        .unwrap();
        fs::create_dir_all(before.join("Contents/Resources")).unwrap();
        stage(&root, &before).unwrap();
        checked(
            Command::new("codesign")
                .args(["--force", "--deep", "--sign", "-"])
                .arg(&before),
        )
        .unwrap();
        let after = directory.join("after/AgentInc.app");
        fs::create_dir_all(after.parent().unwrap()).unwrap();
        super::super::prepare::copytree(&before, &after, false).unwrap();
        fs::write(after.join("Contents/Resources/change"), b"new version").unwrap();
        checked(
            Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&after),
        )
        .unwrap();
        create_delta_with(
            &distribution.join("bin/BinaryDelta"),
            &before,
            &after,
            &directory.join("update.delta"),
        )
        .unwrap();
    }
}
