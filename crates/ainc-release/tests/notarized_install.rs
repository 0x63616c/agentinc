//! Real Mac acceptance using a signed, notarized draft feed downloaded from CI.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the real notarized draft artifact"]
fn install_authenticated_local_feed_and_reject_tampering() {
    use ainc_release::{SignedManifest, updater};
    use std::{fs, path::PathBuf};
    let input = PathBuf::from(
        std::env::var_os("AINC_SIGNED_UPDATE_DIR").expect("signed artifact directory"),
    );
    let key = fs::read_to_string(input.join("test-public-key.txt")).unwrap();
    let feed: SignedManifest =
        serde_json::from_slice(&fs::read(input.join("feed.json")).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let staged = root.path().join("stage");
    let manifest =
        updater::extract(&feed, key.trim(), &input.join("AgentInc.tar.gz"), &staged).unwrap();
    assert!(
        manifest
            .is_upgrade("0.0.0", std::env::consts::ARCH)
            .unwrap()
    );
    updater::verify_bundle(&staged.join("AgentInc.app"), &manifest).unwrap();
    let installed = root.path().join("AgentInc.app");
    fs::create_dir(&installed).unwrap();
    fs::write(installed.join("previous"), "prior installation marker").unwrap();
    let previous = updater::replace_bundle(&installed, &staged.join("AgentInc.app")).unwrap();
    updater::verify_bundle(&installed, &manifest).unwrap();
    assert_eq!(
        fs::read_to_string(previous.join("previous")).unwrap(),
        "prior installation marker"
    );
    assert!(installed.join("Contents/MacOS/aincd").is_file());
    assert!(
        installed
            .join("Contents/Resources/runtime/postgres/bin/postgres")
            .is_file()
    );
    let mut tampered = fs::read(input.join("AgentInc.tar.gz")).unwrap();
    tampered[0] ^= 1;
    let corrupt = root.path().join("corrupt.tar.gz");
    fs::write(&corrupt, tampered).unwrap();
    let rejected = root.path().join("rejected");
    assert!(updater::extract(&feed, key.trim(), &corrupt, &rejected).is_err());
    assert!(!rejected.exists());
    println!(
        "Installed and assessed notarized {} ({}) from local authenticated feed; corrupt archive rejected before extraction",
        manifest.version, manifest.commit
    );
}
