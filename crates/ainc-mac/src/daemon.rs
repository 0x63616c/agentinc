//! The app's one connection to the daemon. Pages read cached snapshots on the
//! foreground and run `refresh`, `send` and `fetch` on the background executor.
//!
//! Commands are idempotent: `send` mints one operation id per command, keeps it
//! in a per-family pending slot until the daemon acknowledges, retries the same
//! id once on a transport failure, and lets go only when the daemon rejects it.
//! While a family's slot is full, a *different* command in that family is
//! refused until the pending one is retried.
#[cfg(any(test, feature = "fixtures"))]
#[path = "daemon/board.rs"]
mod board;
#[path = "daemon/http.rs"]
mod http;
#[cfg(any(test, feature = "fixtures"))]
#[path = "daemon/memory.rs"]
pub mod memory;
#[path = "daemon/transport.rs"]
pub mod transport;

#[cfg(ainc_upgrade_test)]
pub(crate) use http::connect as client;
pub(crate) use http::{block_on, discovery_path};
pub use transport::{ConnectionAction, DaemonError};
use transport::{Envelope, Reply, Request, Slice, Transport};

use ainc_client::types::{
    Assignee, AssigneeKind, AutomationCommand, AutomationRequest, AutomationSnapshot,
    Command as ProductCommand, CommandRequest, ConnectionStatus, ErrorBody, ErrorCode, Snapshot,
    TicketActivity, TicketCommand, TicketCommandRequest, TicketSnapshot, WorkPage, Workspace,
    WorkspaceCommand, WorkspaceRequest, WorkspaceSnapshot,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// The daemon's command families, each with its own receipt table and pending slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Workspace,
    Automation,
    Ticket,
    Product,
}
impl Family {
    const ALL: [Family; 4] = [
        Family::Workspace,
        Family::Automation,
        Family::Ticket,
        Family::Product,
    ];
    fn name(self) -> &'static str {
        match self {
            Family::Workspace => "Workspace",
            Family::Automation => "Automation",
            Family::Ticket => "Ticket",
            Family::Product => "Conversation",
        }
    }
}

/// A command the daemon acknowledges with a receipt.
pub trait Command: serde::Serialize + Clone {
    type Ack;
    fn family() -> Family;
    fn envelope(self, operation_id: String) -> Envelope;
    fn ack(reply: Reply) -> Option<Self::Ack>;
}
impl Command for TicketCommand {
    type Ack = Option<i64>;
    fn family() -> Family {
        Family::Ticket
    }
    fn envelope(self, operation_id: String) -> Envelope {
        Envelope::Ticket(TicketCommandRequest {
            command: self,
            operation_id,
        })
    }
    fn ack(reply: Reply) -> Option<Self::Ack> {
        match reply {
            Reply::Acknowledgement(id) => Some(id),
            _ => None,
        }
    }
}
impl Command for ProductCommand {
    type Ack = Option<i64>;
    fn family() -> Family {
        Family::Product
    }
    fn envelope(self, operation_id: String) -> Envelope {
        Envelope::Product(CommandRequest {
            command: self,
            operation_id,
        })
    }
    fn ack(reply: Reply) -> Option<Self::Ack> {
        match reply {
            Reply::Acknowledgement(id) => Some(id),
            _ => None,
        }
    }
}
impl Command for AutomationCommand {
    type Ack = String;
    fn family() -> Family {
        Family::Automation
    }
    fn envelope(self, operation_id: String) -> Envelope {
        Envelope::Automation(AutomationRequest {
            command: self,
            operation_id,
        })
    }
    fn ack(reply: Reply) -> Option<Self::Ack> {
        match reply {
            Reply::Receipt(id) => Some(id),
            _ => None,
        }
    }
}
impl Command for WorkspaceCommand {
    type Ack = String;
    fn family() -> Family {
        Family::Workspace
    }
    fn envelope(self, operation_id: String) -> Envelope {
        Envelope::Workspace(WorkspaceRequest {
            command: self,
            operation_id,
        })
    }
    fn ack(reply: Reply) -> Option<Self::Ack> {
        match reply {
            Reply::Receipt(id) => Some(id),
            _ => None,
        }
    }
}

