//! Fail on source layout drift: a module with children is `foo.rs` beside `foo/`, never
//! `foo/mod.rs`, and every file in the Mac app opens with a `//!` line saying what it holds.
use anyhow::{Result, bail};
use std::path::Path;

/// `mod.rs` files the engine lane still owns; empty this list as they move.
const MOD_RS_FOLLOW_UPS: [&str; 2] = [
    "crates/turnkeel/src/engine/mod.rs",
    "crates/turnkeel/src/testing/mod.rs",
];

const DOCUMENTED_TREE: &str = "crates/ainc-mac/src/";

/// Every violation for one tracked file, as `path: reason`.
pub(crate) fn violations(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if !path.starts_with("crates/") || !path.ends_with(".rs") {
        return found;
    }
    if path.ends_with("/mod.rs") && !MOD_RS_FOLLOW_UPS.contains(&path) {
        found.push(format!(
            "{path}: use `foo.rs` beside `foo/`, not `foo/mod.rs`"
        ));
    }
    if path.starts_with(DOCUMENTED_TREE) && !text.starts_with("//!") {
        found.push(format!(
            "{path}: open with a `//!` line saying what the file holds"
        ));
    }
    found
}

pub fn run(root: &Path) -> Result<()> {
    let mut found = Vec::new();
    for (path, text) in super::tracked_text_files(root)? {
        found.extend(violations(&path, &text));
    }
    if !found.is_empty() {
        bail!("layout violations:\n{}", found.join("\n"));
    }
    println!("Source layout: no mod.rs, every app file documented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::violations;

    #[test]
    fn mod_rs_is_reported_outside_the_follow_up_list() {
        assert_eq!(
            violations("crates/ainc-mac/src/ui/mod.rs", "//! x").len(),
            1
        );
        assert!(violations("crates/turnkeel/src/engine/mod.rs", "use x;").is_empty());
        assert!(violations("crates/ainc-mac/src/ui.rs", "//! x").is_empty());
    }

    #[test]
    fn app_files_open_with_a_doc_line() {
        assert_eq!(
            violations("crates/ainc-mac/src/shell.rs", "use gpui::*;").len(),
            1
        );
        assert!(
            violations(
                "crates/ainc-mac/src/shell.rs",
                "//! The shell.\nuse gpui::*;"
            )
            .is_empty()
        );
        assert!(violations("crates/ainc-daemon/src/lib.rs", "use axum;").is_empty());
    }
}
