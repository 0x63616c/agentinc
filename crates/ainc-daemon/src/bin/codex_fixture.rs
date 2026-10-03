//! Fake Codex app-server for tests; see `ainc_daemon::testing::fake_codex`.
#![allow(clippy::disallowed_macros)] // user-facing output
use std::path::PathBuf;
fn main() -> std::io::Result<()> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("CODEX_HOME is required"))?;
    ainc_daemon::testing::fake_codex::serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &home,
    )
}
