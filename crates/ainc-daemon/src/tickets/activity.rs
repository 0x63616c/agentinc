//! A Ticket's history: who changed what, from which Conversation or run.
//! Entries commit in the same transaction as the change they describe.
use super::TicketStatus;
use crate::product::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivityKind {
    /// `to_value` is the first title.
    Created,
    /// Old and new title.
    Renamed,
    Described,
    /// Old and new status.
    Status,
    /// Old and new priority.
    Priority,
    /// Old and new assignee ID.
    Assigned,
    /// Old and new labels, comma-separated.
    Labels,
    /// `from_value` is the relation from this Ticket's side (`blocks`,
    /// `blocked_by`, `relates_to`, `duplicates`, `duplicated_by`, `parent_of`,
    /// `child_of`); `to_value` is the other Ticket's ID.
    Linked,
    Unlinked,
    /// `to_value` is the run state (`queued`, `completed`, `failed`,
    /// `cancelled`) and `run_id` names the run.
    Work,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct TicketActivity {
    pub id: i64,
    pub ticket_id: i64,
    pub actor_id: String,
    pub kind: ActivityKind,
    pub from_value: Option<String>,
    pub to_value: Option<String>,
    /// The Conversation that made the change through Evee, if any.
    pub conversation_id: Option<i64>,
    pub run_id: Option<String>,
    pub created_at: i64,
}

/// One entry to record.
pub(crate) struct Entry<'a> {
    ticket_id: i64,
    kind: ActivityKind,
    from: Option<String>,
    to: Option<String>,
    run_id: Option<&'a str>,
}
impl<'a> Entry<'a> {
    pub(crate) fn new(ticket_id: i64, kind: ActivityKind) -> Self {
        Self {
            ticket_id,
            kind,
            from: None,
            to: None,
            run_id: None,
        }
    }
    pub(crate) fn from(mut self, value: impl Into<String>) -> Self {
        self.from = Some(value.into());
        self
    }
    pub(crate) fn to(mut self, value: impl Into<String>) -> Self {
        self.to = Some(value.into());
        self
    }
    pub(crate) fn status(ticket_id: i64, from: TicketStatus, to: TicketStatus) -> Self {
        Self::new(ticket_id, ActivityKind::Status)
            .from(from.as_str())
            .to(to.as_str())
    }
    pub(crate) fn work(ticket_id: i64, run_id: &'a str, state: &str) -> Self {
        Self {
            run_id: Some(run_id),
            ..Self::new(ticket_id, ActivityKind::Work).to(state)
        }
    }
}

pub(crate) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    actor_id: &str,
    conversation: Option<i64>,
    entry: Entry<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO ticket_activity(ticket_id,actor_id,kind,from_value,to_value,conversation_id,run_id) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(entry.ticket_id)
        .bind(actor_id)
        .bind(entry.kind)
        .bind(entry.from)
        .bind(entry.to)
        .bind(conversation)
        .bind(entry.run_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn for_ticket(
    pool: &PgPool,
    workspace: &str,
    ticket_id: i64,
) -> Result<Vec<TicketActivity>, ApiError> {
    Ok(sqlx::query_as("SELECT a.id,a.ticket_id,a.actor_id,a.kind,a.from_value,a.to_value,a.conversation_id,a.run_id,a.created_at FROM ticket_activity a JOIN tickets t ON t.id=a.ticket_id WHERE t.workspace_id=$1 AND t.id=$2 ORDER BY a.id")
        .bind(workspace)
        .bind(ticket_id)
        .fetch_all(pool)
        .await?)
}
