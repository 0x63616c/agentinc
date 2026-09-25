//! Signed app + Pilot relaunch check against an isolated daemon.
#![cfg(target_os = "macos")]
use anyhow::{Context, Result, ensure};
use futures::{SinkExt, StreamExt};
use gpui_pilot::{protocol::Command, transport::Client};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command as Process, Stdio},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;

struct App(Child);
const WINDOW_TITLE: &str = "AgentInc Terminal Persistence Test";
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn launch(root: &Path, discovery: &Path, run: &str) -> Result<(App, Client)> {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("dist/AgentInc Dev.app/Contents/MacOS/AgentInc");
    ensure!(bundle.is_file(), "build the signed automation bundle first");
    let pilot = root.join(run);
    let log = fs::File::create(root.join(format!("{run}.log")))?;
    let mut command = Process::new(bundle);
    command.args(["--gpui-pilot-session", pilot.to_str().unwrap()]);
    if std::env::var_os("AINC_TERMINAL_PROOF_DIR").is_some() {
        command.arg("--gpui-pilot-visible");
    }
    let mut app = App(command
        .env("AGENTINC_SESSION_PATH", root.join("session.json"))
        .env("AINC_DISCOVERY_FILE", discovery)
        .env("AINC_LEGACY_DIR", root.join("legacy"))
        .env("AGENTINC_CODEX_HOME", root.join("codex"))
        .env("AGENTINC_WINDOW_TITLE", WINDOW_TITLE)
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()?);
    let manifest = pilot.join("instance.json");
    while !manifest.exists() {
        ensure!(
            app.0.try_wait()?.is_none(),
            "app exited before Pilot readiness"
        );
        std::thread::yield_now();
    }
    let mut client = Client::connect(&manifest)?;
    client.call(Command::Hello)?;
    client.call(Command::Press {
        key: "cmd-5".into(),
    })?;
    Ok((app, client))
}

fn capture(app: &App, path: &Path) -> Result<()> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/pilot_os_acceptance.swift");
    let result = Process::new("swift")
        .arg(script)
        .arg(app.0.id().to_string())
        .arg(WINDOW_TITLE)
        .output()?;
    ensure!(
        result.status.success(),
        "native window lookup: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let window: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    let number = window["window_number"]
        .as_u64()
        .context("native window number")?;
    for _ in 0..80 {
        let result = Process::new("screencapture")
            .args(["-x", "-l", &number.to_string()])
            .arg(path)
            .status()?;
        ensure!(
            result.success() && path.is_file(),
            "capture native terminal window"
        );
        let image = image::open(path)?.to_rgb8();
        // The right Ghostty pane is otherwise black below its initial login
        // banner. Text here proves the daemon's replay reached the native view.
        let ink = (300..700)
            .flat_map(|y| (1750..2800).map(move |x| (x, y)))
            .filter(|&(x, y)| image.get_pixel(x, y).0.iter().any(|&value| value > 120))
            .count();
        if ink > 200 {
            return Ok(());
        }
    }
    anyhow::bail!("daemon output never appeared in the native Ghostty pane")
}

fn attached(ids: &[Uuid]) -> Result<()> {
    for _ in 0..50 {
        let output = Process::new("ps")
            .args(["-ww", "-axo", "command="])
            .output()?;
        let processes = String::from_utf8(output.stdout)?;
        if ids
            .iter()
            .all(|id| processes.contains(&format!("--terminal-attach {id}")))
        {
            return Ok(());
        }
        std::thread::yield_now();
    }
    anyhow::bail!("Ghostty did not launch both daemon attach clients")
}

