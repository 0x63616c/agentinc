//! Tickets, Comments and dispatch intent commit at one authorization boundary.
//! Board ordering, relationships and history live in the submodules; every
//! mutation still enters through [`execute_in`].
mod activity;
mod board;
mod fence;
mod links;
mod transition;

use crate::api::Product;
use crate::{
    api::{AppState, CommandError, ErrorBody},
    receipts::{self, OperationId, Scope},
};
pub use activity::{ActivityKind, TicketActivity};
pub(crate) use activity::{Entry, record};
use axum::{
    Json,
    extract::{Path, State},
};
pub(crate) use board::{enter_column, lock as lock_board};
pub(crate) use fence::LiveAssignment;
pub use links::{LinkKind, TicketLink};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Where a Ticket sits on the board. Only To do and In progress dispatch an
/// agent assignee; the other four stop any live work.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TicketStatus {
    Backlog,
    ToDo,
    InProgress,
    Blocked,
    Done,
    Cancelled,
}
impl TicketStatus {
    /// Board order, left to right.
    pub const ALL: [Self; 6] = [
        Self::Backlog,
        Self::ToDo,
        Self::InProgress,
        Self::Blocked,
        Self::Done,
        Self::Cancelled,
    ];
    /// An agent assignee works on a Ticket only in these statuses.
    pub fn actionable(self) -> bool {
        matches!(self, Self::ToDo | Self::InProgress)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::ToDo => "to_do",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TicketPriority {
    Urgent,
    High,
    Medium,
    Low,
    None,
}
impl TicketPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Urgent => "urgent",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AssigneeKind {
    Human,
    Agent,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Ticket {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: TicketStatus,
    pub priority: TicketPriority,
    pub labels: Vec<String>,
    pub assignee_kind: AssigneeKind,
    pub assignee_id: String,
    /// Order inside its status column, lowest first.
    pub position: i64,
    pub revision: i64,
    pub generation: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// The Conversation that created this Ticket, if any.
    pub conversation_id: Option<i64>,
}
const TICKET_COLUMNS: &str = "id,title,description,status,priority,labels,assignee_kind,assignee_id,position,revision,generation,created_at,updated_at,conversation_id";
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Comment {
    pub id: i64,
    pub ticket_id: i64,
    pub author_id: String,
    pub body: String,
    pub created_at: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Assignee {
    pub id: String,
    pub name: String,
    pub kind: AssigneeKind,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct WorkRun {
    pub run_id: String,
    pub ticket_id: i64,
    pub generation: i64,
    pub state: String,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TicketSnapshot {
    /// Board order: by status column, then position.
    pub tickets: Vec<Ticket>,
    pub comments: Vec<Comment>,
    pub assignees: Vec<Assignee>,
    pub runs: Vec<WorkRun>,
    pub links: Vec<TicketLink>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TicketCommand {
    CreateAssigned {
        proposal: TicketProposal,
    },
    Create {
        title: String,
    },
    /// Create with any of the board's fields. Omitted fields take their defaults:
    /// To do, no priority, no labels, assigned to the owner. Assigning an agent
    /// in To do or In progress starts its work.
    CreateDetailed {
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        status: Option<TicketStatus>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        priority: Option<TicketPriority>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        labels: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        assignee_id: Option<String>,
    },
    Delete {
        id: i64,
        revision: i64,
    },
    Rename {
        id: i64,
        revision: i64,
        title: String,
    },
    Describe {
        id: i64,
        revision: i64,
        description: String,
    },
    SetStatus {
        id: i64,
        revision: i64,
        status: TicketStatus,
    },
    SetPriority {
        id: i64,
        revision: i64,
        priority: TicketPriority,
    },
    /// Replace the Ticket's labels: up to ten, 1–32 characters, no commas.
    SetLabels {
        id: i64,
        revision: i64,
        labels: Vec<String>,
    },
    /// Put a Ticket into `status` directly below `after` (a Ticket already in
    /// that column), or at the top of the column when `after` is absent.
    /// Changing status has the same effect on agent work as `set_status`.
    Move {
        id: i64,
        revision: i64,
        status: TicketStatus,
        #[serde(default)]
        after: Option<i64>,
    },
    Assign {
        id: i64,
        revision: i64,
        assignee_kind: AssigneeKind,
        assignee_id: String,
    },
    Cancel {
        id: i64,
        revision: i64,
    },
    /// `from_id` blocks, relates to, duplicates or is the parent of `to_id`.
    Link {
        from_id: i64,
        to_id: i64,
        link: LinkKind,
    },
    Unlink {
        from_id: i64,
        to_id: i64,
        link: LinkKind,
    },
    AddComment {
        ticket_id: i64,
        body: String,
    },
    RegisterAgent {
        name: String,
        instructions: String,
        model: String,
    },
}
/// A bounded proposal to create one actionable Ticket for a registered agent.
/// The command boundary authorizes and commits it with a durable receipt.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TicketProposal {
    pub title: String,
    pub agent_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TicketCommandRequest {
    pub operation_id: String,
    pub command: TicketCommand,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TicketReceipt {
    pub operation_id: String,
    pub result_id: Option<i64>,
}

#[derive(Clone)]
pub(crate) struct Actor {
    pub workspace: String,
    pub id: String,
    pub assignment: Option<(i64, i64)>,
    /// The Conversation acting through Evee's tools, recorded in history.
    pub conversation: Option<i64>,
}
impl Actor {
    pub(crate) fn owner() -> Self {
        Self {
            workspace: "local".into(),
            id: "owner".into(),
            assignment: None,
            conversation: None,
        }
    }
    pub(crate) fn owner_in(workspace: String) -> Self {
        Self {
            workspace,
            ..Self::owner()
        }
    }
    async fn log(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entry: Entry<'_>,
    ) -> Result<(), CommandError> {
        record(tx, &self.id, self.conversation, entry).await?;
        Ok(())
    }
}
fn denied() -> CommandError {
    CommandError::Forbidden
}
fn invalid(message: &str) -> CommandError {
    CommandError::Invalid(message.into())
}
fn clean_title(title: &str) -> Result<String, CommandError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 500 {
        return Err(invalid("Use a title of 1–500 characters."));
    }
    Ok(title.to_owned())
}
fn clean_description(description: &str) -> Result<String, CommandError> {
    let description = description.trim();
    if description.chars().count() > 32000 {
        return Err(invalid("Use a description of 32,000 characters or fewer."));
    }
    Ok(description.to_owned())
}
/// Trimmed, deduplicated case-insensitively in the order given.
fn clean_labels(labels: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut clean: Vec<String> = Vec::new();
    for label in labels {
        let label = label.trim();
        if label.is_empty() || label.chars().count() > 32 || label.contains(',') {
            return Err(invalid("Use labels of 1–32 characters without commas."));
        }
        if !clean.iter().any(|seen| seen.eq_ignore_ascii_case(label)) {
            clean.push(label.to_owned());
        }
    }
    if clean.len() > 10 {
        return Err(invalid("Use ten labels or fewer."));
    }
    Ok(clean)
}

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
        .routes(routes!(history))
}
#[utoipa::path(get,path="/v1/tickets",operation_id="tickets_state",responses((status=200,body=TicketSnapshot),(status=403,body=ErrorBody)))]
async fn state(
    State(product): State<Product>,
    actor: Actor,
) -> Result<Json<TicketSnapshot>, CommandError> {
    Ok(Json(snapshot(&product.pool, &actor).await?))
}
#[utoipa::path(post,path="/v1/tickets/commands",operation_id="tickets_command",request_body=TicketCommandRequest,responses((status=200,body=TicketReceipt),(status=400,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody),(status=409,body=ErrorBody)))]
async fn command(
    State(product): State<Product>,
    actor: Actor,
    Json(request): Json<TicketCommandRequest>,
) -> Result<Json<TicketReceipt>, CommandError> {
    Ok(Json(execute(&product.pool, &actor, request).await?))
}
/// One Ticket's history, oldest first. Comments stay in the snapshot.
#[utoipa::path(get,path="/v1/tickets/{id}/activity",operation_id="tickets_activity",params(("id" = i64, Path, description = "Ticket ID")),responses((status=200,body=Vec<TicketActivity>),(status=403,body=ErrorBody)))]
async fn history(
    State(product): State<Product>,
    actor: Actor,
    Path(id): Path<i64>,
) -> Result<Json<Vec<TicketActivity>>, CommandError> {
    if actor.assignment.is_some_and(|(ticket, _)| ticket != id) {
        return Err(denied());
    }
    let mut tx = product.pool.begin().await?;
    fence_read(&mut tx, &actor).await?;
    let history = activity::for_ticket(&mut tx, &actor.workspace, id).await?;
    tx.commit().await?;
    Ok(Json(history))
}

/// An agent reads only while its assignment's run is live; a stale credential
/// sees nothing, in the same transaction as the read.
async fn fence_read(tx: &mut Transaction<'_, Postgres>, actor: &Actor) -> Result<(), CommandError> {
    if actor.assignment.is_some() {
        LiveAssignment::check(tx, actor).await?;
    }
    Ok(())
}

pub(crate) async fn snapshot(pool: &PgPool, actor: &Actor) -> Result<TicketSnapshot, CommandError> {
    let ticket = actor.assignment.map(|a| a.0);
    let mut tx = crate::pg::snapshot_tx(pool).await?;
    fence_read(&mut tx, actor).await?;
    let tickets = sqlx::query_as(&format!("SELECT {TICKET_COLUMNS} FROM tickets WHERE workspace_id=$1 AND ($2::bigint IS NULL OR id=$2) ORDER BY array_position($3::text[],status),position,id DESC"))
        .bind(&actor.workspace)
        .bind(ticket)
        .bind(TicketStatus::ALL.map(TicketStatus::as_str))
        .fetch_all(&mut *tx)
        .await?;
    let comments=sqlx::query_as("SELECT c.id,c.ticket_id,c.author_id,c.body,c.created_at FROM comments c JOIN tickets t ON t.id=c.ticket_id WHERE t.workspace_id=$1 AND ($2::bigint IS NULL OR t.id=$2) ORDER BY c.id").bind(&actor.workspace).bind(ticket).fetch_all(&mut *tx).await?;
    let assignees = sqlx::query_as(
        "SELECT id,name,kind FROM principals WHERE workspace_id=$1 ORDER BY kind,name",
    )
    .bind(&actor.workspace)
    .fetch_all(&mut *tx)
    .await?;
    let runs=sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.state,r.error FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id WHERE t.workspace_id=$1 AND ($2::bigint IS NULL OR t.id=$2) ORDER BY r.ticket_id,r.generation").bind(&actor.workspace).bind(ticket).fetch_all(&mut *tx).await?;
    let links = links::in_workspace(&mut tx, &actor.workspace, ticket).await?;
    tx.commit().await?;
    Ok(TicketSnapshot {
        tickets,
        comments,
        assignees,
        runs,
        links,
    })
}

async fn lock_ticket(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: i64,
    revision: Option<i64>,
) -> Result<Ticket, CommandError> {
    let ticket = if actor.assignment.is_some() {
        // An agent touches only the Ticket it is assigned to, while that assignment is live.
        let live = LiveAssignment::lock(tx, actor).await?;
        if live.ticket.id != id {
            return Err(denied());
        }
        live.ticket
    } else {
        sqlx::query_as(&format!(
            "SELECT {TICKET_COLUMNS} FROM tickets WHERE id=$1 AND workspace_id=$2 FOR UPDATE"
        ))
        .bind(id)
        .bind(&actor.workspace)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CommandError::NotFound)?
    };
    if revision.is_some_and(|revision| ticket.revision != revision) {
        return Err(CommandError::conflict());
    }
    Ok(ticket)
}

pub(crate) async fn execute(
    pool: &PgPool,
    actor: &Actor,
    request: TicketCommandRequest,
) -> Result<TicketReceipt, CommandError> {
    let mut tx = pool.begin().await?;
    let receipt = execute_in(&mut tx, actor, request).await?;
    tx.commit().await?;
    Ok(receipt)
}

pub(crate) async fn execute_in(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    request: TicketCommandRequest,
) -> Result<TicketReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    if actor.assignment.is_some()
        && !matches!(
            request.command,
            TicketCommand::AddComment { .. }
                | TicketCommand::SetStatus {
                    status: TicketStatus::Done,
                    ..
                }
        )
    {
        return Err(denied());
    }
    // Every command may move Tickets or relate them; one writer per board.
    board::lock(tx, &actor.workspace).await?;
    // Fence even receipt reads after reassignment, within the same transaction.
    if let Some((id, _)) = actor.assignment {
        lock_ticket(tx, actor, id, None).await?;
    }
    let scope = Scope::Ticket {
        workspace: actor.workspace.clone(),
        actor: actor.id.clone(),
    };
    let TicketCommandRequest { command, .. } = request;
    let payload = serde_json::to_value(&command).map_err(|_| invalid("Invalid command."))?;
    let actor = actor.clone();
    let receipt = receipts::execute(tx, scope, operation_id, &payload, |tx| {
        Box::pin(async move {
            let result_id = apply(tx, &actor, command).await?;
            sqlx::query("SELECT pg_notify($1,'')")
                .bind(crate::pg::coordination::DISPATCH)
                .execute(&mut **tx)
                .await?;
            Ok(result_id)
        })
    })
    .await?;
    Ok(TicketReceipt {
        operation_id: receipt.operation_id.to_string(),
        result_id: receipt.result,
    })
}

/// A new Ticket's fields after validation.
struct NewTicket {
    title: String,
    description: String,
    status: TicketStatus,
    priority: TicketPriority,
    labels: Vec<String>,
    assignee_kind: AssigneeKind,
    assignee_id: String,
}
async fn create(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    ticket: NewTicket,
) -> Result<i64, CommandError> {
    let dispatch = ticket.assignee_kind == AssigneeKind::Agent && ticket.status.actionable();
    let position = board::top(tx, &actor.workspace, ticket.status).await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO tickets(workspace_id,title,description,status,priority,labels,assignee_kind,assignee_id,generation,conversation_id,position) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id")
        .bind(&actor.workspace)
        .bind(&ticket.title)
        .bind(&ticket.description)
        .bind(ticket.status)
        .bind(ticket.priority)
        .bind(&ticket.labels)
        .bind(ticket.assignee_kind)
        .bind(&ticket.assignee_id)
        .bind(i64::from(dispatch))
        .bind(actor.conversation)
        .bind(position)
        .fetch_one(&mut **tx)
        .await?;
    actor
        .log(tx, Entry::new(id, ActivityKind::Created).to(ticket.title))
        .await?;
    if dispatch {
        start_generation(tx, actor, id).await?;
    }
    Ok(id)
}

async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    command: TicketCommand,
) -> Result<Option<i64>, CommandError> {
    Ok(match command {
        TicketCommand::CreateAssigned { proposal } => {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM agents WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(&actor.workspace)
            .bind(&proposal.agent_id)
            .fetch_one(&mut **tx)
            .await?;
            if !valid || proposal.title.trim().is_empty() || proposal.title.chars().count() > 500 {
                return Err(invalid(
                    "Choose a registered agent and a title of 1–500 characters.",
                ));
            }
            let ticket = NewTicket {
                title: proposal.title.trim().to_owned(),
                description: String::new(),
                status: TicketStatus::ToDo,
                priority: TicketPriority::None,
                labels: vec![],
                assignee_kind: AssigneeKind::Agent,
                assignee_id: proposal.agent_id,
            };
            Some(create(tx, actor, ticket).await?)
        }
        TicketCommand::Create { title } => {
            let ticket = NewTicket {
                title: clean_title(&title)?,
                description: String::new(),
                status: TicketStatus::ToDo,
                priority: TicketPriority::None,
                labels: vec![],
                assignee_kind: AssigneeKind::Human,
                assignee_id: "owner".into(),
            };
            Some(create(tx, actor, ticket).await?)
        }
        TicketCommand::CreateDetailed {
            title,
            description,
            status,
            priority,
            labels,
            assignee_id,
        } => {
            let (assignee_kind, assignee_id) = match assignee_id {
                Some(id) => {
                    let kind: Option<AssigneeKind> = sqlx::query_scalar(
                        "SELECT kind FROM principals WHERE workspace_id=$1 AND id=$2",
                    )
                    .bind(&actor.workspace)
                    .bind(&id)
                    .fetch_optional(&mut **tx)
                    .await?;
                    (
                        kind.ok_or_else(|| invalid("Choose an assignee in this workspace."))?,
                        id,
                    )
                }
                None => (AssigneeKind::Human, "owner".into()),
            };
            let ticket = NewTicket {
                title: clean_title(&title)?,
                description: clean_description(description.as_deref().unwrap_or_default())?,
                status: status.unwrap_or(TicketStatus::ToDo),
                priority: priority.unwrap_or(TicketPriority::None),
                labels: clean_labels(labels.unwrap_or_default())?,
                assignee_kind,
                assignee_id,
            };
            Some(create(tx, actor, ticket).await?)
        }
        TicketCommand::RegisterAgent {
            name,
            instructions,
            model,
        } => {
            if instructions.len() > 32000 {
                return Err(invalid("Instructions are too long."));
            }
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO principals(workspace_id,id,kind,name) VALUES ($1,$2,'agent',$3)",
            )
            .bind(&actor.workspace)
            .bind(&id)
            .bind(name.trim())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO agents(workspace_id,id,instructions,model) VALUES ($1,$2,$3,$4)",
            )
            .bind(&actor.workspace)
            .bind(id)
            .bind(instructions)
            .bind(model.trim())
            .execute(&mut **tx)
            .await?;
            None
        }
        TicketCommand::AddComment { ticket_id, body } => {
            lock_ticket(tx, actor, ticket_id, None).await?;
            Some(
                sqlx::query_scalar(
                    "INSERT INTO comments(ticket_id,author_id,body) VALUES ($1,$2,$3) RETURNING id",
                )
                .bind(ticket_id)
                .bind(&actor.id)
                .bind(body.trim())
                .fetch_one(&mut **tx)
                .await?,
            )
        }
        TicketCommand::Delete { id, revision } => {
            lock_ticket(tx, actor, id, Some(revision)).await?;
            let worked: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ticket_runs WHERE ticket_id=$1)")
                    .bind(id)
                    .fetch_one(&mut **tx)
                    .await?;
            if worked {
                return Err(CommandError::Conflict(
                    "A Ticket that has been worked on keeps its history.".into(),
                ));
            }
            links::detach_all(tx, actor, id).await?;
            sqlx::query("DELETE FROM tickets WHERE id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
            Some(id)
        }
        TicketCommand::Rename {
            id,
            revision,
            title,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            let title = clean_title(&title)?;
            if title != ticket.title {
                sqlx::query("UPDATE tickets SET title=$2,revision=revision+1 WHERE id=$1")
                    .bind(id)
                    .bind(&title)
                    .execute(&mut **tx)
                    .await?;
                actor
                    .log(
                        tx,
                        Entry::new(id, ActivityKind::Renamed)
                            .from(ticket.title)
                            .to(title),
                    )
                    .await?;
            }
            Some(id)
        }
        TicketCommand::Describe {
            id,
            revision,
            description,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            let description = clean_description(&description)?;
            if description != ticket.description {
                sqlx::query("UPDATE tickets SET description=$2,revision=revision+1 WHERE id=$1")
                    .bind(id)
                    .bind(description)
                    .execute(&mut **tx)
                    .await?;
                actor
                    .log(tx, Entry::new(id, ActivityKind::Described))
                    .await?;
            }
            Some(id)
        }
        TicketCommand::SetStatus {
            id,
            revision,
            status,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            change_status(tx, actor, &ticket, status).await?;
            Some(id)
        }
        TicketCommand::SetPriority {
            id,
            revision,
            priority,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            if priority != ticket.priority {
                sqlx::query("UPDATE tickets SET priority=$2,revision=revision+1 WHERE id=$1")
                    .bind(id)
                    .bind(priority)
                    .execute(&mut **tx)
                    .await?;
                actor
                    .log(
                        tx,
                        Entry::new(id, ActivityKind::Priority)
                            .from(ticket.priority.as_str())
                            .to(priority.as_str()),
                    )
                    .await?;
            }
            Some(id)
        }
        TicketCommand::SetLabels {
            id,
            revision,
            labels,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            let labels = clean_labels(labels)?;
            if labels != ticket.labels {
                sqlx::query("UPDATE tickets SET labels=$2,revision=revision+1 WHERE id=$1")
                    .bind(id)
                    .bind(&labels)
                    .execute(&mut **tx)
                    .await?;
                actor
                    .log(
                        tx,
                        Entry::new(id, ActivityKind::Labels)
                            .from(ticket.labels.join(","))
                            .to(labels.join(",")),
                    )
                    .await?;
            }
            Some(id)
        }
        TicketCommand::Move {
            id,
            revision,
            status,
            after,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            change_status(tx, actor, &ticket, status).await?;
            board::place(tx, &actor.workspace, id, status, after).await?;
            Some(id)
        }
        TicketCommand::Assign {
            id,
            revision,
            assignee_kind,
            assignee_id,
        } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            cancel_generation(tx, actor, &ticket).await?;
            sqlx::query("UPDATE tickets SET assignee_kind=$2,assignee_id=$3,revision=revision+1,generation=generation+1 WHERE id=$1").bind(id).bind(assignee_kind).bind(&assignee_id).execute(&mut **tx).await?;
            // Stopped work goes back to To do, at the top like any status change.
            if ticket.status == TicketStatus::InProgress {
                board::enter_column(tx, id, TicketStatus::ToDo).await?;
            }
            actor
                .log(
                    tx,
                    Entry::new(id, ActivityKind::Assigned)
                        .from(ticket.assignee_id)
                        .to(assignee_id),
                )
                .await?;
            if ticket.status == TicketStatus::InProgress {
                actor
                    .log(tx, Entry::status(id, ticket.status, TicketStatus::ToDo))
                    .await?;
            }
            if assignee_kind == AssigneeKind::Agent && ticket.status.actionable() {
                start_generation(tx, actor, id).await?;
            }
            Some(id)
        }
        TicketCommand::Cancel { id, revision } => {
            let ticket = lock_ticket(tx, actor, id, Some(revision)).await?;
            cancel_generation(tx, actor, &ticket).await?;
            sqlx::query(
                "UPDATE tickets SET generation=generation+1,revision=revision+1 WHERE id=$1",
            )
            .bind(id)
            .execute(&mut **tx)
            .await?;
            if ticket.status == TicketStatus::InProgress {
                board::enter_column(tx, id, TicketStatus::ToDo).await?;
            }
            if ticket.status == TicketStatus::InProgress {
                actor
                    .log(tx, Entry::status(id, ticket.status, TicketStatus::ToDo))
                    .await?;
            }
            Some(id)
        }
        TicketCommand::Link {
            from_id,
            to_id,
            link,
        } => {
            links::link(tx, actor, from_id, to_id, link).await?;
            Some(from_id)
        }
        TicketCommand::Unlink {
            from_id,
            to_id,
            link,
        } => {
            links::unlink(tx, actor, from_id, to_id, link).await?;
            Some(from_id)
        }
    })
}

