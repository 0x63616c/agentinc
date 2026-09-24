//! Official Codex stdio client. Codex owns OAuth and its credential store.
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

pub fn home() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AGENTINC_CODEX_HOME") {
        return Ok(path.into());
    }
    Ok(
        PathBuf::from(std::env::var_os("HOME").context("Home directory unavailable")?)
            .join("Library/Application Support/Agentinc OS/codex"),
    )
}
fn executable() -> PathBuf {
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
pub struct Client {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Result<Value, String>>,
    next_id: u64,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    pub fn start() -> Result<Self> {
        Self::start_at(&home()?)
    }
    pub(crate) fn start_at(home: &Path) -> Result<Self> {
        std::fs::create_dir_all(home)?;
        let child = Command::new(executable())
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
            .spawn()
            .context("Install the Codex CLI to connect ChatGPT, then refresh.")?;
        Self::from_child(child)
    }
    fn from_child(mut child: Child) -> Result<Self> {
        let stdin = child.stdin.take().context("Codex input unavailable")?;
        let stdout = child.stdout.take().context("Codex output unavailable")?;
        let (tx, messages) = mpsc::sync_channel(256);
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
                            let _ = tx.send(Err("Codex response was too large".into()));
                            break;
                        }
                        if tx
                            .send(
                                serde_json::from_str(&line)
                                    .map_err(|_| "Codex sent an unreadable response".into()),
                            )
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            messages,
            next_id: 0,
        };
        client.call("initialize", json!({"clientInfo":{"name":"agentinc_os","title":"AgentInc","version":env!("CARGO_PKG_VERSION")}}))?;
        client.write(json!({"method":"initialized","params":{}}))?;
        Ok(client)
    }
    fn write(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, &value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }
    pub fn next(&mut self, deadline: Instant) -> Result<Value> {
        let message = self
            .messages
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("Codex stopped responding. Refresh or retry.")?
            .map_err(anyhow::Error::msg)?;
        if message.get("method").is_some() && message.get("id").is_some() {
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
    pub fn account(&mut self) -> Result<Option<String>> {
        let response = self.call("account/read", json!({"refreshToken":false}))?;
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
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Model {
    pub id: String,
    pub name: String,
}
pub fn status() -> Result<(Option<String>, Vec<Model>)> {
    let mut client = Client::start()?;
    let account = client.account()?;
    let mut models = Vec::new();
    if account.is_some() {
        let mut cursor = Value::Null;
        loop {
            let result = client.call("model/list", json!({"limit":100,"cursor":cursor}))?;
            if let Some(data) = result["data"].as_array() {
                for model in data {
                    if let (Some(id), Some(name)) =
                        (model["model"].as_str(), model["displayName"].as_str())
                    {
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
    }
    Ok((account, models))
}
pub fn login(cancel: Arc<AtomicBool>, open: impl FnOnce(String)) -> Result<()> {
    let mut client = Client::start()?;
    let result = client.call("account/login/start", json!({"type":"chatgpt"}))?;
    let url = result["authUrl"]
        .as_str()
        .context("Codex did not return a sign-in link")?;
    if !url.starts_with("https://") {
        bail!("Codex returned an invalid sign-in link");
    }
    open(url.into());
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let _ = client.call("account/login/cancel", json!({"loginId":result["loginId"]}));
            // A completed callback can race cancellation; re-read Codex's authoritative state.
            if client.account()?.is_some() {
                return Ok(());
            }
            bail!("Sign-in cancelled. You can try again when ready.");
        }
        let event = match client.messages.recv_timeout(Duration::from_millis(250)) {
            Ok(Ok(event)) => event,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            _ => bail!("Codex sign-in stopped. Try again."),
        };
        if event["method"] == "account/login/completed" {
            if event["params"]["success"] == true {
                return Ok(());
            }
            bail!("Sign-in was not completed. Try again.");
        }
    }
}
pub fn logout() -> Result<()> {
    Client::start()?.call("account/logout", json!({}))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdio_handshake_account_events_and_safe_errors() -> Result<()> {
        let script = r#"
read -r init
case "$init" in *'"method":"initialize"'*) ;; *) exit 1;; esac
printf '%s\n' '{"id":1,"result":{}}'
read -r initialized
case "$initialized" in *'"method":"initialized"'*) ;; *) exit 1;; esac
read -r account
printf '%s\n' '{"method":"account/updated","params":{}}' '{"id":2,"result":{"account":{"type":"chatgpt","email":"fixture@example.test","planType":"test"}}}'
read -r fail
printf '%s\n' '{"id":3,"error":{"message":"private backend detail"}}'
"#;
        let child = Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut client = Client::from_child(child)?;
        assert_eq!(
            client.account()?.as_deref(),
            Some("fixture@example.test · test")
        );
        let error = client
            .call("example/error", json!({}))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private backend detail"));
        assert!(error.contains("example/error"));
        Ok(())
    }
}
