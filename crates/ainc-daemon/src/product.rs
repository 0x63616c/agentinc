//! Single-owner product commands. A committed receipt is the acknowledgement;
//! clients never own SQL or the lifetime of accepted turns.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct Product {
    pub pool: PgPool,
    token: String,
}
impl Product {
    pub fn new(pool: PgPool, token: String) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !token.trim().is_empty(),
            "owner credential must not be empty"
        );
        Ok(Self { pool, token })
    }
    pub(crate) fn authorize(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        if headers.get("authorization").and_then(|h| h.to_str().ok())
            != Some(&format!("Bearer {}", self.token))
        {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Owner credential required.",
            ));
        }
        Ok(())
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
    CreateTodo {
        title: String,
    },
    CompleteTodo {
        id: i64,
        completed: bool,
    },
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
pub struct Acknowledgement {
    pub operation_id: String,
    pub result_id: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}
impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &str, message: &str) -> Self {
        Self {
            status,
            body: ErrorBody {
                code: code.into(),
                message: message.into(),
            },
        }
    }
    pub(crate) fn conflict() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "conflict",
            "The record changed or has a reply in progress. Refresh and try again.",
        )
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref e) = error {
            if e.is_unique_violation() || e.is_foreign_key_violation() {
                return Self::conflict();
            }
            if e.is_check_violation() {
                return Self::new(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    "The value is empty or too long.",
                );
            }
        }
        tracing::error!(%error, "product database request failed");
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Data is unavailable. Try again.",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/state", get(state))
        .route("/v1/commands", post(command))
        .with_state(product)
}

