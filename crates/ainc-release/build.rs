use std::process::Command;

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
    if std::env::var_os("AINC_COMMIT").is_none() {
        if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output()
            && output.status.success()
        {
            println!(
                "cargo:rustc-env=AINC_COMMIT={}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
        let branch = Command::new("git")
            .args(["symbolic-ref", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
        for reference in [Some("HEAD"), branch.as_deref()].into_iter().flatten() {
            if let Ok(output) = Command::new("git")
                .args(["rev-parse", "--git-path", reference])
                .output()
            {
                println!(
                    "cargo:rerun-if-changed={}",
                    String::from_utf8_lossy(&output.stdout).trim()
                );
            }
        }
    }
}
