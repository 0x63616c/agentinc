//! Owner-managed workspaces. The legacy `local` identity remains stable.
use crate::product::{ApiError, ErrorBody, Product};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkspaceState {
    pub current_id: String,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceCommand {
    Create {
        name: String,
        icon: Option<String>,
        color: Option<String>,
    },
    Rename {
        id: String,
        name: String,
    },
    Switch {
        id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkspaceRequest {
    pub operation_id: String,
    pub command: WorkspaceCommand,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkspaceReceipt {
    pub result_id: String,
}

pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/workspaces", get(state))
        .route("/v1/workspaces/commands", post(command))
        .with_state(product)
}

#[utoipa::path(get, path="/v1/workspaces", operation_id="workspaces_state", responses((status=200, body=WorkspaceState), (status=401, body=ErrorBody), (status=503, body=ErrorBody)))]
pub async fn state(
    State(product): State<Product>,
    headers: HeaderMap,
) -> Result<Json<WorkspaceState>, ApiError> {
    product.authorize(&headers)?;
    Ok(Json(snapshot(&product.pool).await?))
}

pub async fn current(pool: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT workspace_id FROM selected_workspace WHERE owner_id='owner'")
        .fetch_one(pool)
        .await
}

pub async fn snapshot(pool: &PgPool) -> Result<WorkspaceState, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let current_id =
        sqlx::query_scalar("SELECT workspace_id FROM selected_workspace WHERE owner_id='owner'")
            .fetch_one(&mut *tx)
            .await?;
    let workspaces = sqlx::query_as("SELECT id,name,icon,color FROM workspaces ORDER BY CASE WHEN id='local' THEN 0 ELSE 1 END,name,id")
        .fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(WorkspaceState {
        current_id,
        workspaces,
    })
}

#[utoipa::path(post, path="/v1/workspaces/commands", operation_id="workspaces_command", request_body=WorkspaceRequest, responses((status=200, body=WorkspaceReceipt), (status=400, body=ErrorBody), (status=401, body=ErrorBody), (status=409, body=ErrorBody), (status=503, body=ErrorBody)))]
pub async fn command(
    State(product): State<Product>,
    headers: HeaderMap,
    Json(request): Json<WorkspaceRequest>,
) -> Result<Json<WorkspaceReceipt>, ApiError> {
    product.authorize(&headers)?;
    Ok(Json(execute(&product.pool, request).await?))
}

pub async fn execute(
    pool: &PgPool,
    request: WorkspaceRequest,
) -> Result<WorkspaceReceipt, ApiError> {
    if uuid::Uuid::parse_str(&request.operation_id).is_err() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_operation",
            "Use a UUID operation ID.",
        ));
    }
    let payload = serde_json::to_value(&request.command).expect("serializable command");
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("workspace/{}", request.operation_id))
        .execute(&mut *tx)
        .await?;
    let prior: Option<(serde_json::Value, String)> =
        sqlx::query_as("SELECT command,result_id FROM workspace_receipts WHERE operation_id=$1")
            .bind(&request.operation_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some((command, result_id)) = prior {
        if command != payload {
            return Err(ApiError::conflict());
        }
        return Ok(WorkspaceReceipt { result_id });
    }
    let result_id = match request.command {
        WorkspaceCommand::Create { name, icon, color } => {
            validate_name(&name)?;
            if icon
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.chars().count() > 4)
                || color.as_ref().is_some_and(|v| !is_color(v))
            {
                return Err(invalid());
            }
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO workspaces(id,name,icon,color) VALUES ($1,$2,$3,$4)")
                .bind(&id)
                .bind(name.trim())
                .bind(icon)
                .bind(color)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO principals(workspace_id,id,kind,name) VALUES ($1,'owner','human','You')")
                .bind(&id).execute(&mut *tx).await?;
            sqlx::query("UPDATE selected_workspace SET workspace_id=$1 WHERE owner_id='owner'")
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            id
        }
        WorkspaceCommand::Rename { id, name } => {
            validate_name(&name)?;
            let changed = sqlx::query("UPDATE workspaces SET name=$2 WHERE id=$1")
                .bind(&id)
                .bind(name.trim())
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if changed == 0 {
                return Err(ApiError::conflict());
            }
            id
        }
        WorkspaceCommand::Switch { id } => {
            let changed = sqlx::query("UPDATE selected_workspace SET workspace_id=$1 WHERE owner_id='owner' AND EXISTS(SELECT 1 FROM workspaces WHERE id=$1)")
                .bind(&id).execute(&mut *tx).await?.rows_affected();
            if changed == 0 {
                return Err(ApiError::conflict());
            }
            id
        }
    };
    sqlx::query("INSERT INTO workspace_receipts(operation_id,command,result_id) VALUES ($1,$2,$3)")
        .bind(request.operation_id)
        .bind(payload)
        .bind(&result_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(WorkspaceReceipt { result_id })
}

