//! Claude through the user's own Claude subscription: every model step is one
//! `claude -p` invocation of the official Claude Code CLI, signed in by the
//! user. The daemon never reads, copies or replays Anthropic credentials.
use super::{DeltaSink, ProviderModel};
use anyhow::{Context, Result};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use turnkeel::{Content, Model, ModelError, ModelRequest, ModelResponse, Role, StopReason};

pub const MODELS: &[(&str, &str)] = &[
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-haiku-4-5-20251001", "Haiku 4.5"),
];
const STEP_TIMEOUT: Duration = Duration::from_secs(600);

pub fn models() -> Vec<ProviderModel> {
    MODELS
        .iter()
        .enumerate()
        .map(|(index, (id, name))| ProviderModel {
            id: (*id).into(),
            name: (*name).into(),
            featured: index == 0,
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub logged_in: bool,
    pub email: Option<String>,
    pub subscription: Option<String>,
}
impl Status {
    pub fn account(&self) -> Option<String> {
        if !self.logged_in {
            return None;
        }
        Some(match (&self.email, &self.subscription) {
            (Some(email), Some(plan)) => format!("{email} · Claude {}", capitalize(plan)),
            (Some(email), None) => email.clone(),
            (None, Some(plan)) => format!("Claude {}", capitalize(plan)),
            (None, None) => "Signed in".into(),
        })
    }
}
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub struct ClaudeCli {
    executable: PathBuf,
    cwd: PathBuf,
}
impl ClaudeCli {
    pub fn local() -> Self {
        let cwd = ainc_release::identity::support_dir().join("claude");
        Self::new(executable(), cwd)
    }
    pub fn new(executable: PathBuf, cwd: PathBuf) -> Self {
        Self { executable, cwd }
    }
    fn command(&self) -> Result<tokio::process::Command> {
        std::fs::create_dir_all(&self.cwd)?;
        let mut command = tokio::process::Command::new(&self.executable);
        command
            .current_dir(&self.cwd)
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        Ok(command)
    }
    /// Read sign-in state from the CLI without spending any usage.
    pub async fn status(&self) -> Result<Status> {
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            self.command()?.args(["auth", "status"]).output(),
        )
        .await
        .context("Claude Code did not answer. Install Claude Code, then refresh.")?
        .context("Install Claude Code to connect Claude, then refresh.")?;
        let value: Value = serde_json::from_slice(&output.stdout)
            .context("Claude Code returned an unreadable status. Update Claude Code.")?;
        Ok(Status {
            logged_in: value["loggedIn"] == true,
            email: value["email"].as_str().map(str::to_owned),
            subscription: value["subscriptionType"].as_str().map(str::to_owned),
        })
    }
    /// Start the official browser sign-in. The CLI opens the browser itself and
    /// waits for the code the user copies back; `open` receives the URL for the UI.
    pub async fn login(&self, open: impl FnOnce(String)) -> Result<Login> {
        let mut child = self
            .command()?
            .args(["auth", "login", "--claudeai"])
            .spawn()
            .context("Install Claude Code to connect Claude.")?;
        let stdin = child
            .stdin
            .take()
            .context("Claude Code input unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("Claude Code output unavailable")?;
        let mut lines = BufReader::new(stdout).lines();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        let mut url = None;
        loop {
            let line = match tokio::time::timeout_at(deadline, lines.next_line()).await {
                Ok(Ok(Some(line))) => line,
                _ => break,
            };
            if let Some(found) = find_url(&line) {
                url = Some(found);
                break;
            }
        }
        let url = url.context(
            "Claude Code did not offer a sign-in link. Run `claude` in Terminal to sign in.",
        )?;
        open(url);
        Ok(Login { child, stdin })
    }
    pub fn model(
        self: &Arc<Self>,
        id: &str,
        sink: Option<DeltaSink>,
    ) -> Result<ClaudeModel, ModelError> {
        if id.trim().is_empty() {
            return Err(ModelError::fatal("Select a model before starting work."));
        }
        Ok(ClaudeModel {
            model: id.into(),
            cli: self.clone(),
            sink,
        })
    }
}
/// An in-progress browser sign-in waiting for its pasted code.
pub struct Login {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
}
impl Login {
    /// Hand the copied code to the CLI and wait for it to finish.
    pub async fn submit(mut self, code: &str) -> Result<()> {
        let code = code.trim();
        anyhow::ensure!(
            !code.is_empty() && code.len() < 512 && !code.contains(['\n', '\r']),
            "Paste the code shown after signing in."
        );
        self.stdin.write_all(code.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        drop(self.stdin);
        let status = tokio::time::timeout(Duration::from_secs(60), self.child.wait())
            .await
            .context("Claude Code did not finish signing in.")??;
        anyhow::ensure!(status.success(), "Sign-in was not completed. Try again.");
        Ok(())
    }
    pub async fn cancel(mut self) {
        let _ = self.child.kill().await;
    }
}
fn find_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\u{1b}' || c == '\u{7}')
        .unwrap_or(rest.len());
    let url = &rest[..end];
    // OSC 8 hyperlinks repeat the URL; keep the first copy.
    let url = match url[8..].find("https://") {
        Some(again) => &url[..again + 8],
        None => url,
    };
    Some(url.to_owned())
}
fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("AGENTINC_CLAUDE_PATH") {
        return path.into();
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    for path in [
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ] {
        if path.is_file() {
            return path;
        }
    }
    "claude".into()
}

#[derive(Clone)]
pub struct ClaudeModel {
    model: String,
    cli: Arc<ClaudeCli>,
    sink: Option<DeltaSink>,
}
impl Model for ClaudeModel {
    fn id(&self) -> &str {
        &self.model
    }
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        let this = self.clone();
        Box::pin(async move { this.step(request).await })
    }
}
const SCHEMA: &str = r#"{"type":"object","properties":{"reply":{"type":"string","description":"Your message to the user. Empty when the step is only tool calls."},"tool_calls":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"input":{"type":"object"}},"required":["name","input"],"additionalProperties":false}}},"required":["reply","tool_calls"],"additionalProperties":false}"#;

impl ClaudeModel {
    async fn step(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let (system, prompt) = render(&request);
        let mut child = self
            .cli
            .command()
            .map_err(|_| ModelError::fatal("Claude Code workspace is unavailable."))?
            .args([
                "-p",
                "--output-format",
                "json",
                "--tools",
                "",
                "--strict-mcp-config",
                "--disable-slash-commands",
                "--no-session-persistence",
                "--permission-prompts",
                "none",
                "--setting-sources",
                "user",
                "--model",
                &self.model,
                "--system-prompt",
                &system,
                "--json-schema",
                SCHEMA,
            ])
            .spawn()
            .map_err(|_| ModelError::fatal("Install Claude Code to use Claude, then retry."))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ModelError::fatal("Claude Code input unavailable."))?;
        let write = async {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
            drop(stdin);
            std::io::Result::Ok(())
        };
        let output = async {
            let (_, output) = tokio::join!(write, child.wait_with_output());
            output
        };
        let output = tokio::time::timeout(STEP_TIMEOUT, output)
            .await
            .map_err(|_| ModelError::retryable("Claude did not answer in time."))?
            .map_err(|_| ModelError::retryable("Claude Code stopped unexpectedly."))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let value: Value = match serde_json::from_str(stdout.trim()) {
            Ok(value) => value,
            Err(_) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let detail = stderr
                    .lines()
                    .chain(stdout.lines())
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim();
                return Err(
                    if detail.to_ascii_lowercase().contains("log")
                        && detail.to_ascii_lowercase().contains("in")
                    {
                        ModelError::fatal("Sign in to Claude in Settings.")
                    } else if output.status.success() {
                        ModelError::fatal("Claude Code returned an unreadable reply.")
                    } else {
                        ModelError::retryable(format!("Claude Code failed: {}", sanitize(detail)))
                    },
                );
            }
        };
        if value["is_error"] == true
            || value["subtype"]
                .as_str()
                .is_some_and(|s| s.starts_with("error"))
        {
            let detail = value["result"]
                .as_str()
                .or_else(|| value["error"].as_str())
                .unwrap_or("");
            let lower = detail.to_ascii_lowercase();
            return Err(
                if lower.contains("log in")
                    || lower.contains("login")
                    || lower.contains("authentication")
                {
                    ModelError::fatal("Sign in to Claude in Settings.")
                } else if lower.contains("rate")
                    || lower.contains("overloaded")
                    || lower.contains("limit")
                {
                    ModelError::retryable(format!("Claude is busy: {}", sanitize(detail)))
                } else {
                    ModelError::fatal(format!("Claude could not finish: {}", sanitize(detail)))
                },
            );
        }
        let structured = match value.get("structured_output") {
            Some(structured) if structured.is_object() => structured.clone(),
            _ => serde_json::from_str(value["result"].as_str().unwrap_or(""))
                .map_err(|_| ModelError::fatal("Claude returned an unreadable reply."))?,
        };
        let reply = structured["reply"].as_str().unwrap_or("").to_owned();
        let mut content = Vec::new();
        if !reply.trim().is_empty() {
            if let Some(sink) = &self.sink {
                sink(&reply);
            }
            content.push(Content::Text { text: reply });
        }
        for call in structured["tool_calls"].as_array().into_iter().flatten() {
            let name = call["name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .ok_or_else(|| ModelError::fatal("Claude tool call is incomplete."))?;
            let input = call.get("input").cloned().unwrap_or_else(|| json!({}));
            if !input.is_object() {
                return Err(ModelError::fatal("Claude tool arguments are invalid."));
            }
            content.push(Content::ToolUse {
                id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                name: name.into(),
                input,
            });
        }
        let stop_reason = if content.iter().any(|c| matches!(c, Content::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        if content.is_empty() {
            return Err(ModelError::fatal("Claude returned no reply or tool call."));
        }
        Ok(ModelResponse {
            content,
            stop_reason,
        })
    }
}
fn sanitize(detail: &str) -> String {
    let trimmed: String = detail.chars().take(200).collect();
    trimmed.replace(|c: char| c.is_control(), " ")
}

/// The transcript as Claude Code sees it: instructions and tool contract as the
/// system prompt, the dialogue as one prompt on stdin.
fn render(request: &ModelRequest) -> (String, String) {
    let mut system = request.instructions.clone();
    if request.tools.is_empty() {
        system.push_str("\n\nAnswer in the required JSON structure: put your message in `reply` and leave `tool_calls` empty.");
    } else {
        system.push_str("\n\n# Tools\nYou can call tools by listing them in `tool_calls`; each call runs and its result comes back as the next message. Call at most one tool per step. Available tools:\n");
        for tool in &request.tools {
            system.push_str(&format!(
                "\n- `{}`: {}\n  input schema: {}\n",
                tool.name, tool.description, tool.input_schema
            ));
        }
        system.push_str("\nAnswer in the required JSON structure: `reply` holds your message to the user (empty when you only call tools) and `tool_calls` holds the calls to make now.");
    }
    let mut prompt = String::from("Conversation so far:\n");
    for message in &request.messages {
        for block in &message.content {
            match (message.role, block) {
                (Role::User, Content::Text { text }) => prompt.push_str(&format!("\n<user>\n{text}\n</user>\n")),
                (Role::Assistant, Content::Text { text }) => prompt.push_str(&format!("\n<assistant>\n{text}\n</assistant>\n")),
                (_, Content::ToolUse { id, name, input }) => prompt.push_str(&format!("\n<tool_call id=\"{id}\" name=\"{name}\">\n{input}\n</tool_call>\n")),
                (_, Content::ToolResult { tool_use_id, content, is_error }) => prompt.push_str(&format!("\n<tool_result id=\"{tool_use_id}\" error=\"{is_error}\">\n{content}\n</tool_result>\n")),
                (_, Content::ModelContext { .. }) => {}
            }
        }
    }
    prompt.push_str("\nContinue as the assistant.");
    (system, prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use turnkeel::{Agent, Message, Runtime, tool};

    fn fake_cli(dir: &std::path::Path, script: &str) -> Arc<ClaudeCli> {
        let path = dir.join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{script}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Arc::new(ClaudeCli::new(path, dir.join("cwd")))
    }
    #[tool]
    async fn fixture_echo(value: String) -> anyhow::Result<String> {
        Ok(value)
    }

    #[tokio::test]
    async fn structured_steps_drive_our_tool_loop() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let log = dir.path().join("calls.log");
        let script = format!(
            r#"
if [ "$1" = "auth" ]; then printf '%s' '{{"loggedIn":true,"email":"fixture@example.test","subscriptionType":"max"}}'; exit 0; fi
printf '%s\n' "$*" >> '{log}'
prompt=$(cat)
printf '%s\n---\n' "$prompt" >> '{log}'
case "$prompt" in
  *tool_result*) printf '%s' '{{"type":"result","is_error":false,"structured_output":{{"reply":"Finished with tool evidence","tool_calls":[]}}}}';;
  *) printf '%s' '{{"type":"result","is_error":false,"structured_output":{{"reply":"","tool_calls":[{{"name":"fixture_echo","input":{{"value":"tool evidence"}}}}]}}}}';;
esac
"#,
            log = log.display()
        );
        let cli = fake_cli(dir.path(), &script);
        assert_eq!(
            cli.status().await?.account().as_deref(),
            Some("fixture@example.test · Claude Max")
        );
        let seen = Arc::new(std::sync::Mutex::new(String::new()));
        let target = seen.clone();
        let sink: DeltaSink = Arc::new(move |delta: &str| target.lock().unwrap().push_str(delta));
        let agent = Agent::builder("claude-fixture-v1")
            .model(cli.model("claude-sonnet-5", Some(sink))?)
            .instructions("Fixture instructions")
            .tool(fixture_echo)
            .build();
        let runtime = Runtime::test().await?;
        let run = runtime.start(&agent, "use the fixture").await?;
        assert_eq!(run.result().await?, "Finished with tool evidence");
        runtime.shutdown().await?;
        let log = std::fs::read_to_string(log)?;
        assert!(log.contains("--tools  --strict-mcp-config"));
        assert!(log.contains("--model claude-sonnet-5"));
        assert!(log.contains("--json-schema"));
        assert!(log.contains("Fixture instructions"));
        assert!(log.contains("<tool_call id=\"call_"));
        assert!(log.contains("<tool_result id=\"call_"));
        assert_eq!(seen.lock().unwrap().as_str(), "Finished with tool evidence");
        Ok(())
    }

    #[tokio::test]
    async fn sign_in_and_failure_states_are_typed() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let cli = fake_cli(
            dir.path(),
            r#"
if [ "$1" = "auth" ]; then printf '%s' '{"loggedIn":false}'; exit 0; fi
cat >/dev/null
case "$AINC_FIXTURE" in
  denied) printf '%s' '{"type":"result","is_error":true,"subtype":"error","result":"Not logged in. Please run /login"}';;
  busy) printf '%s' '{"type":"result","is_error":true,"result":"Rate limit reached"}';;
  garbage) echo 'unexpected'; exit 1;;
