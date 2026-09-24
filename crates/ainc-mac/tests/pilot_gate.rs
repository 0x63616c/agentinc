//! A normal app must refuse automation before initializing any native platform.
#[cfg(not(feature = "automation"))]
#[test]
fn default_build_has_no_driver_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let session = temp.path().join("must-not-exist");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_agentinc-os"))
        .arg("--gpui-pilot-session")
        .arg(&session)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not accept automation"));
    assert!(!session.exists());
}
