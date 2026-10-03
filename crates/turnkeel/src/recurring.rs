//! Durable recurring actions. Definitions and effects remain owned by the caller.
use crate::Error;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// A recurring action, identified by a stable caller-owned ID.
#[derive(Clone, Debug)]
pub struct RecurringRule {
    /// The caller-owned ID that identifies this rule.
    pub id: String,
    /// How often the action should run.
    pub every: Duration,
    /// Whether the rule is paused and should not start new occurrences.
    pub paused: bool,
    /// The input passed to every occurrence.
    pub input: Value,
}
/// One durable invocation. Retries retain the same ID and input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Occurrence {
    /// The ID of this occurrence, stable across retries.
    pub id: String,
    /// The rule's input, passed unchanged.
    pub input: Value,
}
/// Implement an idempotent action and wait until its work is finished. While the
/// future is pending, subsequent scheduled invocations are skipped. The action
/// can run again after a crash; retain effect receipts under the occurrence ID.
pub trait RecurringAction: Send + Sync + 'static {
    /// Runs the action for one occurrence, resolving when its work is finished.
    fn execute(&self, occurrence: Occurrence) -> BoxFuture<'static, Result<(), Error>>;
}
/// Observed recurring-action history, including invocations awaiting a worker.
#[derive(Clone, Debug)]
pub struct RecurringState {
    /// Whether the rule is currently paused.
    pub paused: bool,
    /// How many scheduled occurrences were missed.
    pub missed: i64,
    /// How many occurrences were skipped because the previous one was still running.
    pub overlap_skipped: i64,
    /// The most recent occurrences.
    pub recent: Vec<OccurrenceRecord>,
}
#[derive(Clone, Debug)]
/// One occurrence of a recurring action, as recorded in its history.
pub struct OccurrenceRecord {
    /// The ID of the occurrence.
    pub id: String,
    /// When the occurrence was scheduled, in milliseconds since the Unix epoch.
    pub scheduled_at: i64,
}
