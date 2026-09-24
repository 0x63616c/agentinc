//! Owned coding tools. Every subprocess is confined by the OS to an explicitly
//! configured directory, and every effect has a durable unknown/completed receipt.
use crate::tickets::Actor;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
#[cfg(target_os = "macos")]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use turnkeel::{Tool, ToolCtx, ToolError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadFile,
    WriteFile,
    Shell,
    Git,
}
#[derive(Clone)]
pub struct WorkspacePolicy {
    root: PathBuf,
    allowed: Vec<Permission>,
}
impl WorkspacePolicy {
    pub fn new(root: impl AsRef<Path>, allowed: Vec<Permission>) -> anyhow::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        anyhow::ensure!(
            root.is_dir() && root.parent().is_some(),
            "configure a workspace directory, not a filesystem root"
        );
        Ok(Self { root, allowed })
    }
    pub(crate) fn allows(&self, permission: Permission) -> bool {
        self.allowed.contains(&permission)
    }
    fn path(&self, value: &str) -> Result<PathBuf, ToolError> {
        let path = Path::new(value);
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(ToolError::InvalidArguments(
                "Use a relative path inside the configured workspace.".into(),
            ));
        }
        Ok(self.root.join(path))
    }
}

#[derive(Clone)]
pub(crate) struct CodingTool {
    pub pool: PgPool,
    pub actor: Actor,
    pub run_id: String,
    pub policy: Arc<WorkspacePolicy>,
    pub permission: Permission,
}
impl Tool for CodingTool {
    fn name(&self) -> &str {
        match self.permission {
            Permission::ReadFile => "read_file",
            Permission::WriteFile => "write_file",
            Permission::Shell => "shell",
            Permission::Git => "git",
        }
    }
    fn description(&self) -> &str {
        match self.permission {
            Permission::ReadFile => "Read a file in the configured workspace.",
            Permission::WriteFile => {
                "Replace a file in the configured workspace. Inspect any unknown prior outcome before retrying."
            }
            Permission::Shell => {
                "Run a shell command confined to the configured workspace, with no network access."
            }
            Permission::Git => {
                "Run git status, diff, log, add or commit inside the configured workspace."
            }
        }
    }
    fn schema(&self) -> Value {
        match self.permission {
            Permission::ReadFile => {
                json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false})
            }
            Permission::WriteFile => {
                json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false})
            }
            Permission::Shell => {
                json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false})
            }
            Permission::Git => {
                json!({"type":"object","properties":{"args":{"type":"array","items":{"type":"string"}}},"required":["args"],"additionalProperties":false})
            }
        }
    }
    // A crash between an arbitrary process effect and its receipt is unknown, never
    // permission to rerun the process. Explicit same-key retries reconcile below.
    fn idempotent(&self) -> bool {
        false
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        let tool = self.clone();
        Box::pin(async move { tool.execute(ctx, args).await })
    }
}
fn failed(error: impl std::fmt::Display) -> ToolError {
    ToolError::Failed(error.to_string())
}
fn invalid(message: &str) -> ToolError {
    ToolError::InvalidArguments(message.into())
}
impl CodingTool {
    async fn execute(&self, ctx: ToolCtx, args: Value) -> Result<Value, ToolError> {
        if !self.policy.allowed.contains(&self.permission) {
            return Err(invalid("This tool is not allowed by the workspace policy."));
        }
        let (program, arguments, input) = self.invocation(&args)?;
        let mut tx = self.pool.begin().await.map_err(failed)?;
        let (ticket, generation) = self
            .actor
            .assignment
            .ok_or_else(|| invalid("Coding requires an assigned Ticket."))?;
        let live:Option<i64>=sqlx::query_scalar("SELECT t.id FROM tickets t JOIN ticket_runs r ON r.ticket_id=t.id AND r.generation=t.generation WHERE t.id=$1 AND t.workspace_id=$2 AND t.generation=$3 AND t.assignee_id=$4 AND r.run_id=$5 AND r.state='running' AND t.status='in_progress' FOR UPDATE OF t")
            .bind(ticket).bind(&self.actor.workspace).bind(generation).bind(&self.actor.id).bind(&self.run_id).fetch_optional(&mut *tx).await.map_err(failed)?;
        if live.is_none() {
            return Err(invalid("The Ticket assignment is no longer active."));
        }
        let prior: Option<(String, Value, Option<Value>)> = sqlx::query_as(
            "SELECT tool,arguments,result FROM tool_effects WHERE run_id=$1 AND effect_key=$2",
        )
        .bind(&self.run_id)
        .bind(ctx.idempotency_key())
        .fetch_optional(&mut *tx)
        .await
        .map_err(failed)?;
        if let Some((tool, old, result)) = prior {
            if tool != self.name() || old != args {
                return Err(invalid("This effect key belongs to a different command."));
            }
            return result.ok_or_else(||invalid("The previous tool outcome is unknown. Inspect the workspace before taking another action."));
        }
        sqlx::query(
            "INSERT INTO tool_effects(run_id,effect_key,tool,arguments) VALUES ($1,$2,$3,$4)",
        )
        .bind(&self.run_id)
        .bind(ctx.idempotency_key())
        .bind(self.name())
        .bind(&args)
        .execute(&mut *tx)
        .await
        .map_err(failed)?;
        sqlx::query(
            "INSERT INTO comments(ticket_id,author_id,body,effect_key) VALUES ($1,$2,$3,$4)",
        )
        .bind(ticket)
        .bind(&self.actor.id)
        .bind(format!("Started {}: {}", self.name(), summary(&args)))
        .bind(format!("{}/started", ctx.idempotency_key()))
        .execute(&mut *tx)
        .await
        .map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        let result = run_process(&self.policy, &program, &arguments, input).await?;
        let mut tx = self.pool.begin().await.map_err(failed)?;
        sqlx::query("UPDATE tool_effects SET result=$3 WHERE run_id=$1 AND effect_key=$2 AND result IS NULL").bind(&self.run_id).bind(ctx.idempotency_key()).bind(&result).execute(&mut *tx).await.map_err(failed)?;
        // Serialize evidence projection with cancel/reassign, including a cancellation
        // that commits while the subprocess is finishing.
        sqlx::query("SELECT id FROM tickets WHERE id=$1 FOR UPDATE")
            .bind(ticket)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
        // Historical receipts survive reassignment; stale agents cannot project Comments.
        sqlx::query("INSERT INTO comments(ticket_id,author_id,body,effect_key) SELECT id,$2,$3,$4 FROM tickets WHERE id=$1 AND generation=$5 AND assignee_id=$2 AND status='in_progress' ON CONFLICT(effect_key) DO NOTHING").bind(ticket).bind(&self.actor.id).bind(format!("{} result: {}",self.name(),result).chars().take(32000).collect::<String>()).bind(ctx.idempotency_key()).bind(generation).execute(&mut *tx).await.map_err(failed)?;
        tx.commit().await.map_err(failed)?;
        Ok(result)
    }
    fn invocation(&self, args: &Value) -> Result<(String, Vec<String>, Option<String>), ToolError> {
        let string = |key| {
            args[key]
                .as_str()
                .ok_or_else(|| invalid("Missing text argument."))
        };
        Ok(match self.permission {
            Permission::ReadFile => (
                "/bin/cat".into(),
                vec![
                    self.policy
                        .path(string("path")?)?
                        .to_string_lossy()
                        .into_owned(),
                ],
                None,
            ),
            Permission::WriteFile => (
                "/usr/bin/tee".into(),
                vec![
                    self.policy
                        .path(string("path")?)?
                        .to_string_lossy()
                        .into_owned(),
                ],
                Some(string("content")?.into()),
            ),
            Permission::Shell => (
                "/bin/sh".into(),
                vec!["-c".into(), string("command")?.into()],
                None,
            ),
            Permission::Git => {
                let args: Vec<String> = serde_json::from_value(args["args"].clone())
                    .map_err(|_| invalid("Supply git arguments as strings."))?;
                if !args.first().is_some_and(|a| {
                    matches!(a.as_str(), "status" | "diff" | "log" | "add" | "commit")
                }) {
                    return Err(invalid(
                        "Git permits status, diff, log, add and commit only.",
                    ));
                }
                ("/usr/bin/git".into(), args, None)
            }
        })
    }
}
fn summary(args: &Value) -> String {
    let mut safe = args.clone();
    if let Some(content) = safe.get_mut("content") {
        *content = json!("[file contents]");
    }
    safe.to_string().chars().take(2000).collect()
}