#[utoipa::path(get, path = "/v1/state", operation_id = "product_state", responses((status = 200, body = Snapshot), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
pub async fn state(
    State(product): State<Product>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, ApiError> {
    product.authorize(&headers)?;
    Ok(Json(snapshot(&product.pool).await?))
}

pub async fn snapshot(pool: &PgPool) -> Result<Snapshot, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let conversations = sqlx::query_as("SELECT c.id,c.title,COALESCE((SELECT COALESCE(response,prompt) FROM turns WHERE conversation_id=c.id ORDER BY id DESC LIMIT 1),'') AS snippet,to_char(to_timestamp(c.updated_at),'YYYY-MM-DD HH24:MI') AS updated,c.updated_at FROM conversations c WHERE workspace_id='local' ORDER BY updated_at DESC,id DESC").fetch_all(&mut *tx).await?;
    let turns = sqlx::query_as("SELECT t.id,conversation_id,prompt,response,error,state FROM turns t JOIN conversations c ON c.id=t.conversation_id WHERE c.workspace_id='local' ORDER BY t.id").fetch_all(&mut *tx).await?;
    let todos = sqlx::query_as("SELECT id,title,(status='done') AS completed FROM tickets WHERE workspace_id='local' ORDER BY (status='done'),id DESC").fetch_all(&mut *tx).await?;
    let model = sqlx::query_scalar(
        "SELECT value FROM assistant_settings WHERE workspace_id='local' AND key='model'",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let selected: Option<String> = sqlx::query_scalar("SELECT value FROM assistant_settings WHERE workspace_id='local' AND key='selected_conversation'").fetch_optional(&mut *tx).await?;
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

#[utoipa::path(post, path = "/v1/commands", operation_id = "product_command", request_body = CommandRequest, responses((status = 200, body = Acknowledgement), (status = 400, body = ErrorBody), (status = 401, body = ErrorBody), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
pub async fn command(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<CommandRequest>,
) -> Result<Json<Acknowledgement>, ApiError> {
    product.authorize(&headers)?;
    Ok(Json(execute(&product.pool, request).await?))
}

pub async fn execute(pool: &PgPool, request: CommandRequest) -> Result<Acknowledgement, ApiError> {
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_operation",
            "Use a UUID operation ID.",
        ));
    }
    let value = serde_json::to_value(&request.command).expect("serializable command");
    let mut tx = pool.begin().await?;
    // Serialize duplicate operation IDs before looking up their committed receipt.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&request.operation_id)
        .execute(&mut *tx)
        .await?;
    let receipt: Option<(serde_json::Value, Option<i64>)> =
        sqlx::query_as("SELECT command,result_id FROM command_receipts WHERE operation_id=$1")
            .bind(&request.operation_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some((prior, result_id)) = receipt {
        if prior != value {
            return Err(ApiError::conflict());
        }
        return Ok(Acknowledgement {
            operation_id: request.operation_id,
            result_id,
        });
    }
    let result_id = match request.command {
        Command::CreateConversation => Some(
            sqlx::query_scalar(
                "INSERT INTO conversations(title) VALUES ('New conversation') RETURNING id",
            )
            .fetch_one(&mut *tx)
            .await?,
        ),
        Command::RenameConversation { id, title } => {
            changed(
                sqlx::query(
                    "UPDATE conversations SET title=$2 WHERE id=$1 AND workspace_id='local'",
                )
                .bind(id)
                .bind(title.trim())
                .execute(&mut *tx)
                .await?
                .rows_affected(),
            )?;
            Some(id)
        }
        Command::DeleteConversation { id } => {
            // Lock the parent against send, retry and worker claims.
            let row: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM conversations WHERE id=$1 AND workspace_id='local' FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if row.is_none() {
                return Err(ApiError::conflict());
            }
            let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=$1 AND state IN ('queued','running'))").bind(id).fetch_one(&mut *tx).await?;
            if pending {
                return Err(ApiError::conflict());
            }
            sqlx::query("UPDATE conversation_sessions SET state='closed' WHERE conversation_id=$1 AND state='active'").bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM conversations WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            Some(id)
        }
        Command::SelectConversation { id } => {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=$1 AND workspace_id='local')",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                return Err(ApiError::conflict());
            }
            sqlx::query("INSERT INTO assistant_settings(key,value) VALUES ('selected_conversation',$1) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value").bind(id.to_string()).execute(&mut *tx).await?;
            Some(id)
        }
        Command::Send {
            conversation_id,
            prompt,
        } => {
            let parent: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM conversations WHERE id=$1 AND workspace_id='local' FOR UPDATE",
            )
            .bind(conversation_id)
            .fetch_optional(&mut *tx)
            .await?;
            if parent.is_none() {
                return Err(ApiError::conflict());
            }
            let id = sqlx::query_scalar("INSERT INTO turns(conversation_id,prompt,state,model) VALUES ($1,$2,'queued',(SELECT value FROM assistant_settings WHERE workspace_id='local' AND key='model')) RETURNING id").bind(conversation_id).bind(prompt.trim()).fetch_one(&mut *tx).await?;
            sqlx::query("UPDATE conversations SET updated_at=extract(epoch FROM now())::bigint,title=CASE WHEN title='New conversation' THEN left($2,60) ELSE title END WHERE id=$1").bind(conversation_id).bind(prompt.trim()).execute(&mut *tx).await?;
            Some(id)
        }
        Command::Retry { id } => {
            let parent: Option<i64> = sqlx::query_scalar("SELECT c.id FROM conversations c JOIN turns t ON c.id=t.conversation_id WHERE t.id=$1 AND c.workspace_id='local' FOR UPDATE OF c").bind(id).fetch_optional(&mut *tx).await?;
            if parent.is_none() {
                return Err(ApiError::conflict());
            }
            changed(
                sqlx::query(
                    "UPDATE turns SET error=NULL,response=NULL,state='queued',attempt=attempt+1,session_id=NULL WHERE id=$1 AND state='failed'",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?
                .rows_affected(),
            )?;
            Some(id)
        }
        Command::CreateTodo { title } => Some(
            sqlx::query_scalar("INSERT INTO tickets(title) VALUES ($1) RETURNING id")
                .bind(title.trim())
                .fetch_one(&mut *tx)
                .await?,
        ),
        Command::CompleteTodo { id, completed } => {
            changed(
                sqlx::query("UPDATE tickets SET status=CASE WHEN $2 THEN 'done' ELSE 'to_do' END,revision=revision+1 WHERE id=$1 AND workspace_id='local' AND assignee_kind='human'")
                    .bind(id)
                    .bind(completed)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected(),
            )?;
            Some(id)
        }
        Command::DeleteTodo { id } => {
            changed(
                sqlx::query("DELETE FROM tickets WHERE id=$1 AND workspace_id='local'")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected(),
            )?;
            Some(id)
        }
        Command::SelectModel { model } => {
            sqlx::query("INSERT INTO assistant_settings(key,value) VALUES ('model',$1) ON CONFLICT(workspace_id,key) DO UPDATE SET value=excluded.value").bind(model).execute(&mut *tx).await?;
            None
        }
    };
    sqlx::query("INSERT INTO command_receipts(operation_id,command,result_id) VALUES ($1,$2,$3)")
        .bind(&request.operation_id)
        .bind(value)
        .bind(result_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_notify('agentinc_turns','')")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Acknowledgement {
        operation_id: request.operation_id,
        result_id,
    })
}
fn changed(rows: u64) -> Result<(), ApiError> {
    if rows == 0 {
        Err(ApiError::conflict())
    } else {
        Ok(())
    }
}
