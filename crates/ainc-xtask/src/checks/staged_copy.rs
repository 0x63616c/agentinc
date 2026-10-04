//! Reject em dashes in added lines of the Git index, including partially staged edits.
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub fn run(root: &Path) -> Result<()> {
    let output = crate::spawn::command("git")
        .args([
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            "--unified=0",
            "--",
        ])
        .current_dir(root)
        .output()
        .context("read staged changes")?;
    ensure!(output.status.success(), "git diff --cached failed");
    let diff = String::from_utf8_lossy(&output.stdout);
    let mut file = "";
    let mut in_hunk = false;
    let mut violations = Vec::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
        } else if !in_hunk && let Some(path) = line.strip_prefix("+++ ") {
            file = path;
        } else if line.starts_with("@@ ") {
            in_hunk = true;
        } else if in_hunk && line.starts_with('+') && line.contains('\u{2014}') {
            violations.push(format!("{file}: {}", &line[1..]));
        }
    }
    ensure!(
        violations.is_empty(),
        "staged additions must not contain em dashes (U+2014); use a full stop, comma or hyphen:\n{}",
        violations.join("\n")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn checks_the_index_instead_of_unstaged_contents() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "--quiet"]);
        let path = root.path().join("copy with spaces.txt");
        fs::write(&path, "A new version\u{2014}download it.\n").unwrap();
        git(root.path(), &["add", "."]);
        fs::write(&path, "A new version. Download it.\n").unwrap();
        assert!(
            run(root.path())
                .unwrap_err()
                .to_string()
                .contains("copy with spaces.txt")
        );
        git(root.path(), &["add", "."]);
        fs::write(&path, "Unstaged\u{2014}text.\n").unwrap();
        run(root.path()).unwrap();
    }

    #[test]
    fn allows_existing_em_dashes_and_their_removal() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "--quiet"]);
        let path = root.path().join("copy.txt");
        fs::write(&path, "Existing\u{2014}text.\nOld line.\n").unwrap();
        git(root.path(), &["add", "."]);
        git(
            root.path(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--quiet",
                "-m",
                "test: baseline",
            ],
        );
        fs::write(&path, "Existing\u{2014}text.\nNew line.\n").unwrap();
        git(root.path(), &["add", "."]);
        run(root.path()).unwrap();
        fs::write(&path, "New line.\n").unwrap();
        git(root.path(), &["add", "."]);
        run(root.path()).unwrap();
    }
}
