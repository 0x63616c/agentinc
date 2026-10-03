//! Version helpers for releases: `bump` (the `just release` recipe) and the small version reads
//! and comparisons `.github/workflows/release.yml` used to do with inline Python.
use super::{checked, output, read_to_string};
use anyhow::{Result, anyhow, bail};
use regex::Regex;
use std::{fs, path::Path, process::exit};

type Version = (u64, u64, u64);

fn show(version: Version) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// The workspace version in `Cargo.toml` text: the first `version = "x.y.z"` line.
fn current_version(manifest: &str) -> Result<Version> {
    let pattern = Regex::new(r#"(?m)^version = "(\d+)\.(\d+)\.(\d+)""#)?;
    let found = pattern
        .captures(manifest)
        .ok_or_else(|| anyhow!("no workspace version in Cargo.toml"))?;
    let part = |index: usize| Ok::<u64, anyhow::Error>(found[index].parse()?);
    Ok((part(1)?, part(2)?, part(3)?))
}

/// The version `bump` selects: `patch`, `minor`, `major`, or an explicit one of those three.
fn next_version(current: Version, bump: &str) -> Result<Version> {
    let (major, minor, patch) = current;
    let allowed = [
        ("patch", (major, minor, patch + 1)),
        ("minor", (major, minor + 1, 0)),
        ("major", (major + 1, 0, 0)),
    ];
    if let Some((_, version)) = allowed.iter().find(|(name, _)| *name == bump) {
        return Ok(*version);
    }
    let explicit = bump
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok();
    if let Some((_, version)) = allowed
        .iter()
        .find(|(_, version)| explicit.as_deref() == Some(&[version.0, version.1, version.2][..]))
    {
        return Ok(*version);
    }
    let options = allowed
        .iter()
        .map(|(name, version)| format!("{} ({name})", show(*version)))
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "Current version is {}. Next must be one of: {options}.",
        show(current)
    )
}

