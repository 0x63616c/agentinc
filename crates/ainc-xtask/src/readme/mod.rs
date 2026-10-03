//! The README hero screenshot tooling, ported from the Python scripts that used to live in
//! `scripts/`. A faithful port: same flags, same outputs, same exit codes.
use anyhow::{Result, anyhow};
use std::{path::Path, process::exit};

pub mod capture;
pub mod compose;

/// The subcommands this module serves, for the xtask usage string.
pub const NAMES: &str = "readme-capture|readme-compose";

pub fn handles(operation: &str) -> bool {
    NAMES.split('|').any(|name| name == operation)
}

/// Run one readme subcommand. Like the scripts did, a failure prints its message and exits 1.
pub fn run(operation: &str, args: Vec<String>, root: &Path) -> Result<()> {
    let result = match operation {
        "readme-capture" => capture::cli(root, &args),
        "readme-compose" => compose::cli(root, &args),
        other => Err(anyhow!("unknown readme command {other}")),
    };
    if let Err(error) = result {
        eprintln!("{error:#}");
        exit(1);
    }
    Ok(())
}
