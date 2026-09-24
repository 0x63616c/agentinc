//! Tickets, Comments and dispatch intent commit at one authorization boundary.
use crate::product::{ApiError, ErrorBody, Product};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TicketStatus {
    Backlog,
    ToDo,
    InProgress,
    Done,
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
    pub status: TicketStatus,
    pub assignee_kind: AssigneeKind,
    pub assignee_id: String,
    pub revision: i64,
    pub generation: i64,
}
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
    pub tickets: Vec<Ticket>,
    pub comments: Vec<Comment>,
    pub assignees: Vec<Assignee>,
    pub runs: Vec<WorkRun>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TicketCommand {
    Create {
        title: String,
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
    SetStatus {
        id: i64,
        revision: i64,
        status: TicketStatus,
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
}
impl Actor {
    pub(crate) fn owner() -> Self {
        Self {
            workspace: "local".into(),
            id: "owner".into(),
            assignment: None,
        }
    }
}
fn denied() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "forbidden",
        "This command is outside the current assignment.",
    )
}
fn invalid(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
}
fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

async fn authorize(product: &Product, headers: &HeaderMap) -> Result<Actor, ApiError> {
    if product.authorize(headers).is_ok() {
        return Ok(Actor::owner());
    }
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(denied)?;
    let row:Option<(String,String,i64,i64)> = sqlx::query_as("SELECT c.workspace_id,r.agent_id,r.ticket_id,r.generation FROM agent_credentials c JOIN ticket_runs r ON r.run_id=c.run_id JOIN tickets t ON t.id=r.ticket_id WHERE c.token_hash=$1 AND t.workspace_id=c.workspace_id AND t.generation=r.generation AND t.assignee_kind='agent' AND t.assignee_id=r.agent_id AND r.state IN ('queued','running')")
        .bind(token_hash(token)).fetch_optional(&product.pool).await?;
    let (workspace, id, ticket, generation) = row.ok_or_else(denied)?;
    Ok(Actor {
        workspace,
        id,
        assignment: Some((ticket, generation)),
    })
}

pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/tickets", get(state))
        .route("/v1/tickets/commands", post(command))
        .with_state(product)
}
#[utoipa::path(get,path="/v1/tickets",operation_id="tickets_state",responses((status=200,body=TicketSnapshot),(status=403,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn state(
    State(product): State<Product>,
    headers: HeaderMap,
) -> Result<Json<TicketSnapshot>, ApiError> {
    let actor = authorize(&product, &headers).await?;
    Ok(Json(snapshot(&product.pool, &actor).await?))
}
#[utoipa::path(post,path="/v1/tickets/commands",operation_id="tickets_command",request_body=TicketCommandRequest,responses((status=200,body=TicketReceipt),(status=400,body=ErrorBody),(status=403,body=ErrorBody),(status=409,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn command(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<TicketCommandRequest>,
) -> Result<Json<TicketReceipt>, ApiError> {
    let actor = authorize(&product, &headers).await?;
    Ok(Json(execute(&product.pool, &actor, request).await?))
}

pub(crate) async fn snapshot(pool: &PgPool, actor: &Actor) -> Result<TicketSnapshot, ApiError> {
    let ticket = actor.assignment.map(|a| a.0);
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    if let Some((id, generation)) = actor.assignment {
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tickets t JOIN ticket_runs r ON r.ticket_id=t.id AND r.generation=t.generation WHERE t.id=$1 AND t.workspace_id=$2 AND t.generation=$3 AND t.assignee_id=$4 AND t.assignee_kind='agent' AND r.state IN ('queued','running'))").bind(id).bind(&actor.workspace).bind(generation).bind(&actor.id).fetch_one(&mut *tx).await?;
        if !active {
            return Err(denied());
        }
    }
    let tickets=sqlx::query_as("SELECT id,title,status,assignee_kind,assignee_id,revision,generation FROM tickets WHERE workspace_id=$1 AND ($2::bigint IS NULL OR id=$2) ORDER BY id DESC").bind(&actor.workspace).bind(ticket).fetch_all(&mut *tx).await?;
    let comments=sqlx::query_as("SELECT c.id,c.ticket_id,c.author_id,c.body,c.created_at FROM comments c JOIN tickets t ON t.id=c.ticket_id WHERE t.workspace_id=$1 AND ($2::bigint IS NULL OR t.id=$2) ORDER BY c.id").bind(&actor.workspace).bind(ticket).fetch_all(&mut *tx).await?;
    let assignees = sqlx::query_as(
        "SELECT id,name,kind FROM principals WHERE workspace_id=$1 ORDER BY kind,name",
    )
    .bind(&actor.workspace)
    .fetch_all(&mut *tx)
    .await?;
    let runs=sqlx::query_as("SELECT r.run_id,r.ticket_id,r.generation,r.state,r.error FROM ticket_runs r JOIN tickets t ON t.id=r.ticket_id WHERE t.workspace_id=$1 AND ($2::bigint IS NULL OR t.id=$2) ORDER BY r.ticket_id,r.generation").bind(&actor.workspace).bind(ticket).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(TicketSnapshot {
        tickets,
        comments,
        assignees,
        runs,
    })
}