/// A typed read of daemon state.
pub trait Fetch {
    type Snapshot;
    fn slice(&self) -> Slice;
    fn snapshot(reply: Reply) -> Option<Self::Snapshot>;
}
pub struct Workspaces;
pub struct Product;
pub struct Tickets;
pub struct Automations;
pub struct Activity(pub i64);
pub struct Executions {
    pub status: String,
    pub page: Option<String>,
}
macro_rules! fetch {
    ($name:ty, $snapshot:ty, |$this:ident| $slice:expr, $reply:ident) => {
        impl Fetch for $name {
            type Snapshot = $snapshot;
            fn slice(&self) -> Slice {
                let $this = self;
                $slice
            }
            fn snapshot(reply: Reply) -> Option<Self::Snapshot> {
                match reply {
                    Reply::$reply(value) => Some(value),
                    _ => None,
                }
            }
        }
    };
}
fetch!(
    Workspaces,
    WorkspaceSnapshot,
    |_s| Slice::Workspaces,
    Workspaces
);
fetch!(Product, Snapshot, |_s| Slice::Product, Product);
fetch!(Tickets, TicketSnapshot, |_s| Slice::Tickets, Tickets);
fetch!(
    Automations,
    AutomationSnapshot,
    |_s| Slice::Automations,
    Automations
);
fetch!(
    Activity,
    Vec<TicketActivity>,
    |s| Slice::Activity { id: s.0 },
    Activity
);
fetch!(
    Executions,
    WorkPage,
    |s| Slice::Executions {
        status: s.status.clone(),
        page: s.page.clone(),
    },
    Executions
);

pub(crate) fn default_workspaces() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        current_id: "local".into(),
        workspaces: vec![Workspace {
            id: "local".into(),
            name: "World Wide Webb".into(),
            icon: None,
            color: None,
        }],
    }
}
fn default_product() -> Snapshot {
    Snapshot {
        conversations: vec![],
        turns: vec![],
        todos: vec![],
        settings: Default::default(),
    }
}
fn default_tickets() -> TicketSnapshot {
    TicketSnapshot {
        tickets: vec![],
        comments: vec![],
        runs: vec![],
        links: vec![],
        assignees: vec![Assignee {
            id: "owner".into(),
            name: "You".into(),
            kind: AssigneeKind::Human,
        }],
    }
}

struct Pending {
    command: serde_json::Value,
    envelope: Envelope,
}