#[cfg(target_os = "macos")]
struct ChildGroup {
    child: tokio::process::Child,
    id: Option<i32>,
}
#[cfg(target_os = "macos")]
impl Drop for ChildGroup {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            // SAFETY: this is the dedicated process group created for this child;
            // kill takes no pointers. Terminate remaining group members on any exit.
            unsafe {
                libc::kill(-id, libc::SIGKILL);
            }
        }
    }
}

async fn run_process(
    policy: &WorkspacePolicy,
    program: &str,
    args: &[String],
    input: Option<String>,
) -> Result<Value, ToolError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (policy, program, args, input);
        Err(invalid(
            "This worker has no supported OS coding sandbox. Use the Mac worker.",
        ))
    }
    #[cfg(target_os = "macos")]
    {
        use std::{os::unix::process::CommandExt, process::Stdio};
        // Scheme string escaping prevents a workspace name from changing the policy.
        let root = serde_json::to_string(&policy.root.to_string_lossy()).map_err(failed)?;
        let profile = format!(
            "(version 1)(deny default)(allow process*)(allow signal (target self))(allow sysctl-read)(allow mach-lookup)(allow file-read* file-map-executable (literal \"/\") (literal \"/private/var/select/sh\") (subpath \"/System\") (subpath \"/usr\") (subpath \"/bin\") (subpath \"/sbin\") (subpath \"/Library/Developer\") (literal \"/dev/null\") (literal \"/dev/urandom\"))(allow file-read-metadata (path-ancestors {root}))(allow file-read* file-write* (subpath {root}))"
        );
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args(["-p", &profile, program])
            .args(args)
            .current_dir(&policy.root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &policy.root)
            .env("TMPDIR", &policy.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command.as_std_mut().process_group(0);
        let child = command.spawn().map_err(failed)?;
        let id = child.id().and_then(|id| i32::try_from(id).ok());
        let mut child = ChildGroup { child, id };
        let mut stdin = child
            .child
            .stdin
            .take()
            .ok_or_else(|| failed("tool stdin unavailable"))?;
        let stdout = child
            .child
            .stdout
            .take()
            .ok_or_else(|| failed("tool stdout unavailable"))?;
        let stderr = child
            .child
            .stderr
            .take()
            .ok_or_else(|| failed("tool stderr unavailable"))?;
        let write = async move {
            if let Some(input) = input {
                stdin.write_all(input.as_bytes()).await?;
            }
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let read = |stream: Box<dyn tokio::io::AsyncRead + Unpin + Send>| async move {
            let mut bytes = Vec::new();
            stream.take(1024 * 1024 + 1).read_to_end(&mut bytes).await?;
            if bytes.len() > 1024 * 1024 {
                return Err(std::io::Error::other("tool output exceeded 1 MiB"));
            }
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&bytes).into_owned())
        };
        let (_, stdout, stderr, status) = tokio::try_join!(
            write,
            read(Box::new(stdout)),
            read(Box::new(stderr)),
            child.child.wait()
        )
        .map_err(failed)?;
        Ok(json!({"exit_code":status.code(),"stdout":stdout,"stderr":stderr}))
    }
}

