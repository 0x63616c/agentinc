#![cfg(target_os = "macos")]

use std::{fs, path::PathBuf, process::Command};

#[test]
fn update_windows_survive_check_offer_progress_and_close() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = crate_dir.join("../../.local/update-window-tests");
    fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let executable = directory.path().join("ownership");
    let compile = Command::new("clang")
        .args(["-fobjc-arc", "-framework", "AppKit"])
        .arg(crate_dir.join("src/update_window.m"))
        .arg(crate_dir.join("tests/fixtures/update_window_ownership.m"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "Objective-C harness failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success(),
        "update window transitions failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"update window ownership passed\n");
}
