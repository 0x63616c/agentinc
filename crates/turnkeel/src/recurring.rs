//! Durable recurring actions. Definitions and effects remain owned by the caller.
use crate::Error;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// A recurring action, identified by a stable caller-owned ID.
#[derive(Clone, Debug)]
pub struct RecurringRule {
    pub id: String,
    pub every: Duration,
    pub paused: bool,
    pub input: Value,
}
/// One durable invocation. Retries retain the same ID and input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Occurrence {
    pub id: String,
    pub input: Value,
}
/// Implement an idempotent action and wait until its work is finished. While the
/// future is pending, subsequent scheduled invocations are skipped. The action
/// can run again after a crash; retain effect receipts under the occurrence ID.
pub trait RecurringAction: Send + Sync + 'static {
    fn execute(&self, occurrence: Occurrence) -> BoxFuture<'static, Result<(), Error>>;
}
/// Observed recurring-action history, including invocations awaiting a worker.
#[derive(Clone, Debug)]
pub struct RecurringState {
    pub paused: bool,
    pub missed: i64,
    pub overlap_skipped: i64,
    pub recent: Vec<OccurrenceRecord>,
}
#[derive(Clone, Debug)]
pub struct OccurrenceRecord {
    pub id: String,
    pub scheduled_at: i64,
}
