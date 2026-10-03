//! Conversations: a person talking to Evee. Commands and reads live here, the runner
//! turns queued Turns into SDK session work, and the tools give Evee the owner's own
//! Ticket and Automation commands.
mod commands;
mod runner;
mod tools;

use crate::{
    api::{AppState, CommandError, ErrorBody, Owner, Product},
    receipts::{self, OperationId, Scope},
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection, PgPool};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

pub use runner::Runner;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub snippet: String,
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
pub struct ConversationSnapshot {
    pub conversations: Vec<Conversation>,
    pub turns: Vec<Turn>,
    pub settings: Settings,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConversationCommand {
    Create,
    Rename {
        id: i64,
        title: String,
    },
    Delete {
        id: i64,
    },
    Select {
        id: i64,
    },
    Send {
        conversation_id: i64,
        prompt: String,
    },
    Retry {
        id: i64,
    },
    SelectModel {
        model: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ConversationCommandRequest {
    pub operation_id: String,
    pub command: ConversationCommand,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ConversationReceipt {
    pub operation_id: String,
    pub result_id: Option<i64>,
}

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

#[utoipa::path(get, path = "/v1/conversations", operation_id = "conversations_state", responses((status = 200, body = ConversationSnapshot)))]
async fn state(
    State(product): State<Product>,
    owner: Owner,
) -> Result<Json<ConversationSnapshot>, CommandError> {
    Ok(Json(snapshot(&product.pool, &owner.workspace).await?))
}

#[utoipa::path(post, path = "/v1/conversations/commands", operation_id = "conversations_command", request_body = ConversationCommandRequest, responses((status = 200, body = ConversationReceipt), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)))]
async fn command(
    State(product): State<Product>,
    owner: Owner,
    Json(request): Json<ConversationCommandRequest>,
) -> Result<Json<ConversationReceipt>, CommandError> {
    Ok(Json(
        execute(&product.pool, &owner.workspace, request).await?,
    ))
}

pub async fn snapshot(pool: &PgPool, workspace: &str) -> Result<ConversationSnapshot, sqlx::Error> {
    let mut tx = crate::pg::snapshot_tx(pool).await?;
    let snapshot = read(&mut tx, workspace).await?;
    tx.commit().await?;
    Ok(snapshot)
}

/// The Conversations half of a snapshot, read inside the caller's transaction.
pub(crate) async fn read(
    tx: &mut PgConnection,
    workspace: &str,
) -> Result<ConversationSnapshot, sqlx::Error> {
    let conversations = sqlx::query_as("SELECT c.id,c.title,COALESCE((SELECT COALESCE(response,prompt) FROM turns WHERE conversation_id=c.id ORDER BY id DESC LIMIT 1),'') AS snippet,c.updated_at FROM conversations c WHERE workspace_id=$1 ORDER BY updated_at DESC,id DESC").bind(workspace).fetch_all(&mut *tx).await?;
    let turns = sqlx::query_as("SELECT t.id,conversation_id,prompt,response,error,state FROM turns t JOIN conversations c ON c.id=t.conversation_id WHERE c.workspace_id=$1 ORDER BY t.id").bind(workspace).fetch_all(&mut *tx).await?;
    let model = sqlx::query_scalar(
        "SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='model'",
    )
    .bind(workspace)
    .fetch_optional(&mut *tx)
    .await?;
    let selected: Option<String> = sqlx::query_scalar("SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='selected_conversation'").bind(workspace).fetch_optional(&mut *tx).await?;
    Ok(ConversationSnapshot {
        conversations,
        turns,
        settings: Settings {
            model,
            selected_conversation: selected.and_then(|v| v.parse().ok()),
        },
    })
}

pub async fn execute(
    pool: &PgPool,
    workspace: &str,
    request: ConversationCommandRequest,
) -> Result<ConversationReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let mut tx = pool.begin().await?;
    // Operation IDs are one space across workspaces: reuse elsewhere conflicts.
    let payload = serde_json::json!({"workspace": workspace, "command": request.command});
    let command = request.command;
    let workspace = workspace.to_owned();
    let receipt = receipts::execute(&mut tx, Scope::Product, operation_id, &payload, |tx| {
        Box::pin(async move { commands::apply(tx, &workspace, command).await })
    })
    .await?;
    tx.commit().await?;
    Ok(ConversationReceipt {
        operation_id: receipt.operation_id.to_string(),
        result_id: receipt.result,
    })
}
