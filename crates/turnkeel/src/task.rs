//! Durable one-off tasks: work with effects outside an agent, run until it
//! succeeds or fails for good. Definitions and effects remain owned by the caller.
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One task, identified by a stable caller-owned ID. Starting the same ID
/// again attaches to the first start; retries keep the same ID and input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub input: Value,
}

/// Why a task attempt did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskError {
    /// Try again later, with backoff.
    Retry(String),
    /// Stop: the task can never succeed, or is no longer wanted.
    Permanent(String),
}

/// Runs tasks. Attempts can repeat after a crash or a `Retry`, so the effect
/// must be idempotent under the task ID.
pub trait TaskHandler: Send + Sync + 'static {
    fn run(&self, task: Task) -> BoxFuture<'static, Result<(), TaskError>>;
}
