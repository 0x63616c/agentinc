//! Single-owner product commands. A committed receipt is the acknowledgement;
//! clients never own SQL or the lifetime of accepted turns.
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

pub use crate::api::{ErrorBody, Product};
pub use crate::conversations::{Conversation, Settings, Turn};
use crate::{
    api::{AppState, CommandError, Owner},
    conversations::{self, ConversationCommand, ConversationCommandRequest, ConversationSnapshot},
    receipts::OperationId,
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

#[allow(deprecated)]
pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

/// Deprecated: read `/v1/conversations` and `/v1/tickets`. Removed in a later release.
#[deprecated(note = "read /v1/conversations and /v1/tickets")]
#[utoipa::path(get, path = "/v1/state", operation_id = "product_state", responses((status = 200, body = Snapshot)))]
async fn state(
    State(product): State<Product>,
    owner: Owner,
) -> Result<Json<Snapshot>, CommandError> {
    Ok(Json(snapshot_in(&product.pool, &owner.workspace).await?))
}

pub async fn snapshot_in(pool: &PgPool, workspace: &str) -> Result<Snapshot, sqlx::Error> {
    let mut tx = crate::pg::snapshot_tx(pool).await?;
    let ConversationSnapshot {
        conversations,
        turns,
        settings,
    } = conversations::read(&mut tx, workspace).await?;
    let todos = sqlx::query_as("SELECT id,title,(status IN ('done','cancelled')) AS completed FROM tickets WHERE workspace_id=$1 ORDER BY (status IN ('done','cancelled')),id DESC").bind(workspace).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Snapshot {
        conversations,
        turns,
        todos,
        settings,
    })
}

/// Deprecated: send Conversation commands to `/v1/conversations/commands` and Ticket
/// commands to `/v1/tickets/commands`. Removed in a later release.
#[deprecated(note = "send to /v1/conversations/commands or /v1/tickets/commands")]
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

pub async fn execute_in(
    pool: &PgPool,
    workspace: &str,
    request: CommandRequest,
) -> Result<CommandReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let command = match conversation_command(request.command.clone()) {
        Ok(command) => {
            let receipt = conversations::execute(
                pool,
                workspace,
                ConversationCommandRequest {
                    operation_id: request.operation_id,
                    command,
                },
            )
            .await?;
            return Ok(CommandReceipt {
                operation_id: receipt.operation_id,
                result_id: receipt.result_id,
            });
        }
        Err(todo) => todo,
    };
    let mut tx = pool.begin().await?;
    let command = todo_as_ticket(&mut tx, workspace, &command)
        .await?
        .expect("every non-Conversation command is a Todo command");
    let receipt = tickets::execute_in(
        &mut tx,
        &Actor::owner_in(workspace.to_owned()),
        TicketCommandRequest {
            operation_id: operation_id.to_string(),
            command,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(CommandReceipt {
        operation_id: receipt.operation_id,
        result_id: receipt.result_id,
    })
}

/// The Conversation command a legacy command stands for; the Todo commands come back
/// as the error.
#[allow(deprecated)]
fn conversation_command(command: Command) -> Result<ConversationCommand, Command> {
    Ok(match command {
        Command::CreateConversation => ConversationCommand::Create,
        Command::RenameConversation { id, title } => ConversationCommand::Rename { id, title },
        Command::DeleteConversation { id } => ConversationCommand::Delete { id },
        Command::SelectConversation { id } => ConversationCommand::Select { id },
        Command::Send {
            conversation_id,
            prompt,
        } => ConversationCommand::Send {
            conversation_id,
            prompt,
        },
        Command::Retry { id } => ConversationCommand::Retry { id },
        Command::SelectModel { model } => ConversationCommand::SelectModel { model },
        todo @ (Command::CreateTodo { .. }
        | Command::CompleteTodo { .. }
        | Command::DeleteTodo { .. }) => return Err(todo),
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
