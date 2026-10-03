fn main() {
    println!("cargo:rerun-if-env-changed=AINC_CHANNEL");
    println!("cargo:rerun-if-env-changed=AINC_COMMIT");
    println!("cargo:rerun-if-env-changed=AINC_UPGRADE_TEST_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=AINC_UPGRADE_TEST_VERSION");
    println!("cargo:rustc-check-cfg=cfg(ainc_production)");
    println!("cargo:rustc-check-cfg=cfg(ainc_upgrade_test)");
    if std::env::var("AINC_CHANNEL").as_deref() == Ok("production") {
        println!("cargo:rustc-cfg=ainc_production");
    }
    if std::env::var_os("AINC_UPGRADE_TEST_PUBLIC_KEY").is_some() {
        assert_eq!(std::env::var("AINC_CHANNEL").as_deref(), Ok("production"));
        println!("cargo:rustc-cfg=ainc_upgrade_test");
    } else {
        assert!(std::env::var_os("AINC_UPGRADE_TEST_VERSION").is_none());
    }
    // The commit comes only from the release pipeline (`AINC_COMMIT`, set by `release prepare`).
    // Development builds carry none, so a commit never invalidates a build.
}