esac
"#,
        );
        assert_eq!(cli.status().await?.account(), None);
        let request = || ModelRequest {
            instructions: "t".into(),
            messages: vec![Message::user("hi")],
            tools: vec![],
        };
        for (fixture, retryable, needle) in [
            ("denied", false, "Sign in"),
            ("busy", true, "busy"),
            ("garbage", true, "failed"),
        ] {
            // SAFETY: tests in this module run serially on this variable.
            unsafe { std::env::set_var("AINC_FIXTURE", fixture) };
            let error = cli
                .model("claude-sonnet-5", None)?
                .step(request())
                .await
                .unwrap_err();
            assert_eq!(error.retryable, retryable, "{fixture}");
            assert!(
                error.message.contains(needle),
                "{fixture}: {}",
                error.message
            );
        }
        unsafe { std::env::remove_var("AINC_FIXTURE") };
        Ok(())
    }

    #[test]
    fn sign_in_links_are_extracted_from_hyperlinked_output() {
        let line = "If the browser didn't open, visit: \u{1b}]8;;https://claude.com/x?a=1\u{1b}\\https://claude.com/x?a=1\u{1b}]8;;\u{1b}\\";
        assert_eq!(find_url(line).as_deref(), Some("https://claude.com/x?a=1"));
        assert_eq!(
            find_url("visit https://a.test/b now").as_deref(),
            Some("https://a.test/b")
        );
        assert_eq!(find_url("nothing"), None);
    }
}