#[test]
fn split_session_survives_app_quit_and_relaunch() -> Result<()> {
    let source = PathBuf::from(
        std::env::var_os("AINC_DISCOVERY_FILE")
            .context("run against an isolated cargo xtask dev stack")?,
    );
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let discovery = root.join("api-url");
    fs::copy(&source, &discovery)?;
    fs::copy(
        source.with_file_name("owner-token"),
        root.join("owner-token"),
    )?;
    let url = fs::read_to_string(&discovery)?.trim().to_owned();
    let token = fs::read_to_string(root.join("owner-token"))?
        .trim()
        .to_owned();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let layout = root.join("terminal-layout.json");
    fs::write(
        &layout,
        serde_json::to_vec(&serde_json::json!({
            "tree": {"vertical": false, "first": {"id": first.to_string()}, "second": {"id": second.to_string()}},
            "zoomed": null
        }))?,
    )?;
    let expected = fs::read(&layout)?;
    let proof = std::env::var_os("AINC_TERMINAL_PROOF_DIR").map(PathBuf::from);
    if let Some(proof) = &proof {
        fs::create_dir_all(proof)?;
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime
        .block_on(async {
            let http = reqwest::Client::new();
            for id in [first, second] {
                http.post(format!("{url}/v1/terminal/sessions"))
                    .bearer_auth(&token)
                    .header("agent-inc-client", ainc_release::client_header())
                    .json(&serde_json::json!({"id": id.to_string()}))
                    .send()
                    .await?
                    .error_for_status()?;
            }
            let fifo = root.join("gate");
            let path = std::ffi::CString::new(fifo.to_string_lossy().as_bytes())?;
            ensure!(
                unsafe { libc::mkfifo(path.as_ptr(), 0o600) } == 0,
                "create FIFO gate"
            );
            let ws_url = url.replacen("http://", "ws://", 1);
            let mut request =
                format!("{ws_url}/v1/terminal/sessions/{second}/attach").into_client_request()?;
            request
                .headers_mut()
                .insert("authorization", format!("Bearer {token}").parse()?);
            request
                .headers_mut()
                .insert("agent-inc-client", ainc_release::client_header().parse()?);
            let (mut viewer, _) = connect_async(request.clone()).await?;
            let command = format!(
                "IFS= read -r line < {}; printf '__PERSISTED__:%s\\n' \"$line\"\n",
                fifo.display()
            );
            let mut input = vec![0];
            input.extend_from_slice(command.as_bytes());
            viewer.send(Message::Binary(input.into())).await?;
            let mut accepted = Vec::new();
            loop {
                if let Some(Ok(Message::Binary(bytes))) = viewer.next().await {
                    accepted.extend_from_slice(&bytes);
                    if String::from_utf8_lossy(&accepted).contains("__PERSISTED__") {
                        break;
                    }
                }
            }
            drop(viewer);
            Ok::<_, anyhow::Error>((http, request, fifo))
        })
        .and_then(|(http, request, fifo)| {
            let (app, _) = launch(&root, &discovery, "pilot-before")?;
            attached(&[first, second])?;
            ensure!(
                fs::read(&layout)? == expected,
                "app changed saved split layout"
            );
            if let Some(proof) = &proof {
                capture(&app, &proof.join("before.png"))?;
            }
            drop(app);
            runtime.block_on(async {
                tokio::task::spawn_blocking(move || -> Result<()> {
                    let mut gate = fs::OpenOptions::new().write(true).open(fifo)?;
                    gate.write_all(b"after-quit\n")?;
                    Ok(())
                })
                .await??;
                let (mut viewer, _) = connect_async(request).await?;
                let mut replay = Vec::new();
                loop {
                    if let Some(Ok(Message::Binary(bytes))) = viewer.next().await {
                        replay.extend_from_slice(&bytes);
                        if String::from_utf8_lossy(&replay).contains("__PERSISTED__:after-quit") {
                            break;
                        }
                    }
                }
                let sessions: serde_json::Value = http
                    .get(format!("{url}/v1/terminal/sessions"))
                    .bearer_auth(&token)
                    .header("agent-inc-client", ainc_release::client_header())
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                ensure!(
                    sessions.as_array().is_some_and(|items| {
                        items.iter().any(|item| {
                            item["id"] == second.to_string() && item["state"] == "running"
                        })
                    }),
                    "long-running session ended when app quit"
                );
                Ok::<_, anyhow::Error>(())
            })?;
            let (app, _) = launch(&root, &discovery, "pilot-after")?;
            attached(&[first, second])?;
            ensure!(
                fs::read(&layout)? == expected,
                "app did not restore the same split and zoom"
            );
            if let Some(proof) = &proof {
                capture(&app, &proof.join("after.png"))?;
            }
            drop(app);
            runtime.block_on(async {
                for id in [first, second] {
                    http.delete(format!("{url}/v1/terminal/sessions/{id}"))
                        .bearer_auth(&token)
                        .header("agent-inc-client", ainc_release::client_header())
                        .send()
                        .await?
                        .error_for_status()?;
                }
                Ok::<_, anyhow::Error>(())
            })?;
            Ok(())
        })
}
