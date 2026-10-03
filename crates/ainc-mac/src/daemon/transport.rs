//! The seam between the [`Daemon`](super::Daemon) and whatever answers its
//! requests: the real daemon over HTTP, or an in-memory stand-in in tests.
pub use ainc_client::ClientError as DaemonError;
use ainc_client::types::{
    AutomationRequest, AutomationSnapshot, CommandRequest, ConnectionStatus, Snapshot,
    TicketActivity, TicketCommandRequest, TicketSnapshot, WorkPage, WorkspaceRequest,
    WorkspaceSnapshot,
};

/// One request to the daemon, already carrying any operation id.
#[derive(Debug, Clone)]
pub enum Request {
    Fetch(Slice),
    Command(Envelope),
    ConnectionStatus,
    Connection(ConnectionAction),
}

/// A read of daemon state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slice {
    Workspaces,
    Product,
    Tickets,
    Automations,
    Activity {
        id: i64,
    },
    Executions {
        status: String,
        page: Option<String>,
    },
}

/// A command with its operation id, addressed to one command family.
#[derive(Debug, Clone)]
pub enum Envelope {
    Workspace(WorkspaceRequest),
    Automation(AutomationRequest),
    Ticket(TicketCommandRequest),
    Product(CommandRequest),
}
impl Envelope {
    #[cfg(any(test, feature = "fixtures"))]
    pub fn operation_id(&self) -> &str {
        match self {
            Envelope::Workspace(request) => &request.operation_id,
            Envelope::Automation(request) => &request.operation_id,
            Envelope::Ticket(request) => &request.operation_id,
            Envelope::Product(request) => &request.operation_id,
        }
    }
}

/// Daemon-owned Codex sign-in actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionAction {
    Login,
    Cancel,
    Logout,
}

/// The daemon's answer to one [`Request`].
#[derive(Debug, Clone)]
pub enum Reply {
    Workspaces(WorkspaceSnapshot),
    Product(Snapshot),
    Tickets(TicketSnapshot),
    Automations(AutomationSnapshot),
    Activity(Vec<TicketActivity>),
    Executions(WorkPage),
    /// A workspace or automation receipt: the id of what the command produced.
    Receipt(String),
    /// A ticket or product acknowledgement: the id of what the command produced, if any.
    Acknowledgement(Option<i64>),
    ConnectionStatus(ConnectionStatus),
    Done,
}

/// Answers requests. Blocking: callers run on the background executor.
pub trait Transport: Send + Sync {
    fn call(&self, request: Request) -> Result<Reply, DaemonError>;
}
