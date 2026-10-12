//! An in-memory daemon for tests: the same [`Transport`] seam as HTTP, state
//! you can edit directly, scripted failures, and a log of every request.
use super::transport::{DaemonError, Envelope, Reply, Request, Slice, Transport};
use ainc_client::types::{
    CalendarCommand, CalendarEvent, CalendarSnapshot, ErrorBody, ErrorCode, Todo, TodoCommand,
    TodoSnapshot, Workspace, WorkspaceCommand, WorkspaceSnapshot,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// Everything the in-memory daemon knows.
pub struct State {
    pub workspaces: WorkspaceSnapshot,
    pub todos: TodoSnapshot,
    pub calendar: CalendarSnapshot,
}
impl Default for State {
    fn default() -> Self {
        Self {
            workspaces: super::default_workspaces(),
            todos: TodoSnapshot { todos: vec![] },
            calendar: CalendarSnapshot { events: vec![] },
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
fn rejected(code: ErrorCode, message: &str) -> DaemonError {
    DaemonError::Rejected(ErrorBody {
        code,
        message: message.into(),
    })
}
fn next_id(ids: impl Iterator<Item = i64>) -> i64 {
    ids.max().unwrap_or(0) + 1
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
            Request::Fetch(Slice::Todos) => Ok(Reply::Todos(state.todos.clone())),
            Request::Fetch(Slice::Calendar) => Ok(Reply::Calendar(state.calendar.clone())),
            Request::Command(Envelope::Todo(request)) => {
                let todos = &mut state.todos.todos;
                let id = match request.command {
                    TodoCommand::Add { title } => {
                        let title = title.trim().to_owned();
                        if title.is_empty() {
                            return Err(rejected(ErrorCode::Invalid, "A Todo needs a title."));
                        }
                        let id = next_id(todos.iter().map(|t| t.id));
                        todos.push(Todo {
                            id,
                            title,
                            completed: false,
                        });
                        id
                    }
                    TodoCommand::Complete { id, completed } => {
                        let todo = todos
                            .iter_mut()
                            .find(|t| t.id == id)
                            .ok_or_else(|| rejected(ErrorCode::NotFound, "Unknown Todo"))?;
                        todo.completed = completed;
                        id
                    }
                    TodoCommand::Delete { id } => {
                        let before = todos.len();
                        todos.retain(|t| t.id != id);
                        if todos.len() == before {
                            return Err(rejected(ErrorCode::NotFound, "Unknown Todo"));
                        }
                        id
                    }
                };
                Ok(Reply::Acknowledgement(id))
            }
            Request::Command(Envelope::Calendar(request)) => {
                let events = &mut state.calendar.events;
                let id = match request.command {
                    CalendarCommand::Create {
                        title,
                        starts_at,
                        ends_at,
                    } => {
                        let title = title.trim().to_owned();
                        if title.is_empty() {
                            return Err(rejected(ErrorCode::Invalid, "An event needs a title."));
                        }
                        if ends_at < starts_at {
                            return Err(rejected(
                                ErrorCode::Invalid,
                                "An event ends after it starts.",
                            ));
                        }
                        let id = next_id(events.iter().map(|e| e.id));
                        events.push(CalendarEvent {
                            id,
                            title,
                            starts_at,
                            ends_at,
                        });
                        events.sort_by_key(|e| (e.starts_at, e.id));
                        id
                    }
                    CalendarCommand::Delete { id } => {
                        let before = events.len();
                        events.retain(|e| e.id != id);
                        if events.len() == before {
                            return Err(rejected(ErrorCode::NotFound, "Unknown event"));
                        }
                        id
                    }
                };
                Ok(Reply::Acknowledgement(id))
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
                            .ok_or_else(|| rejected(ErrorCode::NotFound, "Unknown workspace"))?;
                        workspace.name = name;
                        id
                    }
                    WorkspaceCommand::Switch { id } => {
                        if !workspaces.workspaces.iter().any(|item| item.id == id) {
                            return Err(rejected(ErrorCode::NotFound, "Unknown workspace"));
                        }
                        workspaces.current_id = id.clone();
                        id
                    }
                };
                Ok(Reply::Receipt(id))
            }
        }
    }
}
