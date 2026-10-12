//! The seam between the [`Daemon`](super::Daemon) and whatever answers its
//! requests: the real daemon over HTTP, or an in-memory stand-in in tests.
pub use ainc_client::ClientError as DaemonError;
use ainc_client::types::{
    CalendarRequest, CalendarSnapshot, TodoRequest, TodoSnapshot, WorkspaceRequest,
    WorkspaceSnapshot,
};

/// One request to the daemon, already carrying any operation id.
#[derive(Debug, Clone)]
pub enum Request {
    Fetch(Slice),
    Command(Envelope),
}

/// A read of daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slice {
    Workspaces,
    Todos,
    Calendar,
}

/// A command with its operation id, addressed to one command family.
#[derive(Debug, Clone)]
pub enum Envelope {
    Workspace(WorkspaceRequest),
    Todo(TodoRequest),
    Calendar(CalendarRequest),
}
impl Envelope {
    #[cfg(any(test, feature = "fixtures"))]
    pub fn operation_id(&self) -> &str {
        match self {
            Envelope::Workspace(request) => &request.operation_id,
            Envelope::Todo(request) => &request.operation_id,
            Envelope::Calendar(request) => &request.operation_id,
        }
    }
}

/// The daemon's answer to one [`Request`].
#[derive(Debug, Clone)]
pub enum Reply {
    Workspaces(WorkspaceSnapshot),
    Todos(TodoSnapshot),
    Calendar(CalendarSnapshot),
    /// A workspace receipt: the id of what the command produced.
    Receipt(String),
    /// A Todo or Calendar acknowledgement: the id of the record the command touched.
    Acknowledgement(i64),
}

/// Answers requests. Blocking: callers run on the background executor.
pub trait Transport: Send + Sync {
    fn call(&self, request: Request) -> Result<Reply, DaemonError>;
}