pub struct Daemon {
    transport: Arc<dyn Transport>,
    // Serialize mutations and refreshes so an older snapshot cannot replace a
    // newer acknowledgement. Never acquire this lock on the foreground.
    serial: Mutex<()>,
    pending: [Mutex<Option<Pending>>; 4],
    workspaces: Mutex<WorkspaceSnapshot>,
    product: Mutex<Snapshot>,
    tickets: Mutex<Arc<TicketSnapshot>>,
    automations: Mutex<AutomationSnapshot>,
    update_required: AtomicBool,
    #[cfg(any(test, feature = "fixtures"))]
    memory: Option<Arc<memory::MemoryTransport>>,
}
impl Daemon {
    /// The real daemon. Discovery, start-if-needed and authentication happen on
    /// the first request and again only after a transport failure.
    pub fn connect() -> Self {
        Self::with_transport(Arc::new(http::HttpTransport::default()))
    }
    #[cfg(any(test, feature = "fixtures"))]
    pub fn in_memory() -> Self {
        let memory = memory::MemoryTransport::new();
        Self {
            memory: Some(memory.clone()),
            ..Self::with_transport(memory)
        }
    }
    fn with_transport(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            serial: Mutex::new(()),
            pending: Default::default(),
            workspaces: Mutex::new(default_workspaces()),
            product: Mutex::new(default_product()),
            tickets: Mutex::new(Arc::new(default_tickets())),
            automations: Mutex::new(AutomationSnapshot {
                rules: vec![],
                occurrences: vec![],
                history: vec![],
            }),
            update_required: AtomicBool::new(false),
            #[cfg(any(test, feature = "fixtures"))]
            memory: None,
        }
    }
    /// The in-memory daemon behind this instance, for editing and inspection.
    #[cfg(any(test, feature = "fixtures"))]
    pub fn memory(&self) -> &memory::MemoryTransport {
        self.memory.as_deref().expect("an in-memory Daemon")
    }
    /// Whether the daemon has refused this client version. Set by the latest
    /// request; cleared when a later request succeeds.
    pub fn update_required(&self) -> bool {
        self.update_required.load(Ordering::Relaxed)
    }

    fn call(&self, request: Request) -> Result<Reply, DaemonError> {
        let result = self.transport.call(request);
        match &result {
            Ok(_) => self.update_required.store(false, Ordering::Relaxed),
            Err(DaemonError::UpdateRequired) => self.update_required.store(true, Ordering::Relaxed),
            Err(_) => {}
        }
        result
    }
    fn slot(&self, family: Family) -> &Mutex<Option<Pending>> {
        &self.pending[family as usize]
    }
    /// Replace one family's cached snapshot with the daemon's.
    fn reload(&self, family: Family) -> Result<(), DaemonError> {
        match family {
            Family::Workspace => {
                *self.workspaces.lock().expect("Workspace snapshot") = self.fetch(Workspaces)?
            }
            Family::Automation => {
                *self.automations.lock().expect("Automation snapshot") = self.fetch(Automations)?
            }
            Family::Ticket => {
                *self.tickets.lock().expect("Ticket snapshot") = Arc::new(self.fetch(Tickets)?)
            }
            Family::Product => {
                *self.product.lock().expect("product snapshot") = self.fetch(Product)?
            }
        }
        Ok(())
    }

    /// Reload every cached snapshot, after re-sending any unacknowledged command.
    pub fn refresh(&self) -> Result<(), DaemonError> {
        for family in Family::ALL {
            let envelope = self
                .slot(family)
                .lock()
                .expect("pending command")
                .as_ref()
                .map(|pending| pending.envelope.clone());
            if let Some(envelope) = envelope {
                return self.resend(family, envelope).map(|_| ());
            }
        }
        let _serial = self.serial.lock().expect("daemon requests");
        for family in Family::ALL {
            self.reload(family)?;
        }
        Ok(())
    }
    /// Read one slice of daemon state, bypassing the cache.
    pub fn fetch<F: Fetch>(&self, slice: F) -> Result<F::Snapshot, DaemonError> {
        let reply = self.call(Request::Fetch(slice.slice()))?;
        F::snapshot(reply).ok_or_else(|| DaemonError::Unavailable("Unexpected daemon reply".into()))
    }
    /// Send one command and refresh its family's snapshot once acknowledged.
    pub fn send<C: Command>(&self, command: C) -> Result<C::Ack, DaemonError> {
        let family = C::family();
        let value = serde_json::to_value(&command)
            .map_err(|error| DaemonError::Unavailable(error.to_string()))?;
        let envelope = {
            let mut slot = self.slot(family).lock().expect("pending command");
            match slot.as_ref() {
                Some(prior) if prior.command == value => prior.envelope.clone(),
                Some(_) => {
                    return Err(DaemonError::Rejected(ErrorBody {
                        code: ErrorCode::Conflict,
                        message: format!(
                            "Retry the unacknowledged {} change before another change.",
                            family.name()
                        ),
                    }));
                }
                None => {
                    let envelope = command.envelope(uuid::Uuid::new_v4().to_string());
                    *slot = Some(Pending {
                        command: value,
                        envelope: envelope.clone(),
                    });
                    envelope
                }
            }
        };
        let reply = self.resend(family, envelope)?;
        C::ack(reply).ok_or_else(|| DaemonError::Unavailable("Unexpected daemon reply".into()))
    }
    /// Send a pending envelope: the same operation id again on an ambiguous
    /// transport failure, never a second operation.
    fn resend(&self, family: Family, envelope: Envelope) -> Result<Reply, DaemonError> {
        let _serial = self.serial.lock().expect("daemon requests");
        let mut result = self.call(Request::Command(envelope.clone()));
        if matches!(result, Err(DaemonError::Retryable(_))) {
            result = self.call(Request::Command(envelope));
        }
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                if matches!(
                    error,
                    DaemonError::Rejected(_) | DaemonError::UpdateRequired
                ) {
                    *self.slot(family).lock().expect("pending command") = None;
                }
                return Err(error);
            }
        };
        self.reload(family)?;
        *self.slot(family).lock().expect("pending command") = None;
        Ok(reply)
    }

    /// Daemon-owned Codex sign-in state.
    pub fn connection_status(&self) -> Result<ConnectionStatus, DaemonError> {
        match self.call(Request::ConnectionStatus)? {
            Reply::ConnectionStatus(status) => Ok(status),
            _ => Err(DaemonError::Unavailable("Unexpected daemon reply".into())),
        }
    }
    pub fn connection(&self, action: ConnectionAction) -> Result<(), DaemonError> {
        self.call(Request::Connection(action)).map(|_| ())
    }

    pub fn workspaces(&self) -> WorkspaceSnapshot {
        self.workspaces.lock().expect("Workspace snapshot").clone()
    }
    pub fn product(&self) -> Snapshot {
        self.product.lock().expect("product snapshot").clone()
    }
    /// Shared, not copied: the palette reads it on every keystroke.
    pub fn tickets(&self) -> Arc<TicketSnapshot> {
        self.tickets.lock().expect("Ticket snapshot").clone()
    }
    pub fn automations(&self) -> AutomationSnapshot {
        self.automations
            .lock()
            .expect("Automation snapshot")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(title: &str) -> TicketCommand {
        TicketCommand::Create {
            title: title.into(),
        }
    }
    fn commands(daemon: &Daemon) -> Vec<String> {
        daemon
            .memory()
            .requests()
            .into_iter()
            .filter_map(|request| match request {
                Request::Command(envelope) => Some(envelope.operation_id().to_owned()),
                _ => None,
            })
            .collect()
    }
    fn fetches(daemon: &Daemon) -> Vec<Slice> {
        daemon
            .memory()
            .requests()
            .into_iter()
            .filter_map(|request| match request {
                Request::Fetch(slice) => Some(slice),
                _ => None,
            })
            .collect()
    }
    fn refused(code: ErrorCode) -> DaemonError {
        DaemonError::Rejected(ErrorBody {
            code,
            message: "refused".into(),
        })
    }

    #[test]
    fn a_command_is_one_request_and_one_fetch() {
        let daemon = Daemon::in_memory();
        let id = daemon.send(create("Pay rent")).unwrap();
        assert_eq!(id, Some(1));
        assert_eq!(commands(&daemon).len(), 1);
        assert_eq!(fetches(&daemon), vec![Slice::Tickets]);
        assert_eq!(daemon.tickets().tickets[0].title, "Pay rent");
    }

    #[test]
    fn a_transport_failure_retries_the_same_operation_once() {
        let daemon = Daemon::in_memory();
        daemon
            .memory()
            .fail_next(DaemonError::Retryable("connection reset".into()));
        daemon.send(create("Pay rent")).unwrap();
        let sent = commands(&daemon);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], sent[1]);
        assert_eq!(daemon.tickets().tickets.len(), 1);
    }

    #[test]
    fn a_rejection_clears_the_pending_slot() {
        let daemon = Daemon::in_memory();
        daemon.memory().fail_next(refused(ErrorCode::Invalid));
        assert_eq!(
            daemon.send(create("Pay rent")),
            Err(refused(ErrorCode::Invalid))
        );
        daemon.send(create("Walk the dog")).unwrap();
        let sent = commands(&daemon);
        assert_eq!(sent.len(), 2);
        assert_ne!(sent[0], sent[1], "a refused operation is not reused");
    }

    #[test]
    fn an_unacknowledged_command_blocks_a_different_one_until_retried() {
        let daemon = Daemon::in_memory();
        daemon
            .memory()
            .fail_next(DaemonError::Unavailable("daemon down".into()));
        assert!(matches!(
            daemon.send(create("Pay rent")),
            Err(DaemonError::Unavailable(_))
        ));
        let blocked = daemon.send(create("Walk the dog"));
        assert!(
            matches!(&blocked, Err(DaemonError::Rejected(body)) if body.code == ErrorCode::Conflict),
            "{blocked:?}"
        );
        daemon.send(create("Pay rent")).unwrap();
        let sent = commands(&daemon);
        assert_eq!(
            sent.len(),
            2,
            "the blocked command never reached the daemon"
        );
        assert_eq!(sent[0], sent[1], "the retry reuses the operation id");
        assert_eq!(daemon.tickets().tickets.len(), 1);
    }

    #[test]
    fn refresh_replays_an_unacknowledged_command_first() {
        let daemon = Daemon::in_memory();
        daemon
            .memory()
            .fail_next(DaemonError::Unavailable("daemon down".into()));
        let _ = daemon.send(create("Pay rent"));
        daemon.memory().clear_requests();
        daemon.refresh().unwrap();
        assert_eq!(commands(&daemon).len(), 1);
        assert_eq!(daemon.tickets().tickets.len(), 1);
        daemon.memory().clear_requests();
        daemon.refresh().unwrap();
        assert!(commands(&daemon).is_empty());
        assert_eq!(fetches(&daemon).len(), 4);
    }

    #[test]
    fn update_required_follows_the_latest_reply() {
        let daemon = Daemon::in_memory();
        assert!(!daemon.update_required());
        daemon.memory().fail_next(DaemonError::UpdateRequired);
        assert_eq!(
            daemon.fetch(Tickets).err(),
            Some(DaemonError::UpdateRequired)
        );
        assert!(daemon.update_required());
        daemon.fetch(Tickets).unwrap();
        assert!(!daemon.update_required());
    }

    #[test]
    fn families_do_not_block_each_other() {
        let daemon = Daemon::in_memory();
        daemon
            .memory()
            .fail_next(DaemonError::Unavailable("daemon down".into()));
        let _ = daemon.send(create("Pay rent"));
        let id = daemon
            .send(WorkspaceCommand::Create {
                name: "Home".into(),
                icon: None,
                color: None,
            })
            .unwrap();
        assert_eq!(daemon.workspaces().current_id, id);
    }
}
