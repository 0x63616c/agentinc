//! Single-owner product commands. A committed receipt is the acknowledgement;
//! clients never own SQL or the lifetime of accepted turns.
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

pub use crate::api::{ErrorBody, Product};
use crate::{
    api::{AppState, CommandError, Owner},
    receipts::{self, OperationId, Scope},
    tickets::{self, Actor, TicketCommand, TicketCommandRequest, TicketStatus},
};

/// Legacy name for [`CommandError`], kept for `temporal.rs` until WP-D5 lands.
pub(crate) type ApiError = CommandError;
impl CommandError {
    /// Legacy constructor for `temporal.rs`; new code names a variant.
    pub(crate) fn new(status: axum::http::StatusCode, _code: &str, message: &str) -> Self {
        use axum::http::StatusCode;
        match status {
            StatusCode::BAD_REQUEST => Self::Invalid(message.into()),
            StatusCode::CONFLICT => Self::Conflict(message.into()),
            StatusCode::UNAUTHORIZED => Self::Unauthorized,
            StatusCode::FORBIDDEN => Self::Forbidden,
            StatusCode::NOT_FOUND => Self::NotFound,
            StatusCode::SERVICE_UNAVAILABLE => Self::Unavailable(message.into()),
            _ => Self::Internal,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub updated: String,
    pub updated_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Turn {
    pub id: i64,
    pub conversation_id: i64,
    pub prompt: String,
    pub response: Option<String>,
    pub error: Option<String>,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Settings {
    pub model: Option<String>,
    pub selected_conversation: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Snapshot {
    pub conversations: Vec<Conversation>,
    pub turns: Vec<Turn>,
    pub todos: Vec<Todo>,
    pub settings: Settings,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    CreateConversation,
    RenameConversation {
        id: i64,
        title: String,
    },
    DeleteConversation {
        id: i64,
    },
    SelectConversation {
        id: i64,
    },
    Send {
        conversation_id: i64,
        prompt: String,
    },
    Retry {
        id: i64,
    },
    /// Phase-2 name for the Ticket command `create`.
    #[deprecated(note = "Use the Ticket command `create` at /v1/tickets/commands.")]
    CreateTodo {
        title: String,
    },
    /// Phase-2 name for the Ticket command `set_status` on a human-owned Ticket.
    #[deprecated(note = "Use the Ticket command `set_status` at /v1/tickets/commands.")]
    CompleteTodo {
        id: i64,
        completed: bool,
    },
    /// Phase-2 name for the Ticket command `delete`.
    #[deprecated(note = "Use the Ticket command `delete` at /v1/tickets/commands.")]
    DeleteTodo {
        id: i64,
    },
    SelectModel {
        model: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CommandRequest {
    pub operation_id: String,
    pub command: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CommandReceipt {
    pub operation_id: String,
    pub result_id: Option<i64>,
}

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

#[utoipa::path(get, path = "/v1/state", operation_id = "product_state", responses((status = 200, body = Snapshot)))]
async fn state(
    State(product): State<Product>,
    owner: Owner,
) -> Result<Json<Snapshot>, CommandError> {
    Ok(Json(snapshot_in(&product.pool, &owner.workspace).await?))
}

pub async fn snapshot(pool: &PgPool) -> Result<Snapshot, sqlx::Error> {
    snapshot_in(pool, "local").await
}

pub async fn snapshot_in(pool: &PgPool, workspace: &str) -> Result<Snapshot, sqlx::Error> {
    let mut tx = crate::pg::snapshot_tx(pool).await?;
    let conversations = sqlx::query_as("SELECT c.id,c.title,COALESCE((SELECT COALESCE(response,prompt) FROM turns WHERE conversation_id=c.id ORDER BY id DESC LIMIT 1),'') AS snippet,to_char(to_timestamp(c.updated_at),'YYYY-MM-DD HH24:MI') AS updated,c.updated_at FROM conversations c WHERE workspace_id=$1 ORDER BY updated_at DESC,id DESC").bind(workspace).fetch_all(&mut *tx).await?;
    let turns = sqlx::query_as("SELECT t.id,conversation_id,prompt,response,error,state FROM turns t JOIN conversations c ON c.id=t.conversation_id WHERE c.workspace_id=$1 ORDER BY t.id").bind(workspace).fetch_all(&mut *tx).await?;
    let todos = sqlx::query_as("SELECT id,title,(status IN ('done','cancelled')) AS completed FROM tickets WHERE workspace_id=$1 ORDER BY (status IN ('done','cancelled')),id DESC").bind(workspace).fetch_all(&mut *tx).await?;
    let model = sqlx::query_scalar(
        "SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='model'",
    )
    .bind(workspace)
    .fetch_optional(&mut *tx)
    .await?;
    let selected: Option<String> = sqlx::query_scalar("SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='selected_conversation'").bind(workspace).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    Ok(Snapshot {
        conversations,
        turns,
        todos,
        settings: Settings {
            model,
            selected_conversation: selected.and_then(|v| v.parse().ok()),
        },
    })
}

#[utoipa::path(post, path = "/v1/commands", operation_id = "product_command", request_body = CommandRequest, responses((status = 200, body = CommandReceipt), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
async fn command(
    State(product): State<Product>,
    owner: Owner,
    Json(request): Json<CommandRequest>,
) -> Result<Json<CommandReceipt>, CommandError> {
    Ok(Json(
        execute_in(&product.pool, &owner.workspace, request).await?,
    ))
}

pub async fn execute(
    pool: &PgPool,
    request: CommandRequest,
) -> Result<CommandReceipt, CommandError> {
    execute_in(pool, "local", request).await
}

pub async fn execute_in(
    pool: &PgPool,
    workspace: &str,
    request: CommandRequest,
) -> Result<CommandReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let mut tx = pool.begin().await?;
    if let Some(command) = todo_as_ticket(&mut tx, workspace, &request.command).await? {
        let receipt = tickets::execute_in(
            &mut tx,
            &Actor::owner_in(workspace.to_owned()),
            TicketCommandRequest {
                operation_id: request.operation_id,
                command,
            },
        )
        .await?;
        tx.commit().await?;
        return Ok(CommandReceipt {
            operation_id: receipt.operation_id,
            result_id: receipt.result_id,
        });
    }
    // Product IDs are one space across workspaces: reuse elsewhere conflicts.
    let payload = serde_json::json!({"workspace": workspace, "command": request.command});
    let CommandRequest { command, .. } = request;
    let workspace = workspace.to_string();
    let receipt = receipts::execute(&mut tx, Scope::Product, operation_id, &payload, |tx| {
        Box::pin(async move {
            let workspace = workspace.as_str();
            let result_id = match command {
                Command::CreateConversation => Some(
                    sqlx::query_scalar(
                        "INSERT INTO conversations(workspace_id,title) VALUES ($1,'New conversation') RETURNING id",
                    )
                    .bind(workspace)
                    .fetch_one(&mut **tx)
                    .await?,
                ),
                Command::RenameConversation { id, title } => {
                    changed(
                        sqlx::query(
                            "UPDATE conversations SET title=$2 WHERE id=$1 AND workspace_id=$3",
                        )
                        .bind(id)
                        .bind(title.trim())
                        .bind(workspace)
                        .execute(&mut **tx)
                        .await?
                        .rows_affected(),
                    )?;
                    Some(id)
                }
                Command::DeleteConversation { id } => {
                    // Lock the parent against send, retry and worker claims.
                    let row: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM conversations WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
                    )
                    .bind(id)
                    .bind(workspace)
                    .fetch_optional(&mut **tx)
                    .await?;
                    if row.is_none() {
                        return Err(CommandError::NotFound);
                    }
                    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=$1 AND state IN ('queued','running'))").bind(id).fetch_one(&mut **tx).await?;
                    if pending {
                        return Err(CommandError::Conflict("Wait for the reply in progress.".into()));
                    }
                    sqlx::query("UPDATE conversation_sessions SET state='closed' WHERE conversation_id=$1 AND state='active'").bind(id).execute(&mut **tx).await?;
                    sqlx::query("DELETE FROM conversations WHERE id=$1")
                        .bind(id)
                        .execute(&mut **tx)
                        .await?;
                    Some(id)
                }
                Command::SelectConversation { id } => {
                    let exists: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=$1 AND workspace_id=$2)",
                    )
                    .bind(id)
                    .bind(workspace)
                    .fetch_one(&mut **tx)
                    .await?;
                    if !exists {
                        return Err(CommandError::NotFound);
                    }
                    sqlx::query("INSERT INTO assistant_settings(workspace_id,key,value) VALUES ($1,'selected_conversation',$2) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value").bind(workspace).bind(id.to_string()).execute(&mut **tx).await?;
                    Some(id)
                }
                Command::Send {
                    conversation_id,
                    prompt,
                } => {
                    let parent: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM conversations WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
                    )
                    .bind(conversation_id)
                    .bind(workspace)
                    .fetch_optional(&mut **tx)
                    .await?;
                    if parent.is_none() {
                        return Err(CommandError::NotFound);
                    }
                    let id = sqlx::query_scalar("INSERT INTO turns(conversation_id,prompt,state,model) VALUES ($1,$2,'queued',(SELECT value FROM assistant_settings WHERE workspace_id=$3 AND key='model')) RETURNING id").bind(conversation_id).bind(prompt.trim()).bind(workspace).fetch_one(&mut **tx).await?;
                    sqlx::query("UPDATE conversations SET updated_at=extract(epoch FROM now())::bigint,title=CASE WHEN title='New conversation' THEN left($2,60) ELSE title END WHERE id=$1").bind(conversation_id).bind(prompt.trim()).execute(&mut **tx).await?;
                    Some(id)
                }
                Command::Retry { id } => {
                    let parent: Option<i64> = sqlx::query_scalar("SELECT c.id FROM conversations c JOIN turns t ON c.id=t.conversation_id WHERE t.id=$1 AND c.workspace_id=$2 FOR UPDATE OF c").bind(id).bind(workspace).fetch_optional(&mut **tx).await?;
                    if parent.is_none() {
                        return Err(CommandError::NotFound);
                    }
                    let retried = sqlx::query(
                        "UPDATE turns SET error=NULL,response=NULL,state='queued',attempt=attempt+1,session_id=NULL WHERE id=$1 AND state='failed'",
                    )
                    .bind(id)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected();
                    if retried == 0 {
                        return Err(CommandError::Conflict(
                            "Only a failed reply can be retried.".into(),
                        ));
                    }
                    Some(id)
                }
                #[allow(deprecated)]
                Command::CreateTodo { .. } | Command::CompleteTodo { .. } | Command::DeleteTodo { .. } => {
                    unreachable!("Todo commands take the Ticket path")
                }
                Command::SelectModel { model } => {
                    sqlx::query("INSERT INTO assistant_settings(workspace_id,key,value) VALUES ($1,'model',$2) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value").bind(workspace).bind(model).execute(&mut **tx).await?;
                    None
                }
            };
            sqlx::query("SELECT pg_notify('agentinc_turns','')")
                .execute(&mut **tx)
                .await?;
            Ok(result_id)
        })
    })
    .await?;
    tx.commit().await?;
    Ok(CommandReceipt {
        operation_id: receipt.operation_id.to_string(),
        result_id: receipt.result,
    })
}
/// The phase-2 Todo commands are Ticket commands for older clients, and take
/// the Ticket path: board lock, receipt, history and column placement. `None`
/// for every other command.
#[allow(deprecated)]
async fn todo_as_ticket(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: &str,
    command: &Command,
) -> Result<Option<TicketCommand>, CommandError> {
    Ok(Some(match command {
        Command::CreateTodo { title } => TicketCommand::Create {
            title: title.clone(),
        },
        Command::CompleteTodo { id, completed } => {
            // Legacy completion only touches human-owned Tickets, never an agent's work.
            let revision = todo_revision(tx, workspace, *id, true).await?;
            TicketCommand::SetStatus {
                id: *id,
                revision,
                status: if *completed {
                    TicketStatus::Done
                } else {
                    TicketStatus::ToDo
                },
            }
        }
        Command::DeleteTodo { id } => TicketCommand::Delete {
            id: *id,
            revision: todo_revision(tx, workspace, *id, false).await?,
        },
        _ => return Ok(None),
    }))
}
/// The Ticket's current revision, read under the board lock so the translated
/// command's revision check cannot race another writer.
async fn todo_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: &str,
    id: i64,
    human_only: bool,
) -> Result<i64, CommandError> {
    tickets::lock_board(tx, workspace).await?;
    sqlx::query_scalar("SELECT revision FROM tickets WHERE id=$1 AND workspace_id=$2 AND (NOT $3 OR assignee_kind='human')")
        .bind(id)
        .bind(workspace)
        .bind(human_only)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CommandError::NotFound)
}
/// An UPDATE that matched no row: the record is gone.
fn changed(rows: u64) -> Result<(), CommandError> {
    if rows == 0 {
        Err(CommandError::NotFound)
    } else {
        Ok(())
    }
}
