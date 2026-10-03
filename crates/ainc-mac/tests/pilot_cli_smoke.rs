//! macOS CLI/OS smoke: drive the real app through the `gpui-pilot-cli` CLI with a visible window and
//! check the native window with Swift. Needs a desktop, so it is opt-in (`--ignored`).
//! Build first: `cargo build --workspace --features automation`.
#![cfg(target_os = "macos")]
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const TITLE: &str = "AgentInc Pilot CLI QA";

struct App(Child);
impl Drop for App {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

struct Cli {
    binary: PathBuf,
    manifest: PathBuf,
}

impl Cli {
    fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<Value> {
        let mut child = Command::new(&self.binary)
            .args(["--instance"])
            .arg(&self.manifest)
            .arg("--json")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(text) = stdin {
            child
                .stdin
                .take()
                .context("stdin")?
                .write_all(text.as_bytes())?;
        }
        let result = child.wait_with_output()?;
        if !result.status.success() {
            bail!("{}", String::from_utf8_lossy(&result.stderr));
        }
        let mut reply: Value = serde_json::from_slice(&result.stdout)?;
        Ok(reply["output"].take())
    }
}

/// The first node's `reference` whose `author_id` matches.
fn node_reference(snapshot: &Value, author_id: &str) -> Option<String> {
    snapshot["nodes"]
        .as_array()?
        .iter()
        .find(|node| node["author_id"] == author_id)
        .and_then(|node| node["reference"].as_str().map(str::to_owned))
}

fn run(root: &Path, app_binary: &Path) -> Result<()> {
    let output = root.join("target/pilot-acceptance");
    fs::create_dir_all(&output)?;
    fs::create_dir_all(root.join(".local"))?;
    let temp = tempfile::Builder::new()
        .prefix("c")
        .tempdir_in(root.join(".local"))?;
    let state = temp.path();
    let session = state.join("s");
    let log = fs::File::create(output.join("cli-app.log"))?;
    let mut app = App(Command::new(app_binary)
        .arg("--gpui-pilot-session")
        .arg(&session)
        .arg("--gpui-pilot-visible")
        .env("AINC_SESSION_PATH", state.join("session.json"))
        .env(
            "AINC_DISCOVERY_FILE",
            std::env::var_os("AINC_DISCOVERY_FILE")
                .context("run against an isolated cargo xtask dev stack")?,
        )
        .env("AINC_LEGACY_DIR", state.join("legacy"))
        .env("AGENTINC_CODEX_HOME", state.join("codex"))
        .env("AINC_WINDOW_TITLE", TITLE)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?);
    let manifest = session.join("instance.json");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !manifest.exists() {
        ensure!(app.0.try_wait()?.is_none(), "app exited");
        ensure!(Instant::now() < deadline, "startup timeout");
        std::thread::sleep(Duration::from_millis(20)); // Process readiness only; UI waits use the protocol.
    }
    let cli = Cli {
        binary: app_binary.with_file_name("gpui-pilot-cli"),
        manifest: manifest.clone(),
    };
    ensure!(
        cli.binary.is_file(),
        "{} is missing; run cargo build --workspace --features automation",
        cli.binary.display()
    );

    ensure!(cli.run(&["hello"], None)?["title"] == TITLE);
    let snapshot = cli.run(&["snapshot"], None)?["snapshot"].take();
    ensure!(node_reference(&snapshot, "shell.search").is_some());
    let native = Command::new("swift")
        .arg(root.join("crates/ainc-mac/tests/pilot_os_acceptance.swift"))
        .arg(app.0.id().to_string())
        .arg(TITLE)
        .current_dir(root)
        .output()?;
    ensure!(native.status.success(), "swift acceptance failed");
    fs::write(output.join("os-window.json"), &native.stdout)?;
    cli.run(&["press", "cmd-k"], None)?;
    cli.run(
        &[
            "wait",
            r#"{"kind":"present","author_id":"search.input"}"#,
            "3000",
        ],
        None,
    )?;
    let mut typed = false;
    for _ in 0..100 {
        let snapshot = cli.run(&["snapshot"], None)?["snapshot"].take();
        let reference = node_reference(&snapshot, "search.input").context("search.input")?;
        match cli.run(&["type", &reference, "--stdin"], Some("Tickets")) {
            Ok(_) => {
                typed = true;
                break;
            }
            Err(error) if error.to_string().contains("stale_ref") => {}
            Err(error) => return Err(error),
        }
    }
    ensure!(typed, "unable to get fresh ref");
    cli.run(
        &[
            "wait",
            r#"{"kind":"value","author_id":"search.input","equals":"Tickets"}"#,
            "3000",
        ],
        None,
    )?;
    cli.run(&["press", "enter"], None)?;
    cli.run(
        &[
            "wait",
            r#"{"kind":"present","author_id":"tickets.create"}"#,
            "3000",
        ],
        None,
    )?;
    let capture = cli.run(&["screenshot"], None)?;
    let path = capture["path"].as_str().context("screenshot path")?;
    ensure!(Path::new(path).is_file(), "screenshot missing");
    println!(
        "CLI hello/snapshot/press/type/wait/screenshot passed; native PID/title/dimensions passed."
    );
    println!("{}", String::from_utf8_lossy(&native.stdout));
    // Graceful quit via the same driver; a closed transport is expected.
    let _ = cli.run(&["press", "cmd-q"], None);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.0.try_wait()?.is_none() {
        ensure!(Instant::now() < deadline, "app did not quit");
        std::thread::sleep(Duration::from_millis(20)); // Process exit only.
    }
    ensure!(
        !manifest.exists(),
        "owned endpoints were not removed on normal quit"
    );
    println!("Normal shutdown removed instance manifest/token/socket.");
    Ok(())
}

#[test]
#[ignore = "needs a real desktop and a visible window; run with --ignored"]
fn cli_drives_the_visible_app_and_quits_cleanly() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    run(
        &root.canonicalize()?,
        Path::new(env!("CARGO_BIN_EXE_AgentInc")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_node_reference_by_author_id() {
        let snapshot = serde_json::json!({"nodes": [
            {"author_id": "a", "reference": "r1"},
            {"author_id": "search.input", "reference": "r2"},
            {"author_id": "search.input", "reference": "r3"},
        ]});
        assert_eq!(
            node_reference(&snapshot, "search.input").as_deref(),
            Some("r2")
        );
        assert_eq!(node_reference(&snapshot, "missing"), None);
    }
}
