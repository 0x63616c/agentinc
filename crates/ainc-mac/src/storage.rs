//! Generated daemon client plus an owned presentation snapshot. The foreground
//! reads this cache only; refresh and command methods run on the background executor.
pub use ainc_client::types::{
    ActivityKind, Assignee, AssigneeKind, Automation, AutomationCommand, AutomationSnapshot,
    Command, Comment, Conversation, LinkKind, Ticket, TicketActivity, TicketCommand, TicketLink,
    TicketPriority, TicketProposal, TicketSnapshot, TicketStatus, Turn, Workspace,
    WorkspaceCommand, WorkspaceState,
};
use ainc_client::{
    Client,
    types::{CommandRequest, Snapshot, TicketCommandRequest},
};
#[cfg(test)]
use anyhow::bail;
use anyhow::{Context, Result};
use std::{
    future::Future,
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

pub fn background<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Runtime::new().expect("HTTP runtime"))
        .block_on(future)
}
pub(crate) fn discovery_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AINC_DISCOVERY_FILE") {
        return Ok(path.into());
    }
    Ok(ainc_release::identity::support_dir().join("daemon/api-url"))
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
            let mut command = std::process::Command::new(binary);
            if let Ok(database) = std::env::var("AINC_DATABASE_URL") {
                command.env("DATABASE_URL", database);
            }
            std::fs::create_dir_all(discovery.parent().context("discovery directory")?)?;
            let log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(discovery.with_file_name("daemon.log"))?;
            ainc_release::process::prepare_child(&mut command)
                .process_group(0)
                .env("AINC_DISCOVERY_FILE", &discovery)
                .stdin(std::process::Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?;
            for _ in 0..1200 {
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
    tickets: Mutex<TicketSnapshot>,
    automations: Mutex<AutomationSnapshot>,
    workspaces: Mutex<WorkspaceState>,
    pending_workspace: Mutex<Option<ainc_client::types::WorkspaceRequest>>,
    pending_automation: Mutex<Option<ainc_client::types::AutomationRequest>>,
    pending_ticket: Mutex<Option<TicketCommandRequest>>,
    // Serialize mutations and refreshes so an older snapshot cannot replace a
    // newer acknowledgement. Never acquire this lock on the foreground.
    requests: Mutex<()>,
    pending: Mutex<Option<CommandRequest>>,
    #[cfg(test)]
    fixture: bool,
    #[cfg(test)]
    fixture_activity: Mutex<Vec<TicketActivity>>,
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
            tickets: Mutex::new(TicketSnapshot {
                tickets: vec![],
                comments: vec![],
                runs: vec![],
                links: vec![],
                assignees: vec![Assignee {
                    id: "owner".into(),
                    name: "You".into(),
                    kind: AssigneeKind::Human,
                }],
            }),
            automations: Mutex::new(AutomationSnapshot {
                rules: vec![],
                occurrences: vec![],
                history: vec![],
            }),
            workspaces: Mutex::new(WorkspaceState {
                current_id: "local".into(),
                workspaces: vec![Workspace {
                    id: "local".into(),
                    name: "World Wide Webb".into(),
                    icon: None,
                    color: None,
                }],
            }),
            pending_workspace: Mutex::new(None),
            pending_automation: Mutex::new(None),
            pending_ticket: Mutex::new(None),
            requests: Mutex::new(()),
            pending: Mutex::new(None),
            #[cfg(test)]
            fixture: false,
            #[cfg(test)]
            fixture_activity: Mutex::new(vec![]),
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
        let pending_workspace = self
            .pending_workspace
            .lock()
            .expect("pending Workspace command")
            .clone();
        if let Some(request) = pending_workspace {
            return self.workspace_command(request.command).map(|_| ());
        }
        let pending_ticket = self
            .pending_ticket
            .lock()
            .expect("pending Ticket command")
            .clone();
        if let Some(request) = pending_ticket {
            return self.ticket_command(request.command).map(|_| ());
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
        let (snapshot, tickets, automations, workspaces) = background(async {
            let client = client().await?;
            let workspaces = client.workspaces_state().send().await?.into_inner();
            let snapshot = client.product_state().send().await?.into_inner();
            let tickets = client.tickets_state().send().await?.into_inner();
            let automations = client.automations_state().send().await?.into_inner();
            anyhow::Ok((snapshot, tickets, automations, workspaces))
        })?;
        *self.workspaces.lock().expect("Workspace snapshot") = workspaces;
        *self.tickets.lock().expect("Ticket snapshot") = tickets;
        *self.automations.lock().expect("Automation snapshot") = automations;
        *self
            .snapshot
            .lock()
            .map_err(|_| anyhow::anyhow!("Snapshot unavailable"))? = snapshot;
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().expect("presentation snapshot").clone()
    }
    pub fn tickets(&self) -> TicketSnapshot {
        self.tickets.lock().expect("Ticket snapshot").clone()
    }
    /// One Ticket's history, oldest first. Blocks; run it on the background executor.
    pub fn ticket_activity(&self, id: i64) -> Result<Vec<TicketActivity>> {
        #[cfg(test)]
        if self.fixture {
            let log = self.fixture_activity.lock().expect("fixture history");
            return Ok(log.iter().filter(|a| a.ticket_id == id).cloned().collect());
        }
        background(async {
            let client = client().await?;
            Ok(client.tickets_activity().id(id).send().await?.into_inner())
        })
    }
    pub fn automations(&self) -> AutomationSnapshot {
        self.automations
            .lock()
            .expect("Automation snapshot")
            .clone()
    }
    pub fn workspaces(&self) -> WorkspaceState {
        self.workspaces.lock().expect("Workspace snapshot").clone()
    }
    pub fn workspace_command(&self, command: WorkspaceCommand) -> Result<String> {
        #[cfg(test)]
        if self.fixture {
            return self.fixture_workspace_command(command);
        }
        let _serial = self
            .requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Request state unavailable"))?;
        let request = {
            let mut pending = self
                .pending_workspace
                .lock()
                .expect("pending Workspace command");
            if let Some(prior) = pending.as_ref() {
                anyhow::ensure!(
                    serde_json::to_value(&prior.command)? == serde_json::to_value(&command)?,
                    "Retry the unacknowledged Workspace change before another change."
                );
                prior.clone()
            } else {
                let request = ainc_client::types::WorkspaceRequest {
                    command,
                    operation_id: uuid::Uuid::new_v4().to_string(),
                };
                *pending = Some(request.clone());
                request
            }
        };
        let result = background(async {
            let client = client().await?;
            let mut result = client
                .workspaces_command()
                .body(request.clone())
                .send()
                .await;
            if matches!(result, Err(progenitor_client::Error::CommunicationError(_))) {
                result = client.workspaces_command().body(request).send().await;
            }
            match result {
                Ok(receipt) => Ok(receipt.into_inner()),
                Err(error) => {
                    if error.status().is_some_and(|s| s.is_client_error()) {
                        *self
                            .pending_workspace
                            .lock()
                            .expect("pending Workspace command") = None;
                    }
                    Err(error.into())
                }
            }
        })?;
        self.refresh_inner()
            .context("Workspace changed; refresh to load it")?;
        *self
            .pending_workspace
            .lock()
            .expect("pending Workspace command") = None;
        Ok(result.result_id)
    }
    #[cfg(test)]
    fn fixture_workspace_command(&self, command: WorkspaceCommand) -> Result<String> {
        let mut state = self.workspaces.lock().expect("Workspace snapshot");
        match command {
            WorkspaceCommand::Create { name, icon, color } => {
                let id = uuid::Uuid::new_v4().to_string();
                state.workspaces.push(Workspace {
                    id: id.clone(),
                    name,
                    icon,
                    color,
                });
                state.current_id = id.clone();
                Ok(id)
            }
            WorkspaceCommand::Rename { id, name } => {
                let workspace = state
                    .workspaces
                    .iter_mut()
                    .find(|item| item.id == id)
                    .context("Workspace unavailable")?;
                workspace.name = name;
                Ok(id)
            }
            WorkspaceCommand::Switch { id } => {
                anyhow::ensure!(
                    state.workspaces.iter().any(|item| item.id == id),
                    "Workspace unavailable"
                );
                state.current_id = id.clone();
                Ok(id)
            }
        }
    }
    pub fn automation_command(&self, command: AutomationCommand) -> Result<String> {
        #[cfg(test)]
        if self.fixture {
            bail!("Automation writes require the daemon");
        }
        let _serial = self
            .requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Request state unavailable"))?;
        let request = {
            let mut pending = self
                .pending_automation
                .lock()
                .expect("pending Automation command");
            if let Some(prior) = pending.as_ref() {
                anyhow::ensure!(
                    serde_json::to_value(&prior.command)? == serde_json::to_value(&command)?,
                    "Retry the unacknowledged Automation change before another change."
                );
                prior.clone()
            } else {
                let request = ainc_client::types::AutomationRequest {
                    command,
                    operation_id: uuid::Uuid::new_v4().to_string(),
                };
                *pending = Some(request.clone());
                request
            }
        };
        let ack = background(async {
            let client = client().await?;
            let mut result = client
                .automations_command()
                .body(request.clone())
                .send()
                .await;
            if matches!(result, Err(progenitor_client::Error::CommunicationError(_))) {
                result = client.automations_command().body(request).send().await;
            }
            match result {
                Ok(ack) => Ok(ack.into_inner()),
                Err(error) => {
                    if error.status().is_some_and(|s| s.is_client_error()) {
                        *self
                            .pending_automation
                            .lock()
                            .expect("pending Automation command") = None;
                    }
                    Err(error.into())
                }
            }
        })?;
        self.refresh_inner()
            .context("Change acknowledged; refresh to load the saved result")?;
        *self
            .pending_automation
            .lock()
            .expect("pending Automation command") = None;
        Ok(ack.result_id)
    }
    pub fn ticket_command(&self, command: TicketCommand) -> Result<Option<i64>> {
        #[cfg(test)]
        if self.fixture {
            return self.fixture_ticket(command);
        }
        let _serial = self
            .requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Request state unavailable"))?;
        let request = {
            let mut pending = self.pending_ticket.lock().expect("pending Ticket command");
            if let Some(prior) = pending.as_ref() {
                anyhow::ensure!(
                    serde_json::to_value(&prior.command)? == serde_json::to_value(&command)?,
                    "Retry the unacknowledged Ticket change before another change."
                );
                prior.clone()
            } else {
                let request = TicketCommandRequest {
                    command,
                    operation_id: uuid::Uuid::new_v4().to_string(),
                };
                *pending = Some(request.clone());
                request
            }
        };
        let ack = background(async {
            let client = client().await?;
            let mut result = client.tickets_command().body(request.clone()).send().await;
            if matches!(result, Err(progenitor_client::Error::CommunicationError(_))) {
                result = client.tickets_command().body(request).send().await;
            }
            match result {
                Ok(ack) => Ok(ack.into_inner()),
                Err(error) => {
                    if error.status().is_some_and(|s| s.is_client_error()) {
                        *self.pending_ticket.lock().expect("pending Ticket command") = None;
                    }
                    Err(error.into())
                }
            }
        })?;
        self.refresh_inner()
            .context("Change acknowledged; refresh to load the saved result")?;
        *self.pending_ticket.lock().expect("pending Ticket command") = None;
        Ok(ack.result_id)
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
    #[cfg(test)]
    fn fixture_command(&self, command: Command) -> Result<Option<i64>> {
        match command {
            Command::SelectModel { model } => {
                let mut snapshot = self.snapshot.lock().expect("presentation snapshot");
                snapshot.settings.model = Some(model).filter(|model| !model.is_empty());
                Ok(None)
            }
            _ => bail!("No rendered fixture for this command"),
        }
    }
    /// Edit the fixture snapshot and history directly, for what no command
    /// produces on its own: runs, agent Comments and times in the past.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn fixture_edit(&self, edit: impl FnOnce(&mut TicketSnapshot, &mut Vec<TicketActivity>)) {
        let mut snapshot = self.tickets.lock().expect("Ticket snapshot");
        let mut history = self.fixture_activity.lock().expect("fixture history");
        edit(&mut snapshot, &mut history);
    }
    #[cfg(test)]
    fn fixture_ticket(&self, command: TicketCommand) -> Result<Option<i64>> {
        use crate::tickets::model::{apply_move, status_key};
        let mut s = self.tickets.lock().unwrap();
        let mut log = self.fixture_activity.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let mut record = |ticket_id: i64, kind, from: Option<String>, to: Option<String>| {
            let id = log.len() as i64 + 1;
            log.push(TicketActivity {
                id,
                ticket_id,
                actor_id: "owner".into(),
                kind,
                from_value: from,
                to_value: to,
                conversation_id: None,
                run_id: None,
                created_at: now,
            });
        };
        let ticket = |s: &TicketSnapshot, id: i64| -> Result<usize> {
            s.tickets
                .iter()
                .position(|t| t.id == id)
                .context("No such fixture Ticket")
        };
        match command {
            TicketCommand::Delete { id, .. } => {
                s.tickets.retain(|t| t.id != id);
                s.comments.retain(|c| c.ticket_id != id);
                s.links.retain(|l| l.from_id != id && l.to_id != id);
                Ok(Some(id))
            }
            TicketCommand::Create { title } => self.fixture_ticket_create(
                &mut s,
                TicketCommand::CreateDetailed {
                    title,
                    description: None,
                    status: None,
                    priority: None,
                    labels: vec![],
                    assignee_id: None,
                },
                now,
                &mut record,
            ),
            command @ TicketCommand::CreateDetailed { .. } => {
                self.fixture_ticket_create(&mut s, command, now, &mut record)
            }
            TicketCommand::SetStatus { id, status, .. } => {
                let from = s.tickets[ticket(&s, id)?].status;
                if from != status {
                    apply_move(&mut s.tickets, id, status, None);
                    record(
                        id,
                        ActivityKind::Status,
                        Some(status_key(from).into()),
                        Some(status_key(status).into()),
                    );
                }
                Ok(Some(id))
            }
            TicketCommand::Move {
                id, status, after, ..
            } => {
                let from = s.tickets[ticket(&s, id)?].status;
                anyhow::ensure!(apply_move(&mut s.tickets, id, status, after), "Stale board");
                if from != status {
                    record(
                        id,
                        ActivityKind::Status,
                        Some(status_key(from).into()),
                        Some(status_key(status).into()),
                    );
                }
                Ok(Some(id))
            }
            TicketCommand::Rename { id, title, .. } => {
                let index = ticket(&s, id)?;
                let old = std::mem::replace(&mut s.tickets[index].title, title.trim().into());
                s.tickets[index].revision += 1;
                record(
                    id,
                    ActivityKind::Renamed,
                    Some(old),
                    Some(title.trim().into()),
                );
                Ok(Some(id))
            }
            TicketCommand::Describe {
                id, description, ..
            } => {
                let index = ticket(&s, id)?;
                s.tickets[index].description = description.trim().into();
                s.tickets[index].revision += 1;
                record(id, ActivityKind::Described, None, None);
                Ok(Some(id))
            }
            TicketCommand::SetPriority { id, priority, .. } => {
                let index = ticket(&s, id)?;
                s.tickets[index].priority = priority;
                s.tickets[index].revision += 1;
                record(
                    id,
                    ActivityKind::Priority,
                    None,
                    Some(crate::tickets::model::priority_key(priority).into()),
                );
                Ok(Some(id))
            }
            TicketCommand::SetLabels { id, labels, .. } => {
                let index = ticket(&s, id)?;
                let old = std::mem::replace(&mut s.tickets[index].labels, labels.clone());
                s.tickets[index].revision += 1;
                record(
                    id,
                    ActivityKind::Labels,
                    Some(old.join(",")),
                    Some(labels.join(",")),
                );
                Ok(Some(id))
            }
            TicketCommand::Assign {
                id,
                assignee_id,
                assignee_kind,
                ..
            } => {
                let index = ticket(&s, id)?;
                let old = std::mem::replace(&mut s.tickets[index].assignee_id, assignee_id.clone());
                s.tickets[index].assignee_kind = assignee_kind;
                s.tickets[index].revision += 1;
                record(id, ActivityKind::Assigned, Some(old), Some(assignee_id));
                Ok(Some(id))
            }
            TicketCommand::Link {
                from_id,
                to_id,
                link,
            }
            | TicketCommand::Unlink {
                from_id,
                to_id,
                link,
            } => {
                let adding = s
                    .links
                    .iter()
                    .all(|l| (l.from_id, l.to_id, l.kind) != (from_id, to_id, link));
                s.links
                    .retain(|l| (l.from_id, l.to_id, l.kind) != (from_id, to_id, link));
                let kind = if adding {
                    s.links.push(TicketLink {
                        from_id,
                        to_id,
                        kind: link,
                    });
                    ActivityKind::Linked
                } else {
                    ActivityKind::Unlinked
                };
                let (source, target) = match link {
                    LinkKind::Blocks => ("blocks", "blocked_by"),
                    LinkKind::RelatesTo => ("relates_to", "relates_to"),
                    LinkKind::Duplicates => ("duplicates", "duplicated_by"),
                    LinkKind::ParentOf => ("parent_of", "child_of"),
                };
                record(from_id, kind, Some(source.into()), Some(to_id.to_string()));
                record(to_id, kind, Some(target.into()), Some(from_id.to_string()));
                Ok(Some(from_id))
            }
            TicketCommand::AddComment { ticket_id, body } => {
                let id = s.comments.len() as i64 + 1;
                s.comments.push(Comment {
                    id,
                    ticket_id,
                    body,
                    author_id: "owner".into(),
                    created_at: now,
                });
                Ok(Some(id))
            }
            TicketCommand::RegisterAgent { name, .. } => {
                let id = format!("agent-{}", s.assignees.len());
                s.assignees.push(Assignee {
                    id,
                    name,
                    kind: AssigneeKind::Agent,
                });
                Ok(None)
            }
            _ => bail!("No rendered fixture for this Ticket command"),
        }
    }
    #[cfg(test)]
    fn fixture_ticket_create(
        &self,
        s: &mut TicketSnapshot,
        command: TicketCommand,
        now: i64,
        record: &mut impl FnMut(i64, ActivityKind, Option<String>, Option<String>),
    ) -> Result<Option<i64>> {
        let TicketCommand::CreateDetailed {
            title,
            description,
            status,
            priority,
            labels,
            assignee_id,
        } = command
        else {
            bail!("Not a create command");
        };
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 500 {
            bail!("Invalid title");
        }
        let id = s.tickets.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let status = status.unwrap_or(TicketStatus::ToDo);
        let (assignee_kind, assignee_id) = match assignee_id {
            Some(id) => (
                s.assignees
                    .iter()
                    .find(|a| a.id == id)
                    .context("Unknown fixture assignee")?
                    .kind,
                id,
            ),
            None => (AssigneeKind::Human, "owner".into()),
        };
        s.tickets.push(Ticket {
            id,
            title: title.into(),
            description: description.unwrap_or_default(),
            status,
            priority: priority.unwrap_or(TicketPriority::None),
            labels,
            assignee_id,
            assignee_kind,
            position: 0,
            generation: 0,
            revision: 0,
            created_at: now,
            updated_at: now,
            conversation_id: None,
        });
        crate::tickets::model::apply_move(&mut s.tickets, id, status, None);
        record(id, ActivityKind::Created, None, Some(title.into()));
        Ok(Some(id))
    }
}
