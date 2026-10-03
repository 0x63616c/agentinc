//! Official Codex stdio client. Codex owns OAuth and its credential store; the
//! daemon owns the process. One `Client` is one `codex app-server` child.
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

pub fn home() -> PathBuf {
    if let Some(path) = std::env::var_os("AGENTINC_CODEX_HOME") {
        return path.into();
    }
    ainc_release::identity::support_dir().join("codex")
}
pub fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("AGENTINC_CODEX_PATH") {
        return path.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        let bundled = exe.with_file_name("../Resources/runtime/codex");
        if bundled.is_file() {
            return bundled;
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    for path in [
        home.join(".local/bin/codex"),
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ] {
        if path.is_file() {
            return path;
        }
    }
    "codex".into()
}

/// Notifications Codex sends without being asked (sign-in completion, account
/// changes). Shared so a sign-in can wait on them without holding the client.
pub type Events = Arc<Mutex<Receiver<Value>>>;

pub struct Client {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    events: Events,
    next_id: u64,
}
impl Drop for Client {
    fn drop(&mut self) {
        // The child leads its own process group; take any helpers down with it.
        if let Ok(group) = i32::try_from(self.child.id()) {
            // SAFETY: killpg has no memory-safety preconditions; the group id is
            // the child's own pid, which we created with process_group(0).
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    pub fn start_at(home: &Path, executable: &Path) -> Result<Self> {
        std::fs::create_dir_all(home)?;
        let child = Command::new(executable)
            .args([
                "app-server",
                "--listen",
                "stdio://",
                "-c",
                "forced_login_method=\"chatgpt\"",
                "-c",
                "model_provider=\"openai\"",
                "-c",
                "features.shell_tool=false",
                "-c",
                "web_search=\"disabled\"",
            ])
            .env("CODEX_HOME", home)
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .current_dir(home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .context("Install the Codex CLI to connect ChatGPT, then refresh.")?;
        Self::from_child(child)
    }
    fn from_child(mut child: Child) -> Result<Self> {
        let stdin = child.stdin.take().context("Codex input unavailable")?;
        let stdout = child.stdout.take().context("Codex output unavailable")?;
        let (responses_tx, responses) = mpsc::sync_channel(256);
        let (events_tx, events) = mpsc::sync_channel(64);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                // Bound protocol frames, including malformed child output.
                let result = reader.by_ref().take(2 * 1024 * 1024).read_line(&mut line);
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if line.len() >= 2 * 1024 * 1024 {
                            let _ = responses_tx.send(Err("Codex response was too large".into()));
                            break;
                        }
                        let message: Result<Value, String> = serde_json::from_str(&line)
                            .map_err(|_| "Codex sent an unreadable response".into());
                        match message {
                            Ok(message) if message.get("id").is_none() => {
                                // Nobody listening is not an error; stale events are dropped.
                                let _ = events_tx.try_send(message);
                            }
                            message => {
                                if responses_tx.send(message).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            responses,
            events: Arc::new(Mutex::new(events)),
            next_id: 0,
        };
        client.call("initialize", json!({"clientInfo":{"name":"agentinc_os","title":"AgentInc","version":env!("CARGO_PKG_VERSION")}}))?;
        client.write(json!({"method":"initialized","params":{}}))?;
        Ok(client)
    }
    /// Whether the child is still running. A dead child is replaced by its owner.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub fn events(&self) -> Events {
        self.events.clone()
    }
    fn write(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, &value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }
    fn next(&mut self, deadline: Instant) -> Result<Value> {
        let message = self
            .responses
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("Codex stopped responding. Refresh or retry.")?
            .map_err(anyhow::Error::msg)?;
        if message.get("method").is_some() {
            // No approvals or externally supplied credentials are granted by this client.
            self.write(json!({"id":message["id"],"error":{"code":-32601,"message":"Unsupported client request"}}))?;
        }
        Ok(message)
    }
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.write(json!({"id":id,"method":method,"params":params}))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let message = self.next(deadline)?;
            if message["id"] == id {
                if message.get("error").is_some() {
                    bail!(
                        "Codex could not complete {method}. Check your connection and Codex version."
                    );
                }
                return Ok(message["result"].clone());
            }
        }
    }
    /// Who is signed in, as "email · plan", or `None`.
    pub fn signed_in_as(&mut self, refresh_token: bool) -> Result<Option<String>> {
        let response = self.call("account/read", json!({"refreshToken":refresh_token}))?;
        let account = &response["account"];
        if account["type"] == "chatgpt" {
            Ok(Some(format!(
                "{} · {}",
                account["email"].as_str().unwrap_or("ChatGPT"),
                account["planType"].as_str().unwrap_or("Connected")
            )))
        } else {
            Ok(None)
        }
    }
    /// Selectable models and the id Codex marks as the default.
    pub fn models(&mut self) -> Result<(Vec<Model>, Option<String>)> {
        let mut models = Vec::new();
        let mut default = None;
        let mut cursor = Value::Null;
        loop {
            let result = self.call("model/list", json!({"limit":100,"cursor":cursor}))?;
            if let Some(data) = result["data"].as_array() {
                for model in data {
                    if let (Some(id), Some(name)) =
                        (model["model"].as_str(), model["displayName"].as_str())
                    {
                        if model["isDefault"] == true {
                            default = Some(id.to_owned());
                        }
                        models.push(Model {
                            id: id.into(),
                            name: name.into(),
                        });
                    }
                }
            }
            cursor = result["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        Ok((models, default))
    }
    /// Begin a browser sign-in; returns the link to open and the sign-in id.
    pub fn login_start(&mut self) -> Result<(String, Value)> {
        let result = self.call("account/login/start", json!({"type":"chatgpt"}))?;
        let url = result["authUrl"]
            .as_str()
            .context("Codex did not return a sign-in link")?;
        if !url.starts_with("https://") {
            bail!("Codex returned an invalid sign-in link");
        }
        Ok((url.into(), result["loginId"].clone()))
    }
    pub fn login_cancel(&mut self, login_id: &Value) -> Result<()> {
        self.call("account/login/cancel", json!({"loginId":login_id}))?;
        Ok(())
    }
    pub fn logout(&mut self) -> Result<()> {
        self.call("account/logout", json!({}))?;
        Ok(())
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Model {
    pub id: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdio_handshake_identity_events_and_safe_errors() -> Result<()> {
        let home = tempfile::tempdir()?;
        std::fs::write(home.path().join("auth.json"), "{}")?;
        let mut client = Client::start_at(home.path(), &crate::testing::fake_codex::executable())?;
        assert_eq!(
            client.signed_in_as(false)?.as_deref(),
            Some("fixture@example.test · fixture")
        );
        let error = client
            .call("example/error", json!({}))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private backend detail"));
        assert!(error.contains("example/error"));
        assert!(client.alive());
        assert_eq!(crate::testing::fake_codex::spawns(home.path()), 1);
        Ok(())
    }
}
