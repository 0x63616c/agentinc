#![allow(clippy::disallowed_macros)] // cargo build-script protocol
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
