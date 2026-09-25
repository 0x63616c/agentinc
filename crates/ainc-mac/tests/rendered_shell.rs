//! Real Metal regression runner; AppKit must execute on the process main thread.
// The custom harness imports production modules; their ordinary #[test] functions
// are not registered here, so test-only imports and native startup code are unused.
#![allow(dead_code, unused_imports)]
#[path = "../src/about.rs"]
mod about;
#[path = "../src/assistant.rs"]
mod assistant;
#[path = "../src/automations.rs"]
mod automations;
#[path = "../src/evee.rs"]
mod evee;
#[path = "../src/input.rs"]
mod input;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/native_update.rs"]
mod native_update;
#[path = "../src/profile.rs"]
mod profile;
#[path = "../src/shell.rs"]
mod shell;
#[path = "../src/storage.rs"]
mod storage;
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/temporal.rs"]
mod temporal;
#[path = "../src/tickets.rs"]
mod tickets;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/updates.rs"]
mod updates;

#[cfg(target_os = "macos")]
#[path = "rendered/runner.rs"]
mod runner;
fn main() {
    #[cfg(target_os = "macos")]
    runner::run().expect("rendered shell regression");
    #[cfg(not(target_os = "macos"))]
    println!("Rendered shell test requires the macOS Metal backend; skipped.");
}
