//! Fixtures for tests. Nothing here touches a user's profile or subscription.

/// A fake `codex app-server` speaking the stdio JSON-RPC subset the daemon uses.
///
/// Like Codex, it keeps sign-in state in its home directory: `auth.json` present
/// means signed in. Sign-in completes at once unless `fixture-login-pending`
/// exists in the home, and every start appends a line to `fixture-spawns`.
/// The `codex-fixture` binary serves it over stdin/stdout for
/// `AINC_CODEX_PATH`; `executable()` locates that binary.
pub mod fake_codex {
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, Write},
        path::{Path, PathBuf},
    };

    pub const SIGNED_IN_AS: &str = "fixture@example.test · fixture";
    pub const MODEL: &str = "fixture";
    pub const AUTH_URL: &str = "https://fixture.example.test/sign-in";
    pub const ACCESS_TOKEN: &str = "fixture-access-token";
    pub const ACCOUNT_ID: &str = "fixture-account";

    /// The `auth.json` Codex writes after a sign-in, with fixture tokens.
    pub fn auth_file() -> String {
        json!({"tokens":{"access_token":ACCESS_TOKEN,"account_id":ACCOUNT_ID}}).to_string()
    }
    /// Sign the fixture profile in without a browser round trip.
    pub fn sign_in(home: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(home)?;
        std::fs::write(home.join("auth.json"), auth_file())
    }
    /// Make the next sign-in wait for cancellation instead of completing.
    pub fn hold_sign_in(home: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(home)?;
        std::fs::write(home.join("fixture-login-pending"), "")
    }
    /// How many times the fake was started for this home.
    pub fn spawns(home: &Path) -> usize {
        std::fs::read_to_string(home.join("fixture-spawns"))
            .map(|log| log.lines().count())
            .unwrap_or(0)
    }
    /// The `codex-fixture` binary Cargo builds next to this crate's tests.
    pub fn executable() -> PathBuf {
        if let Some(path) = option_env!("CARGO_BIN_EXE_codex-fixture") {
            return path.into();
        }
        let exe = std::env::current_exe().expect("test executable path");
        let target = exe
            .parent()
            .and_then(|deps| deps.parent())
            .expect("target directory");
        let path = target.join("codex-fixture");
        assert!(
            path.is_file(),
            "{} is missing; run cargo test for the whole crate so its binaries build",
            path.display()
        );
        path
    }

    pub fn serve(input: impl BufRead, mut output: impl Write, home: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(home)?;
        let mut spawns = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(home.join("fixture-spawns"))?;
        writeln!(spawns, "{}", std::process::id())?;
        let mut send = |message: Value| -> std::io::Result<()> {
            writeln!(output, "{message}")?;
            output.flush()
        };
        for line in input.lines() {
            let request: Value = match serde_json::from_str(&line?) {
                Ok(request) => request,
                Err(_) => continue,
            };
            let Some(id) = request.get("id").cloned() else {
                continue; // notifications such as `initialized`
            };
            let result = match request["method"].as_str() {
                Some("initialize") | Some("account/login/cancel") => json!({}),
                Some("account/read") => {
                    if home.join("auth.json").is_file() {
                        json!({"account":{"type":"chatgpt","email":"fixture@example.test","planType":"fixture"}})
                    } else {
                        json!({"account":null})
                    }
                }
                Some("model/list") => {
                    json!({"data":[{"model":MODEL,"displayName":"Fixture","isDefault":true}],"nextCursor":null})
                }
                Some("account/login/start") => {
                    send(json!({"id":id,"result":{"authUrl":AUTH_URL,"loginId":"login-fixture"}}))?;
                    if !home.join("fixture-login-pending").is_file() {
                        sign_in(home)?;
                        send(
                            json!({"method":"account/login/completed","params":{"loginId":"login-fixture","success":true}}),
                        )?;
                    }
                    continue;
                }
                Some("account/logout") => {
                    let _ = std::fs::remove_file(home.join("auth.json"));
                    json!({})
                }
                _ => {
                    send(
                        json!({"id":id,"error":{"code":-32601,"message":"private backend detail"}}),
                    )?;
                    continue;
                }
            };
            send(json!({"id":id,"result":result}))?;
        }
        Ok(())
    }
}
