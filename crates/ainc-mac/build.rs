fn main() {
    println!("cargo:rerun-if-changed=src/update_window.m");
    println!("cargo:rerun-if-changed=src/calendar_bridge.m");
    println!("cargo:rerun-if-env-changed=AINC_UPGRADE_TEST_PUBLIC_KEY");
    println!("cargo:rustc-check-cfg=cfg(ainc_upgrade_test)");
    let upgrade_test = std::env::var_os("AINC_UPGRADE_TEST_PUBLIC_KEY").is_some();
    if upgrade_test {
        println!("cargo:rustc-cfg=ainc_upgrade_test");
    }
    #[cfg(target_os = "macos")]
    {
        let mut build = cc::Build::new();
        build
            .file("src/update_window.m")
            .file("src/calendar_bridge.m")
            .flag("-fobjc-arc");
        if upgrade_test {
            build.define("AINC_UPGRADE_TEST", None);
        }
        build.compile("agentinc_update_window");
    }
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=AppKit");
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=EventKit");
}
