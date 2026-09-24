fn main() {
    println!("cargo:rerun-if-changed=src/update_window.m");
    #[cfg(target_os = "macos")]
    cc::Build::new()
        .file("src/update_window.m")
        .flag("-fobjc-arc")
        .compile("agentinc_update_window");
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=AppKit");
}