#[derive(Clone)]
pub(crate) struct CommentTool {
    pub pool: PgPool,
    pub actor: Actor,
}
impl Tool for CommentTool {
    fn name(&self) -> &str {
        "comment"
    }
    fn description(&self) -> &str {
        "Add a Comment to this Ticket's work log."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","properties":{"body":{"type":"string"}},"required":["body"],"additionalProperties":false})
    }
    fn call(&self, ctx: ToolCtx, args: Value) -> BoxFuture<'static, Result<Value, ToolError>> {
        use sha2::{Digest, Sha256};
        let tool = self.clone();
        Box::pin(async move {
            let body = args["body"]
                .as_str()
                .ok_or_else(|| invalid("Supply Comment text."))?;
            let (ticket_id, _) = tool
                .actor
                .assignment
                .ok_or_else(|| invalid("Comments require an assignment."))?;
            let digest = Sha256::digest(ctx.idempotency_key().as_bytes());
            let id = uuid::Uuid::from_bytes(digest[..16].try_into().expect("SHA-256 has 32 bytes"));
            let receipt = crate::tickets::execute(
                &tool.pool,
                &tool.actor,
                crate::tickets::TicketCommandRequest {
                    operation_id: id.to_string(),
                    command: crate::tickets::TicketCommand::AddComment {
                        ticket_id,
                        body: body.into(),
                    },
                },
            )
            .await
            .map_err(|_| {
                invalid("Comment refused: the assignment changed or the text is invalid.")
            })?;
            Ok(json!({"comment_id":receipt.result_id}))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_reject_traversal_and_absolute_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let policy = WorkspacePolicy::new(dir.path(), vec![]).unwrap();
        for path in ["../outside", "/tmp/outside", "a/../../outside", ""] {
            assert!(policy.path(path).is_err());
        }
        assert!(
            policy
                .path("src/main.rs")
                .unwrap()
                .starts_with(dir.path().canonicalize().unwrap())
        );
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn kernel_sandbox_confines_shell_and_symlink_effects() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        let outside = dir.path().join("outside");
        std::fs::write(&outside, "private fixture").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        let policy = WorkspacePolicy::new(root, vec![Permission::Shell]).unwrap();
        // Use owned argument vectors so each future borrows until completion.
        let args = vec![
            "-c".into(),
            "printf evidence > result.txt; cat result.txt".into(),
        ];
        let result = run_process(&policy, "/bin/sh", &args, None).await.unwrap();
        assert_eq!(result["exit_code"], 0, "{result}");
        assert_eq!(result["stdout"], "evidence");
        for command in ["cat escape", "printf overwritten > escape"] {
            let args = vec!["-c".into(), command.into()];
            let result = run_process(&policy, "/bin/sh", &args, None).await.unwrap();
            assert_ne!(result["exit_code"], 0, "{result}");
        }
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "private fixture");
    }
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn unsupported_workers_never_fall_back_to_unsandboxed_shell() {
        let dir = tempfile::tempdir().unwrap();
        let policy = WorkspacePolicy::new(dir.path(), vec![Permission::Shell]).unwrap();
        assert!(
            run_process(
                &policy,
                "/bin/sh",
                &["-c".into(), "touch escaped".into()],
                None
            )
            .await
            .is_err()
        );
        assert!(!dir.path().join("escaped").exists());
    }
}
