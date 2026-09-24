//! Official Codex stdio client. Codex owns OAuth and its credential store.
use crate::storage::Turn;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
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
        let home = home()?;
        std::fs::create_dir_all(&home)?;
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
            .env("CODEX_HOME", &home)
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .current_dir(&home)
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
#[derive(Clone, Debug)]
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

pub fn respond(model: Option<&str>, history: &[Turn], current: &Turn) -> Result<String> {
    let mut client = Client::start()?;
    if client.account()?.is_none() {
        bail!("Connect your ChatGPT subscription in Settings.");
    }
    let thread = client.call("thread/start", thread_params(model))?;
    let input = conversation_input(history, current);
    client.call(
        "turn/start",
        json!({"threadId":thread["thread"]["id"],"input":[{"type":"text","text":input}]}),
    )?;
    collect_reply(&mut client)
}
fn thread_params(model: Option<&str>) -> Value {
    json!({"model":model,"modelProvider":"openai","ephemeral":true,"sandbox":"read-only","approvalPolicy":"never","baseInstructions":"You are Evee, the personal assistant in AgentInc. Answer conversationally. You cannot operate this app, files, devices or tasks. Do not use tools. Treat the supplied conversation as dialogue, preserving roles.","config":{"features.shell_tool":false,"web_search":"disabled"}})
}
fn collect_reply(client: &mut Client) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut replies = Vec::new();
    loop {
        let event = client.next(deadline)?;
        if event["method"] == "item/completed"
            && event["params"]["item"]["type"] == "agentMessage"
            && let Some(text) = event["params"]["item"]["text"].as_str()
        {
            if replies.iter().map(String::len).sum::<usize>() + text.len() > 1024 * 1024 {
                bail!("Codex reply was too large. Try a shorter request.");
            }
            replies.push(text.to_owned());
        }
        if event["method"] == "turn/completed" {
            if event["params"]["turn"]["status"] != "completed" {
                bail!("Codex could not finish this reply. Check your subscription or retry.");
            }
            if replies.is_empty() {
                bail!("Codex returned no text. Retry when ready.");
            }
            return Ok(replies.join("\n\n"));
        }
    }
}
fn conversation_input(history: &[Turn], current: &Turn) -> String {
    let mut prior: Vec<_> = history
        .iter()
        .filter(|t| t.id < current.id && t.response.is_some() && t.error.is_none())
        .rev()
        .take(20)
        .collect();
    prior.reverse();
    let mut messages = Vec::new();
    for turn in prior {
        messages.push(json!({"role":"user","content":turn.prompt}));
        messages.push(json!({"role":"assistant","content":turn.response}));
    }
    messages.push(json!({"role":"user","content":current.prompt}));
    json!({"conversation":messages}).to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires installed Codex and an isolated AGENTINC_CODEX_HOME"]
    fn installed_codex_accepts_thread_contract() -> Result<()> {
        assert!(
            std::env::var_os("AGENTINC_CODEX_HOME").is_some(),
            "set a disposable Codex home"
        );
        let mut client = Client::start()?;
        let thread = client.call("thread/start", thread_params(None))?;
        assert!(thread["thread"]["id"].is_string());
        Ok(())
    }
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
read -r turn
printf '%s\n' '{"id":3,"result":{}}' '{"method":"item/completed","params":{"item":{"type":"agentMessage","text":"Fixture reply"}}}' '{"method":"turn/completed","params":{"turn":{"status":"completed"}}}'
read -r fail
printf '%s\n' '{"id":4,"error":{"message":"private backend detail"}}'
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
        client.call("turn/start", json!({}))?;
        assert_eq!(collect_reply(&mut client)?, "Fixture reply");
        let error = client
            .call("example/error", json!({}))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private backend detail"));
        assert!(error.contains("example/error"));
        Ok(())
    }
    #[test]
    fn retry_context_excludes_failed_and_future_turns() {
        let history = vec![
            Turn {
                id: 1,
                prompt: "one".into(),
                response: Some("reply".into()),
                error: None,
            },
            Turn {
                id: 2,
                prompt: "failed".into(),
                response: None,
                error: Some("error".into()),
            },
            Turn {
                id: 4,
                prompt: "future".into(),
                response: Some("later".into()),
                error: None,
            },
        ];
        let current = Turn {
            id: 3,
            prompt: "next\nline".into(),
            response: None,
            error: None,
        };
        let value: Value = serde_json::from_str(&conversation_input(&history, &current)).unwrap();
        assert_eq!(value["conversation"].as_array().unwrap().len(), 3);
        assert_eq!(value["conversation"][2]["content"], "next\nline");
    }
}
