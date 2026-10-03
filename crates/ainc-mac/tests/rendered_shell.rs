//! Real Metal regression runner; AppKit must execute on the process main thread.
#![allow(clippy::disallowed_macros)]

#[cfg(target_os = "macos")]
#[path = "rendered/runner.rs"]
mod runner;
fn main() {
    #[cfg(target_os = "macos")]
    runner::run().expect("rendered shell regression");
    #[cfg(not(target_os = "macos"))]
    println!("Rendered shell test requires the macOS Metal backend; skipped.");
}
