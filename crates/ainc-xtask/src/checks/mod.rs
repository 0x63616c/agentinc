//! The native UI checks that used to be Python scripts under `crates/ainc-mac/scripts`.
use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub mod colors;
pub mod ui_core;
pub mod ui_spacing;

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
