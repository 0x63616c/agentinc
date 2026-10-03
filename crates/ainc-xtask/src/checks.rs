//! Repository checks, including native UI rules and Git commit messages.
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub mod colors;
pub mod commit_msg;
pub mod copy;
pub mod env_names;
pub mod layout;
pub mod naming;
pub mod openapi;
pub mod sdk_vocabulary;
pub mod ui_core;
pub mod ui_spacing;
pub mod ui_vocabulary;

/// Every file under `dir` with the given extension, sorted so output is stable.
fn files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        if !next.exists() {
            continue;
        }
        for entry in fs::read_dir(&next)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == extension) {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Every Git-tracked file under `root` that holds UTF-8 text, as `(repo-relative path, text)`,
/// in `git ls-files` order. Binary files and the untracked `target/` and `.local/` never appear.
fn tracked_text_files(root: &Path) -> Result<Vec<(String, String)>> {
    let output = crate::spawn::command("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .context("could not list tracked files")?;
    anyhow::ensure!(output.status.success(), "git ls-files failed");
    let mut files = Vec::new();
    for path in String::from_utf8(output.stdout)?.split('\0') {
        if path.is_empty() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(root.join(path)) {
            files.push((path.to_string(), text));
        }
    }
    Ok(files)
}
