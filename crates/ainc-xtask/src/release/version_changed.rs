//! Only a product version change on main starts a release.
use super::read_to_string;
use anyhow::{Result, anyhow, bail};
use std::{path::Path, process::Command};

const WORKSPACE_PATH: &str = "Cargo.toml";
const LEGACY_PATH: &str = "crates/ainc-release/Cargo.toml";

/// The string at `keys` in `path` as of `revision`, or `None` when the file does not exist there
/// or the value is missing or not a string (e.g. `version.workspace = true`).
fn previous_version(root: &Path, revision: &str, path: &str, keys: &[&str]) -> Option<String> {
    let previous = Command::new("git")
        .args(["show", &format!("{revision}:{path}")])
        .current_dir(root)
        .output()
        .ok()?;
    if !previous.status.success() {
        return None;
    }
    let mut data: toml::Value = String::from_utf8(previous.stdout).ok()?.parse().ok()?;
    for key in keys {
        data = data
            .get(key)
            .cloned()
            .unwrap_or_else(|| toml::Table::new().into());
    }
    data.as_str().map(str::to_string)
}

/// Whether the product version in `root` differs from the one at `before`.
pub fn version_changed(root: &Path, before: &str) -> Result<bool> {
    let workspace: toml::Value = read_to_string(&root.join(WORKSPACE_PATH))?.parse()?;
    let current = workspace["workspace"]["package"]["version"]
        .as_str()
        .ok_or_else(|| anyhow!("workspace.package.version is not a string"))?
        .to_string();
    let old = previous_version(
        root,
        before,
        WORKSPACE_PATH,
        &["workspace", "package", "version"],
    )
    .or_else(|| previous_version(root, before, LEGACY_PATH, &["package", "version"]));
    Ok(old.as_deref() != Some(current.as_str()))
}

pub fn cli(args: &[String]) -> Result<()> {
    let [before] = args else {
        bail!("usage: cargo xtask release-version-changed REVISION");
    };
    println!("{}", version_changed(&std::env::current_dir()?, before)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            // A pre-commit hook exports these; the fixture repo must be its own.
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn repository(root: &Path) {
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Release Test"]);
        git(root, &["config", "user.email", "release-test@example.com"]);
    }

    fn commit(root: &Path) -> String {
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "snapshot"]);
        git(root, &["rev-parse", "HEAD"])
    }

    // The tests call git show through the library, which must see the fixture repo too.
    fn changed(root: &Path, before: &str) -> bool {
        version_changed(root, before).unwrap()
    }

    #[test]
    fn only_product_version_starts_release() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        repository(root);
        let manifest = root.join("Cargo.toml");
        fs::write(&manifest, "[workspace.package]\nversion = \"0.2.0\"\n").unwrap();
        let old = commit(root);

        // SDK or other changes must not start Distribution.
        fs::write(
            &manifest,
            "[workspace.package]\nversion = \"0.2.0\"\n[workspace.dependencies]\nturnkeel = \"0.3.0\"\n",
        )
        .unwrap();
        assert!(!changed(root, &old));
        let unchanged = commit(root);

        fs::write(&manifest, "[workspace.package]\nversion = \"0.3.0\"\n").unwrap();
        assert!(changed(root, &unchanged));
    }

    #[test]
    fn layout_migration_does_not_start_release() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        repository(root);
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nedition = \"2024\"\n",
        )
        .unwrap();
        let legacy = root.join("crates/ainc-release/Cargo.toml");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, "[package]\nversion = \"0.2.0\"\n").unwrap();
        let before = commit(root);

        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.2.0\"\n",
        )
        .unwrap();
        fs::write(&legacy, "[package]\nversion.workspace = true\n").unwrap();
        assert!(!changed(root, &before));
    }
}
