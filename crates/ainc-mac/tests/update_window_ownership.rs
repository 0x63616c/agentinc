#![cfg(target_os = "macos")]

use std::{fs, path::PathBuf, process::Command};

#[test]
fn sparkle_driver_callbacks_complete_replies_and_preserve_native_ownership() {
    check_driver(false);
}

#[test]
fn upgrade_fixture_handoff_survives_relaunch_without_editing_signed_bundles() {
    check_driver(true);
}

fn check_driver(upgrade_test: bool) {
    check_fixture(
        upgrade_test,
        "sparkle_driver.m",
        b"Sparkle driver callbacks passed\n",
    );
}

#[test]
fn inert_cache_serves_selected_archive_through_public_download_hook() {
    check_fixture(
        false,
        "sparkle_cache.m",
        b"Sparkle inert HTTP cache passed\n",
    );
}

fn check_fixture(upgrade_test: bool, fixture: &str, expected: &[u8]) {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = crate_dir.join("../../.local/update-window-tests");
    fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let executable = directory.path().join("sparkle-driver");
    let mut compile = Command::new("clang");
    if upgrade_test {
        compile.arg("-DAINC_UPGRADE_TEST");
    }
    let compile = compile
        .args([
            "-fobjc-arc",
            "-fblocks",
            "-DBUILDING_SPARKLE_SOURCES_EXTERNALLY",
            "-framework",
            "AppKit",
            "-I",
        ])
        .arg(crate_dir.join("../../vendor/sparkle/Headers"))
        .arg(crate_dir.join("tests/fixtures").join(fixture))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "driver harness failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success(),
        "driver callbacks failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, expected);
}

#[test]
fn update_windows_survive_check_offer_progress_and_close() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = crate_dir.join("../../.local/update-window-tests");
    fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let executable = directory.path().join("ownership");
    let compile = Command::new("clang")
        .args([
            "-fobjc-arc",
            "-fblocks",
            "-DBUILDING_SPARKLE_SOURCES_EXTERNALLY",
            "-framework",
            "AppKit",
            "-I",
        ])
        .arg(crate_dir.join("../../vendor/sparkle/Headers"))
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

    let result = Command::new(executable)
        .env(
            "AINC_UPDATE_TEST_ICON",
            crate_dir.join("assets/AppIcon.png"),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "update window transitions failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"update window ownership passed\n");
}
