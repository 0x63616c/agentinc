//! Relationships between two Tickets in one workspace. Each is stored once from
//! its source; blocked-by, duplicated-by and sub-issue are the reverse reading.
use super::{ActivityKind, Actor, Entry, invalid, lock_ticket};
use crate::product::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LinkKind {
    /// The source must finish before the target can.
    Blocks,
    /// Symmetric; stored with the lower ID as the source.
    RelatesTo,
    /// The source repeats the target.
    Duplicates,
    /// The target is a sub-issue of the source. A Ticket has one parent.
    ParentOf,
}
impl LinkKind {
    /// How the relation reads from the source's and the target's side.
    fn sides(self) -> (&'static str, &'static str) {
        match self {
            Self::Blocks => ("blocks", "blocked_by"),
            Self::RelatesTo => ("relates_to", "relates_to"),
            Self::Duplicates => ("duplicates", "duplicated_by"),
            Self::ParentOf => ("parent_of", "child_of"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow, PartialEq, Eq)]
pub struct TicketLink {
    pub from_id: i64,
    pub to_id: i64,
    pub kind: LinkKind,
}

/// Links in the workspace, or only those touching one Ticket.
pub(super) async fn in_workspace(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    ticket: Option<i64>,
) -> Result<Vec<TicketLink>, sqlx::Error> {
    sqlx::query_as("SELECT from_id,to_id,kind FROM ticket_links WHERE workspace_id=$1 AND ($2::bigint IS NULL OR from_id=$2 OR to_id=$2) ORDER BY from_id,to_id,kind")
        .bind(workspace)
        .bind(ticket)
        .fetch_all(&mut **tx)
        .await
}

fn canonical(from: i64, to: i64, kind: LinkKind) -> (i64, i64) {
    if kind == LinkKind::RelatesTo && from > to {
        (to, from)
    } else {
        (from, to)
    }
}

/// Lock both Tickets in ID order, so two opposite links cannot deadlock.
async fn lock_pair(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    from: i64,
    to: i64,
) -> Result<(), ApiError> {
    if from == to {
        return Err(invalid("A Ticket cannot relate to itself."));
    }
    lock_ticket(tx, actor, from.min(to), None).await?;
    lock_ticket(tx, actor, from.max(to), None).await?;
    Ok(())
}

async fn touch_and_log(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    from: i64,
    to: i64,
    kind: LinkKind,
    activity: ActivityKind,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE tickets SET updated_at=extract(epoch FROM clock_timestamp())::bigint WHERE id=ANY($1)",
    )
    .bind([from, to])
    .execute(&mut **tx)
    .await?;
    let (source, target) = kind.sides();
    actor
        .log(
            tx,
            Entry::new(from, activity).from(source).to(to.to_string()),
        )
        .await?;
    actor
        .log(
            tx,
            Entry::new(to, activity).from(target).to(from.to_string()),
        )
        .await
}

pub(super) async fn link(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    from: i64,
    to: i64,
    kind: LinkKind,
) -> Result<(), ApiError> {
    let (from, to) = canonical(from, to, kind);
    lock_pair(tx, actor, from, to).await?;
    if kind != LinkKind::RelatesTo {
        // Refuse a loop such as A blocks B blocks A, or a Ticket inside its own subtree.
        let cycle: bool = sqlx::query_scalar("WITH RECURSIVE reach(id) AS (SELECT to_id FROM ticket_links WHERE from_id=$1 AND kind=$3 UNION SELECT l.to_id FROM ticket_links l JOIN reach r ON l.from_id=r.id AND l.kind=$3) SELECT EXISTS(SELECT 1 FROM reach WHERE id=$2)")
            .bind(to)
            .bind(from)
            .bind(kind)
            .fetch_one(&mut **tx)
            .await?;
        if cycle {
            return Err(invalid("That relationship would make a loop."));
        }
    }
    if kind == LinkKind::ParentOf {
        let parent: Option<i64> = sqlx::query_scalar(
            "SELECT from_id FROM ticket_links WHERE to_id=$1 AND kind='parent_of'",
        )
        .bind(to)
        .fetch_optional(&mut **tx)
        .await?;
        if parent.is_some_and(|parent| parent != from) {
            return Err(invalid("That Ticket already has a parent."));
        }
    }
    let inserted = sqlx::query("INSERT INTO ticket_links(workspace_id,from_id,to_id,kind) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(&actor.workspace)
        .bind(from)
        .bind(to)
        .bind(kind)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    if inserted > 0 {
        touch_and_log(tx, actor, from, to, kind, ActivityKind::Linked).await?;
    }
    Ok(())
}

pub(super) async fn unlink(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    from: i64,
    to: i64,
    kind: LinkKind,
) -> Result<(), ApiError> {
    let (from, to) = canonical(from, to, kind);
    lock_pair(tx, actor, from, to).await?;
    let deleted = sqlx::query(
        "DELETE FROM ticket_links WHERE workspace_id=$1 AND from_id=$2 AND to_id=$3 AND kind=$4",
    )
    .bind(&actor.workspace)
    .bind(from)
    .bind(to)
    .bind(kind)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if deleted > 0 {
        touch_and_log(tx, actor, from, to, kind, ActivityKind::Unlinked).await?;
    }
    Ok(())
}