/// `manifest` with the first `version = "..."` line set to `version`.
fn with_version(manifest: &str, version: Version) -> Result<String> {
    let pattern = Regex::new(r#"(?m)^version = ".*?""#)?;
    let line = format!(r#"version = "{}""#, show(version));
    Ok(pattern
        .replace(manifest, regex::NoExpand(&line))
        .into_owned())
}

pub fn bump(root: &Path, args: &[String]) -> Result<()> {
    let [bump] = args else {
        bail!("usage: cargo xtask bump patch|minor|major|X.Y.Z");
    };
    let path = root.join("Cargo.toml");
    let manifest = read_to_string(&path)?;
    let current = current_version(&manifest)?;
    let new = next_version(current, bump)?;
    if !output(
        "git",
        &["-C", &root.to_string_lossy(), "status", "--porcelain"],
    )?
    .is_empty()
    {
        bail!(
            "Working tree is not clean: commit or stash first, so the release commit holds only the version bump."
        );
    }
    fs::write(&path, with_version(&manifest, new)?)?;
    let run = |program: &str, args: &[&str]| {
        checked(crate::spawn::command(program).args(args).current_dir(root))
    };
    run("cargo", &["update", "--workspace"])?;
    run("cargo", &["xtask", "generate"])?;
    run("git", &["add", "-A"])?;
    run(
        "git",
        &[
            "commit",
            "-m",
            &format!("chore(release): release {}", show(new)),
        ],
    )?;
    println!("Committed Release {}. Push to main to ship it.", show(new));
    Ok(())
}

fn parse(text: &str) -> Result<Version> {
    let parts = text
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| anyhow!("invalid version {text:?}"))?;
    match parts[..] {
        [major, minor, patch] => Ok((major, minor, patch)),
        _ => bail!("invalid version {text:?}"),
    }
}

fn workspace_version(manifest: &str) -> Result<String> {
    let data: toml::Value = manifest.parse()?;
    data["workspace"]["package"]["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("workspace.package.version is not a string"))
}

fn package_version(metadata: &str, name: &str) -> Result<String> {
    let data: serde_json::Value = serde_json::from_str(metadata)?;
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|package| package["name"] == name)
        .and_then(|package| package["version"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no package named {name}"))
}

fn next_patch(version: &str) -> Result<String> {
    let (major, minor, patch) = parse(version)?;
    Ok(show((major, minor, patch + 1)))
}

/// `cargo xtask release-version`: the workspace version from Cargo.toml.
pub fn version_cli(root: &Path, args: &[String]) -> Result<()> {
    if !args.is_empty() {
        bail!("usage: cargo xtask release-version");
    }
    println!(
        "{}",
        workspace_version(&read_to_string(&root.join("Cargo.toml"))?)?
    );
    Ok(())
}

/// `cargo xtask release-package-version NAME`: a workspace package's version per cargo metadata.
pub fn package_version_cli(root: &Path, args: &[String]) -> Result<()> {
    let [name] = args else {
        bail!("usage: cargo xtask release-package-version PACKAGE");
    };
    let metadata = output(
        "cargo",
        &[
            "metadata",
            "--no-deps",
            "--format-version=1",
            "--manifest-path",
            &root.join("Cargo.toml").to_string_lossy(),
        ],
    )?;
    println!("{}", package_version(&metadata, name)?);
    Ok(())
}

/// `cargo xtask release-next-patch VERSION`.
pub fn next_patch_cli(args: &[String]) -> Result<()> {
    let [version] = args else {
        bail!("usage: cargo xtask release-next-patch VERSION");
    };
    println!("{}", next_patch(version)?);
    Ok(())
}

/// `cargo xtask release-version-less A B`: exit 0 when A < B, 1 otherwise, silently.
pub fn version_less_cli(args: &[String]) -> Result<()> {
    let [a, b] = args else {
        bail!("usage: cargo xtask release-version-less A B");
    };
    exit(if parse(a)? < parse(b)? { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "[workspace.package]\nversion = \"0.4.9\"\nedition = \"2024\"\n[workspace.dependencies]\nx = { version = \"1.0.0\" }\n";

    #[test]
    fn reads_the_first_version_line() {
        assert_eq!(current_version(MANIFEST).unwrap(), (0, 4, 9));
        assert!(current_version("[package]\n").is_err());
    }

    #[test]
    fn selects_named_and_explicit_versions() {
        let current = (0, 4, 9);
        assert_eq!(next_version(current, "patch").unwrap(), (0, 4, 10));
        assert_eq!(next_version(current, "minor").unwrap(), (0, 5, 0));
        assert_eq!(next_version(current, "major").unwrap(), (1, 0, 0));
        assert_eq!(next_version(current, "0.4.10").unwrap(), (0, 4, 10));
        assert_eq!(next_version(current, "1.0.0").unwrap(), (1, 0, 0));
    }

    #[test]
    fn rejects_anything_else_with_the_options() {
        for bad in [
            "0.4.9", "0.4.11", "2.0.0", "1.0", "x", "", "1.0.0.0", "minor ",
        ] {
            let error = next_version((0, 4, 9), bad).unwrap_err().to_string();
            assert_eq!(
                error,
                "Current version is 0.4.9. Next must be one of: 0.4.10 (patch), 0.5.0 (minor), 1.0.0 (major)."
            );
        }
    }

    #[test]
    fn rewrites_only_the_first_version() {
        let updated = with_version(MANIFEST, (0, 5, 0)).unwrap();
        assert!(updated.contains("version = \"0.5.0\"\nedition"));
        assert!(updated.contains("x = { version = \"1.0.0\" }"));
        assert_eq!(updated.matches("0.5.0").count(), 1);
    }

    #[test]
    fn reads_versions() {
        assert_eq!(workspace_version(MANIFEST).unwrap(), "0.4.9");
        let metadata = r#"{"packages":[{"name":"a","version":"1.0.0"},{"name":"ainc-release","version":"0.4.9"}]}"#;
        assert_eq!(package_version(metadata, "ainc-release").unwrap(), "0.4.9");
        assert!(package_version(metadata, "missing").is_err());
    }

    #[test]
    fn next_patch_and_ordering_compare_numerically() {
        assert_eq!(next_patch("0.4.9").unwrap(), "0.4.10");
        assert!(parse("0.4.9").unwrap() < parse("0.4.10").unwrap());
        assert!(parse("0.9.0").unwrap() < parse("0.10.0").unwrap());
        assert!(parse("1.2").is_err());
    }
}
