//! SQL is checked at compile time. Production code in `crates/ainc-daemon/src` uses
//! `sqlx::query!`/`query_as!`/`query_scalar!`, whose answers live in `.sqlx/` so a build needs no
//! database; this check keeps the runtime forms out, and [`prepare_check`] keeps `.sqlx/` equal
//! to what the migrations and the queries say.
//!
//! Runtime queries stay legal in test code (after a `#[cfg(test)]` line and under `tests/`), where
//! fixtures build SQL from scratch databases, and for the allow-list below.
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::{env, path::Path};

use crate::{installed, spawn, step};

/// `path:text` of runtime queries that cannot be macros, each with the reason. Empty: nothing
/// in the daemon builds SQL at run time.
const ALLOWED: [(&str, &str); 0] = [];

/// Every runtime query in one file's production code, as `path:line: text`.
pub(crate) fn violations(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if !path.starts_with("crates/ainc-daemon/src/") || !path.ends_with(".rs") {
        return found;
    }
    let runtime = Regex::new(r"sqlx::query(_as|_scalar)?(::<[^>]*>)?\(").unwrap();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        if runtime.is_match(line) && !ALLOWED.iter().any(|(allowed, _)| *allowed == path) {
            found.push(format!(
                "{path}:{}: runtime SQL; use sqlx::query!, query_as! or query_scalar!",
                index + 1
            ));
        }
    }
    found
}

pub fn run(root: &Path) -> Result<()> {
    let mut found = Vec::new();
    for (path, text) in super::tracked_text_files(root)? {
        found.extend(violations(&path, &text));
    }
    if !found.is_empty() {
        bail!("SQL checked at run time:\n{}", found.join("\n"));
    }
    println!("Daemon SQL is checked at compile time");
    Ok(())
}

/// The server part of a `postgres://…/database` URL, for naming sibling databases.
fn server_of(url: &str) -> Result<&str> {
    url.rsplit_once('/')
        .map(|(server, _)| server)
        .context("DATABASE_URL has no database name")
}

/// Run `body` with a scratch database holding the daemon's migrations, then drop it. Needs
/// `DATABASE_URL` (the server) and `sqlx-cli`; with `required` false a missing one is a note (`check` skips; `test` requires both).
fn scratch(root: &Path, required: bool, body: impl FnOnce(&str) -> Result<()>) -> Result<()> {
    let Some(base) = env::var("DATABASE_URL").ok() else {
        if required {
            bail!("DATABASE_URL is required for .sqlx/");
        }
        println!("note: DATABASE_URL is not set; skipping the .sqlx/ check");
        return Ok(());
    };
    if !installed("cargo-sqlx") {
        if required {
            bail!("sqlx-cli is required: {INSTALL}");
        }
        println!("note: sqlx-cli is not installed; skipping the .sqlx/ check ({INSTALL})");
        return Ok(());
    }
    let url = format!("{}/ainc_sqlx_{}", server_of(&base)?, std::process::id());
    let sqlx = |args: &[&str]| -> Result<()> {
        let mut command = vec!["cargo", "sqlx"];
        command.extend(args);
        command.extend(["--database-url", &url]);
        step(root, &command)
    };
    sqlx(&["database", "create"])?;
    let result = sqlx(&[
        "migrate",
        "run",
        "--source",
        "crates/ainc-daemon/migrations",
    ])
    .and_then(|()| body(&url));
    let dropped = sqlx(&["database", "drop", "-y"]);
    result.and(dropped)
}

const INSTALL: &str = "cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres";

/// `cargo sqlx prepare --check`: `.sqlx/` is what the migrations and the queries say.
pub fn prepare_check(root: &Path, required: bool) -> Result<()> {
    scratch(root, required, |url| {
        let status = spawn::command("cargo")
            .args(["sqlx", "prepare", "--check", "--workspace"])
            .args(["--database-url", url, "--", "--all-targets"])
            .current_dir(root)
            .status()?;
        anyhow::ensure!(
            status.success(),
            ".sqlx/ is stale: run `cargo xtask prepare-sqlx` and commit it"
        );
        Ok(())
    })
}

/// Rewrite `.sqlx/` from the queries: the fix `prepare_check` names. Uses a scratch database
/// like the check.
pub fn prepare(root: &Path) -> Result<()> {
    scratch(root, true, |url| {
        let status = spawn::command("cargo")
            .args(["sqlx", "prepare", "--workspace"])
            .args(["--database-url", url, "--", "--all-targets"])
            .current_dir(root)
            .status()?;
        anyhow::ensure!(status.success(), "cargo sqlx prepare failed");
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{server_of, violations};

    #[test]
    fn runtime_queries_in_production_code_are_named() {
        let text = "fn a() { sqlx::query_scalar::<_, i64>(\"SELECT 1\"); }\n#[cfg(test)]\nmod t { fn b() { sqlx::query(\"x\"); } }\n";
        assert_eq!(violations("crates/ainc-daemon/src/x.rs", text).len(), 1);
        assert!(violations("crates/ainc-daemon/tests/x.rs", text).is_empty());
        assert!(violations("crates/ainc-daemon/src/x.rs", "sqlx::query!(\"SELECT 1\")").is_empty());
    }

    #[test]
    fn the_scratch_database_replaces_only_the_name() {
        assert_eq!(
            server_of("postgres://u:p@h:5432/postgres").unwrap(),
            "postgres://u:p@h:5432"
        );
    }
}
