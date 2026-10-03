//! An in-memory daemon for tests: the same [`Transport`] seam as HTTP, state
//! you can edit directly, scripted failures, and a log of every request.
use super::board;
use super::transport::{DaemonError, Envelope, Reply, Request, Slice, Transport};
use ainc_client::types::{
    AutomationSnapshot, ConversationSnapshot, ErrorBody, ErrorCode, TicketActivity, TicketSnapshot,
    Workspace, WorkspaceCommand, WorkspaceSnapshot,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// Everything the in-memory daemon knows.
pub struct State {
    pub workspaces: WorkspaceSnapshot,
    pub product: ConversationSnapshot,
    pub tickets: TicketSnapshot,
    pub automations: AutomationSnapshot,
    pub activity: Vec<TicketActivity>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            workspaces: super::default_workspaces(),
            product: super::default_product(),
            tickets: super::default_tickets(),
            automations: AutomationSnapshot {
                automations: vec![],
                occurrences: vec![],
                history: vec![],
            },
            activity: vec![],
        }
    }
}

#[derive(Default)]
pub struct MemoryTransport {
    state: Mutex<State>,
    requests: Mutex<Vec<Request>>,
    failures: Mutex<VecDeque<DaemonError>>,
}
impl MemoryTransport {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }
    /// Edit the state directly, for what no command produces on its own.
    pub fn edit(&self, edit: impl FnOnce(&mut State)) {
        edit(&mut self.state.lock().expect("memory daemon state"));
    }
    /// Fail the next request with `error` instead of answering it.
    pub fn fail_next(&self, error: DaemonError) {
        self.failures
            .lock()
            .expect("scripted failures")
            .push_back(error);
    }
    /// Every request received so far, oldest first.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("request log").clone()
    }
    pub fn clear_requests(&self) {
        self.requests.lock().expect("request log").clear();
    }
}
fn unsupported(what: &str) -> DaemonError {
    DaemonError::Rejected(ErrorBody {
        code: ErrorCode::Unavailable,
        message: format!("{what} require the daemon"),
    })
}
impl Transport for MemoryTransport {
    fn call(&self, request: Request) -> Result<Reply, DaemonError> {
        self.requests
            .lock()
            .expect("request log")
            .push(request.clone());
        if let Some(error) = self.failures.lock().expect("scripted failures").pop_front() {
            return Err(error);
        }
        let mut state = self.state.lock().expect("memory daemon state");
        match request {
            Request::Fetch(Slice::Workspaces) => Ok(Reply::Workspaces(state.workspaces.clone())),
            Request::Fetch(Slice::Product) => Ok(Reply::Product(state.product.clone())),
            Request::Fetch(Slice::Tickets) => Ok(Reply::Tickets(state.tickets.clone())),
            Request::Fetch(Slice::Automations) => Ok(Reply::Automations(state.automations.clone())),
            Request::Fetch(Slice::Activity { id }) => Ok(Reply::Activity(
                state
                    .activity
                    .iter()
                    .filter(|a| a.ticket_id == id)
                    .cloned()
                    .collect(),
            )),
            Request::Fetch(Slice::Executions { .. }) => Err(unsupported("Workflow histories")),
            Request::Command(Envelope::Ticket(request)) => {
                let State {
                    tickets, activity, ..
                } = &mut *state;
                board::apply(tickets, activity, crate::ui::time::now(), request.command)
                    .map(Reply::Acknowledgement)
                    .map_err(DaemonError::Rejected)
            }
            Request::Command(Envelope::Workspace(request)) => {
                let workspaces = &mut state.workspaces;
                let id = match request.command {
                    WorkspaceCommand::Create { name, icon, color } => {
                        let id = uuid::Uuid::new_v4().to_string();
                        workspaces.workspaces.push(Workspace {
                            id: id.clone(),
                            name,
                            icon,
                            color,
                        });
                        workspaces.current_id = id.clone();
                        id
                    }
                    WorkspaceCommand::Rename { id, name } => {
                        let workspace = workspaces
                            .workspaces
                            .iter_mut()
                            .find(|item| item.id == id)
                            .ok_or_else(|| unsupported("Unknown workspaces"))?;
                        workspace.name = name;
                        id
                    }
                    WorkspaceCommand::Switch { id } => {
                        if !workspaces.workspaces.iter().any(|item| item.id == id) {
                            return Err(unsupported("Unknown workspaces"));
                        }
                        workspaces.current_id = id.clone();
                        id
                    }
                };
                Ok(Reply::Receipt(id))
            }
            Request::Command(Envelope::Product(request)) => match request.command {
                ainc_client::types::ConversationCommand::SelectModel { model } => {
                    state.product.settings.model = Some(model).filter(|model| !model.is_empty());
                    Ok(Reply::Acknowledgement(None))
                }
                _ => Err(unsupported("Conversation commands")),
            },
            Request::Command(Envelope::Automation(_)) => Err(unsupported("Automation writes")),
            Request::ConnectionStatus | Request::Connection(_) => Err(unsupported("Connections")),
        }
    }
}