async fn lock_ticket(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: i64,
    revision: Option<i64>,
) -> Result<Ticket, ApiError> {
    if actor.assignment.is_some_and(|a| a.0 != id) {
        return Err(denied());
    }
    let ticket:Ticket=sqlx::query_as("SELECT id,title,status,assignee_kind,assignee_id,revision,generation FROM tickets WHERE id=$1 AND workspace_id=$2 FOR UPDATE")
        .bind(id).bind(&actor.workspace).fetch_optional(&mut **tx).await?.ok_or_else(denied)?;
    if actor.assignment.is_some_and(|(_, generation)| {
        ticket.generation != generation
            || ticket.assignee_kind != AssigneeKind::Agent
            || ticket.assignee_id != actor.id
    }) {
        return Err(denied());
    }
    if actor.assignment.is_some() {
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ticket_runs WHERE ticket_id=$1 AND generation=$2 AND agent_id=$3 AND state IN ('queued','running'))").bind(id).bind(ticket.generation).bind(&actor.id).fetch_one(&mut **tx).await?;
        if !active {
            return Err(denied());
        }
    }
    if revision.is_some_and(|revision| ticket.revision != revision) {
        return Err(ApiError::conflict());
    }
    Ok(ticket)
}

pub(crate) async fn execute(
    pool: &PgPool,
    actor: &Actor,
    request: TicketCommandRequest,
) -> Result<TicketReceipt, ApiError> {
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(invalid("Use a UUID operation ID."));
    }
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
    let payload =
        serde_json::to_value(&request.command).map_err(|_| invalid("Invalid command."))?;
    let mut tx = pool.begin().await?;
    // Fence even receipt reads after reassignment, within the same transaction.
    if let Some((id, _)) = actor.assignment {
        lock_ticket(&mut tx, actor, id, None).await?;
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "{}/{}/{}",
            actor.workspace, actor.id, request.operation_id
        ))
        .execute(&mut *tx)
        .await?;
    let prior:Option<(Value,Option<i64>)>=sqlx::query_as("SELECT command,result_id FROM ticket_receipts WHERE workspace_id=$1 AND actor_id=$2 AND operation_id=$3")
        .bind(&actor.workspace).bind(&actor.id).bind(&request.operation_id).fetch_optional(&mut *tx).await?;
    if let Some((old, result_id)) = prior {
        if old != payload {
            return Err(ApiError::conflict());
        }
        return Ok(TicketReceipt {
            operation_id: request.operation_id,
            result_id,
        });
    }
    let result_id = match request.command {
        TicketCommand::Create { title } => Some(
            sqlx::query_scalar(
                "INSERT INTO tickets(workspace_id,title) VALUES ($1,$2) RETURNING id",
            )
            .bind(&actor.workspace)
            .bind(title.trim())
            .fetch_one(&mut *tx)
            .await?,
        ),
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
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO agents(workspace_id,id,instructions,model) VALUES ($1,$2,$3,$4)",
            )
            .bind(&actor.workspace)
            .bind(id)
            .bind(instructions)
            .bind(model.trim())
            .execute(&mut *tx)
            .await?;
            None
        }
        TicketCommand::AddComment { ticket_id, body } => {
            lock_ticket(&mut tx, actor, ticket_id, None).await?;
            Some(
                sqlx::query_scalar(
                    "INSERT INTO comments(ticket_id,author_id,body) VALUES ($1,$2,$3) RETURNING id",
                )
                .bind(ticket_id)
                .bind(&actor.id)
                .bind(body.trim())
                .fetch_one(&mut *tx)
                .await?,
            )
        }
        TicketCommand::Delete { id, revision } => {
            lock_ticket(&mut tx, actor, id, Some(revision)).await?;
            let worked: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ticket_runs WHERE ticket_id=$1)")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            if worked {
                return Err(ApiError::conflict());
            }
            sqlx::query("DELETE FROM tickets WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            Some(id)
        }
        TicketCommand::Rename {
            id,
            revision,
            title,
        } => {
            lock_ticket(&mut tx, actor, id, Some(revision)).await?;
            sqlx::query("UPDATE tickets SET title=$2,revision=revision+1 WHERE id=$1")
                .bind(id)
                .bind(title.trim())
                .execute(&mut *tx)
                .await?;
            Some(id)
        }
        TicketCommand::SetStatus {
            id,
            revision,
            status,
        } => {
            let ticket = lock_ticket(&mut tx, actor, id, Some(revision)).await?;
            if ticket.status != status {
                if actor.assignment.is_some() && ticket.status != TicketStatus::InProgress {
                    return Err(denied());
                }
                if actor.assignment.is_none() {
                    cancel_generation(&mut tx, &ticket).await?;
                }
                sqlx::query("UPDATE tickets SET status=$2,revision=revision+1,generation=generation+$3 WHERE id=$1")
                    .bind(id)
                    .bind(status)
                    .bind(i64::from(actor.assignment.is_none()))
                    .execute(&mut *tx)
                    .await?;
                if actor.assignment.is_none()
                    && matches!(status, TicketStatus::ToDo | TicketStatus::InProgress)
                    && ticket.assignee_kind == AssigneeKind::Agent
                {
                    start_generation(&mut tx, actor, id).await?;
                }
            }
            Some(id)
        }
        TicketCommand::Assign {
            id,
            revision,
            assignee_kind,
            assignee_id,
        } => {
            let ticket = lock_ticket(&mut tx, actor, id, Some(revision)).await?;
            cancel_generation(&mut tx, &ticket).await?;
            sqlx::query("UPDATE tickets SET assignee_kind=$2,assignee_id=$3,revision=revision+1,generation=generation+1,status=CASE WHEN status='in_progress' THEN 'to_do' ELSE status END WHERE id=$1").bind(id).bind(assignee_kind).bind(assignee_id).execute(&mut *tx).await?;
            if assignee_kind == AssigneeKind::Agent
                && matches!(ticket.status, TicketStatus::ToDo | TicketStatus::InProgress)
            {
                start_generation(&mut tx, actor, id).await?;
            }
            Some(id)
        }
        TicketCommand::Cancel { id, revision } => {
            let ticket = lock_ticket(&mut tx, actor, id, Some(revision)).await?;
            cancel_generation(&mut tx, &ticket).await?;
            sqlx::query("UPDATE tickets SET generation=generation+1,revision=revision+1,status=CASE WHEN status='in_progress' THEN 'to_do' ELSE status END WHERE id=$1").bind(id).execute(&mut *tx).await?;
            Some(id)
        }
    };
    sqlx::query("INSERT INTO ticket_receipts(workspace_id,actor_id,operation_id,command,result_id) VALUES ($1,$2,$3,$4,$5)").bind(&actor.workspace).bind(&actor.id).bind(&request.operation_id).bind(payload).bind(result_id).execute(&mut *tx).await?;
    sqlx::query("SELECT pg_notify('agentinc_dispatch','')")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(TicketReceipt {
        operation_id: request.operation_id,
        result_id,
    })
}
async fn cancel_generation(
    tx: &mut Transaction<'_, Postgres>,
    ticket: &Ticket,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO dispatch_outbox(ticket_id,generation,action,run_id) SELECT ticket_id,generation,'cancel',run_id FROM ticket_runs WHERE ticket_id=$1 AND generation=$2 AND state IN ('queued','running') ON CONFLICT DO NOTHING").bind(ticket.id).bind(ticket.generation).execute(&mut **tx).await?;
    sqlx::query("UPDATE ticket_runs SET state='cancelled' WHERE ticket_id=$1 AND generation=$2 AND state IN ('queued','running')").bind(ticket.id).bind(ticket.generation).execute(&mut **tx).await?;
    Ok(())
}
async fn start_generation(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: i64,
) -> Result<(), ApiError> {
    // A fresh assignment owns immutable prompt/model/definition snapshots.
    let run_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ticket_runs(run_id,ticket_id,generation,agent_id,model,instructions,prompt,state) SELECT $1,t.id,t.generation,a.id,a.model,a.instructions,t.title,'queued' FROM tickets t JOIN agents a ON a.workspace_id=t.workspace_id AND a.id=t.assignee_id WHERE t.id=$2 AND t.workspace_id=$3").bind(&run_id).bind(id).bind(&actor.workspace).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO dispatch_outbox(ticket_id,generation,action,run_id) SELECT ticket_id,generation,'start',run_id FROM ticket_runs WHERE run_id=$1").bind(run_id).execute(&mut **tx).await?;
    Ok(())
}