fn invalid() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid",
        "Use a name of 1 to 120 characters, up to four icon characters, and a #RRGGBB color.",
    )
}
fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.chars().count() > 120 {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn is_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{automations, product, tickets};

    fn request(command: WorkspaceCommand) -> WorkspaceRequest {
        WorkspaceRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }

    #[sqlx::test]
    async fn migrates_legacy_workspace_and_scopes_product_data(pool: PgPool) {
        let initial = snapshot(&pool).await.unwrap();
        assert_eq!(initial.current_id, "local");
        assert_eq!(initial.workspaces[0].name, "World Wide Webb");

        let conversation_operation = uuid::Uuid::new_v4().to_string();
        let conversation = product::execute(
            &pool,
            product::CommandRequest {
                operation_id: conversation_operation.clone(),
                command: product::Command::CreateConversation,
            },
        )
        .await
        .unwrap()
        .result_id
        .unwrap();
        product::execute(
            &pool,
            product::CommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: product::Command::SelectModel {
                    model: "original".into(),
                },
            },
        )
        .await
        .unwrap();

        let created = execute(
            &pool,
            request(WorkspaceCommand::Create {
                name: "Personal".into(),
                icon: Some("P".into()),
                color: Some("#123ABC".into()),
            }),
        )
        .await
        .unwrap()
        .result_id;
        assert_eq!(current(&pool).await.unwrap(), created);
        assert_eq!(snapshot(&pool).await.unwrap().workspaces.len(), 2);
        let fresh = product::snapshot_in(&pool, &created).await.unwrap();
        assert!(fresh.conversations.is_empty());
        assert!(fresh.todos.is_empty());
        assert!(fresh.settings.model.is_none());
        assert!(
            product::execute_in(
                &pool,
                &created,
                product::CommandRequest {
                    operation_id: conversation_operation,
                    command: product::Command::CreateConversation,
                }
            )
            .await
            .is_err()
        );
        assert_eq!(
            product::snapshot_in(&pool, "local")
                .await
                .unwrap()
                .conversations[0]
                .id,
            conversation
        );
        assert!(
            product::execute_in(
                &pool,
                &created,
                product::CommandRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    command: product::Command::SelectConversation { id: conversation },
                }
            )
            .await
            .is_err()
        );
        execute(
            &pool,
            request(WorkspaceCommand::Switch { id: "local".into() }),
        )
        .await
        .unwrap();
        assert_eq!(current(&pool).await.unwrap(), "local");
        assert_eq!(
            product::snapshot_in(&pool, "local")
                .await
                .unwrap()
                .settings
                .model
                .as_deref(),
            Some("original")
        );
    }

    #[sqlx::test]
    async fn tickets_agents_and_automations_stay_in_their_workspace(pool: PgPool) {
        let created = execute(
            &pool,
            request(WorkspaceCommand::Create {
                name: "Other".into(),
                icon: None,
                color: None,
            }),
        )
        .await
        .unwrap()
        .result_id;
        let actor = tickets::Actor::owner_in(created.clone());
        tickets::execute(
            &pool,
            &actor,
            tickets::TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: tickets::TicketCommand::RegisterAgent {
                    name: "Other agent".into(),
                    instructions: "Work".into(),
                    model: "fixture".into(),
                },
            },
        )
        .await
        .unwrap();
        let agent: String = sqlx::query_scalar(
            "SELECT id FROM principals WHERE workspace_id=$1 AND name='Other agent'",
        )
        .bind(&created)
        .fetch_one(&pool)
        .await
        .unwrap();
        tickets::execute(
            &pool,
            &actor,
            tickets::TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: tickets::TicketCommand::Create {
                    title: "Other ticket".into(),
                },
            },
        )
        .await
        .unwrap();
        automations::execute(
            &pool,
            &actor,
            automations::AutomationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command: automations::AutomationCommand::Save {
                    id: None,
                    revision: None,
                    name: "Other rule".into(),
                    proposal: tickets::TicketProposal {
                        title: "Scheduled".into(),
                        agent_id: agent,
                    },
                    every_minutes: 60,
                },
            },
        )
        .await
        .unwrap();
        assert_eq!(
            tickets::snapshot(&pool, &actor)
                .await
                .unwrap()
                .tickets
                .len(),
            1
        );
        assert_eq!(
            automations::snapshot(&pool, &actor)
                .await
                .unwrap()
                .rules
                .len(),
            1
        );
        assert!(
            tickets::snapshot(&pool, &tickets::Actor::owner())
                .await
                .unwrap()
                .tickets
                .is_empty()
        );
        assert!(
            tickets::snapshot(&pool, &tickets::Actor::owner())
                .await
                .unwrap()
                .assignees
                .iter()
                .all(|agent| agent.name != "Other agent")
        );
        assert!(
            automations::snapshot(&pool, &tickets::Actor::owner())
                .await
                .unwrap()
                .rules
                .is_empty()
        );
    }
}