/// The one status transition. The owner's change stops live work and starts a
/// fresh generation when the new status is actionable for an agent; an agent may
/// only finish the In progress work it holds. The Ticket moves to the top of its
/// new column.
async fn change_status(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    ticket: &Ticket,
    status: TicketStatus,
) -> Result<(), CommandError> {
    let Some(plan) = transition::plan(
        actor.assignment.is_none(),
        ticket.status,
        status,
        ticket.assignee_kind,
    )
    .map_err(|transition::Refused| denied())?
    else {
        return Ok(());
    };
    if plan.cancel {
        cancel_generation(tx, actor, ticket).await?;
    }
    sqlx::query("UPDATE tickets SET revision=revision+1,generation=generation+$2 WHERE id=$1")
        .bind(ticket.id)
        .bind(i64::from(plan.bump_generation))
        .execute(&mut **tx)
        .await?;
    board::enter_column(tx, ticket.id, status).await?;
    actor
        .log(tx, Entry::status(ticket.id, ticket.status, status))
        .await?;
    if plan.start {
        start_generation(tx, actor, ticket.id).await?;
    }
    Ok(())
}
async fn cancel_generation(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    ticket: &Ticket,
) -> Result<(), CommandError> {
    sqlx::query("INSERT INTO dispatch_outbox(ticket_id,generation,action,run_id) SELECT ticket_id,generation,'cancel',run_id FROM ticket_runs WHERE ticket_id=$1 AND generation=$2 AND state IN ('queued','running') ON CONFLICT DO NOTHING").bind(ticket.id).bind(ticket.generation).execute(&mut **tx).await?;
    let cancelled: Vec<String> = sqlx::query_scalar("UPDATE ticket_runs SET state='cancelled' WHERE ticket_id=$1 AND generation=$2 AND state IN ('queued','running') RETURNING run_id").bind(ticket.id).bind(ticket.generation).fetch_all(&mut **tx).await?;
    for run_id in &cancelled {
        actor
            .log(tx, Entry::work(ticket.id, run_id, "cancelled"))
            .await?;
    }
    Ok(())
}
async fn start_generation(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: i64,
) -> Result<(), CommandError> {
    // A fresh assignment owns immutable prompt/model/definition snapshots.
    let run_id = uuid::Uuid::new_v4().to_string();
    let queued = sqlx::query("INSERT INTO ticket_runs(run_id,ticket_id,generation,agent_id,model,instructions,prompt,state) SELECT $1,t.id,t.generation,a.id,a.model,a.instructions,t.title||CASE WHEN t.description='' THEN '' ELSE E'\n\n'||t.description END,'queued' FROM tickets t JOIN agents a ON a.workspace_id=t.workspace_id AND a.id=t.assignee_id WHERE t.id=$2 AND t.workspace_id=$3").bind(&run_id).bind(id).bind(&actor.workspace).execute(&mut **tx).await?.rows_affected();
    // An agent principal without a definition has nothing to run.
    if queued == 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO dispatch_outbox(ticket_id,generation,action,run_id) SELECT ticket_id,generation,'start',run_id FROM ticket_runs WHERE run_id=$1").bind(&run_id).execute(&mut **tx).await?;
    actor.log(tx, Entry::work(id, &run_id, "queued")).await?;
    Ok(())
}
