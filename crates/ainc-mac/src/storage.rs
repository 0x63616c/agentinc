//! Generated daemon client plus an owned presentation snapshot. The foreground
//! reads this cache only; refresh and command methods run on the background executor.
pub use ainc_client::types::{Command, Conversation, Todo, Turn};
use ainc_client::{
    Client,
    types::{CommandRequest, Snapshot},
};
#[cfg(test)]
use anyhow::bail;
use anyhow::{Context, Result};
use std::{
    future::Future,
    os::unix::process::CommandExt,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

pub fn background<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Runtime::new().expect("HTTP runtime"))
        .block_on(future)
}
fn discovery_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AINC_DISCOVERY_FILE") {
        return Ok(path.into());
    }
    Ok(
        PathBuf::from(std::env::var_os("HOME").context("Home directory unavailable")?)
            .join("Library/Application Support/Agentinc OS/daemon/api-url"),
    )
}
pub async fn client() -> Result<Client> {
    let discovery = discovery_path()?;
    let configured = std::env::var("AINC_DAEMON_URL").ok();
    let url = if let Some(url) = configured {
        url
    } else {
        let existing = std::fs::read_to_string(&discovery).ok();
        let probe = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()?;
        let ready = if let Some(url) = &existing {
            probe
                .get(format!("{}/health/ready", url.trim()))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
        } else {
            false
        };
        if !ready {
            let binary = std::env::current_exe()?.with_file_name("aincd");
            anyhow::ensure!(
                binary.exists(),
                "Companion daemon missing. Start the development stack or reinstall AgentInc."
            );
            let database = std::env::var("AINC_DATABASE_URL").context(
                "Daemon unavailable. Start the local stack or configure AINC_DATABASE_URL.",
            )?;
            std::process::Command::new(binary)
                .process_group(0)
                .env("DATABASE_URL", database)
                .env("AINC_DISCOVERY_FILE", &discovery)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            for _ in 0..100 {
                if let Ok(url) = std::fs::read_to_string(&discovery)
                    && probe
                        .get(format!("{}/health/ready", url.trim()))
                        .send()
                        .await
                        .is_ok_and(|r| r.status().is_success())
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
        std::fs::read_to_string(&discovery)
            .context("Daemon unavailable. Start the local stack and refresh.")?
            .trim()
            .to_owned()
    };
    let token_path = std::env::var_os("AINC_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| discovery.with_file_name("owner-token"));
    let token =
        std::fs::read_to_string(token_path).context("Daemon owner credential unavailable")?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {}", token.trim()).parse()?,
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(40))
        .build()?;
    Ok(Client::new_with_client(url.trim_end_matches('/'), http))
}

pub struct Store {
    snapshot: Mutex<Snapshot>,
    // Serialize mutations and refreshes so an older snapshot cannot replace a
    // newer acknowledgement. Never acquire this lock on the foreground.
    requests: Mutex<()>,
    pending: Mutex<Option<CommandRequest>>,
    #[cfg(test)]
    fixture: bool,
}
impl Store {
    pub fn new() -> Self {
        Self {
            snapshot: Mutex::new(Snapshot {
                conversations: vec![],
                turns: vec![],
                todos: vec![],
                settings: Default::default(),
            }),
            requests: Mutex::new(()),
            pending: Mutex::new(None),
            #[cfg(test)]
            fixture: false,
        }
    }
    #[cfg(test)]
    pub fn open(_: &std::path::Path) -> Result<Self> {
        Ok(Self {
            fixture: true,
            ..Self::new()
        })
    }
    pub fn refresh(&self) -> Result<()> {
        #[cfg(test)]
        if self.fixture {
            return Ok(());
        }
        let pending = self.pending.lock().expect("pending command").clone();
        if let Some(request) = pending {
            return self.command(request.command).map(|_| ());
        }
        let _serial = self
            .requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Request state unavailable"))?;
        self.refresh_inner()
    }
    fn refresh_inner(&self) -> Result<()> {
        let snapshot =
            background(async { Ok(client().await?.product_state().send().await?.into_inner()) })?;
        *self
            .snapshot
            .lock()
            .map_err(|_| anyhow::anyhow!("Snapshot unavailable"))? = snapshot;
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().expect("presentation snapshot").clone()
    }
    pub fn todos(&self) -> Result<Vec<Todo>> {
        Ok(self.snapshot().todos)
    }
    pub fn command(&self, command: Command) -> Result<Option<i64>> {
        #[cfg(test)]
        if self.fixture {
            return self.fixture_command(command);
        }
        let _serial = self
            .requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Request state unavailable"))?;
        let request = {
            let mut pending = self.pending.lock().expect("pending command");
            if let Some(prior) = pending.as_ref() {
                anyhow::ensure!(
                    serde_json::to_value(&prior.command)? == serde_json::to_value(&command)?,
                    "A previous command has an unknown acknowledgement. Retry that command before making another change."
                );
                prior.clone()
            } else {
                let request = CommandRequest {
                    command,
                    operation_id: uuid::Uuid::new_v4().to_string(),
                };
                *pending = Some(request.clone());
                request
            }
        };
        // Retry the same operation ID on an ambiguous transport failure. The
        // receipt makes this safe; never create a second operation in this path.
        let ack = background(async {
            let client = client().await?;
            let mut result = client.product_command().body(request.clone()).send().await;
            if matches!(result, Err(progenitor_client::Error::CommunicationError(_))) {
                result = client.product_command().body(request).send().await;
            }
            match result {
                Ok(ack) => Ok(ack.into_inner()),
                Err(error) => {
                    if error.status().is_some_and(|s| s.is_client_error()) {
                        *self.pending.lock().expect("pending command") = None;
                    }
                    Err(error.into())
                }
            }
        })?;
        self.refresh_inner()
            .context("Change acknowledged by the daemon; refresh to load the saved result")?;
        *self.pending.lock().expect("pending command") = None;
        Ok(ack.result_id)
    }
    pub fn new_conversation(&self) -> Result<i64> {
        self.command(Command::CreateConversation)?
            .context("Missing conversation acknowledgement")
    }
    pub fn begin_turn(&self, id: i64, prompt: &str) -> Result<i64> {
        let turn = self
            .command(Command::Send {
                conversation_id: id,
                prompt: prompt.into(),
            })?
            .context("Missing turn acknowledgement")?;
        Ok(turn)
    }
    pub fn add_todo(&self, title: &str) -> Result<()> {
        self.command(Command::CreateTodo {
            title: title.into(),
        })?;
        Ok(())
    }
    pub fn set_completed(&self, id: i64, completed: bool) -> Result<()> {
        self.command(Command::CompleteTodo { id, completed })?;
        Ok(())
    }
    pub fn delete_todo(&self, id: i64) -> Result<()> {
        self.command(Command::DeleteTodo { id })?;
        Ok(())
    }
    #[cfg(test)]
    fn fixture_command(&self, command: Command) -> Result<Option<i64>> {
        let mut s = self.snapshot.lock().unwrap();
        match command {
            Command::CreateTodo { title } => {
                let title = title.trim();
                if title.is_empty() || title.chars().count() > 500 {
                    bail!("Invalid title");
                }
                let id = s.todos.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                s.todos.insert(
                    0,
                    Todo {
                        id,
                        title: title.into(),
                        completed: false,
                    },
                );
                Ok(Some(id))
            }
            Command::CompleteTodo { id, completed } => {
                if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                    t.completed = completed;
                }
                Ok(Some(id))
            }
            Command::DeleteTodo { id } => {
                s.todos.retain(|t| t.id != id);
                Ok(Some(id))
            }
            _ => bail!("No rendered fixture for this command"),
        }
    }
}
