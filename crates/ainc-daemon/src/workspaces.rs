//! Owner-managed workspaces. The legacy `local` identity remains stable.
use crate::{
    api::{AppState, CommandError, ErrorBody, Owner, Product},
    receipts::{self, OperationId, Scope},
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct WorkspaceSnapshot {
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

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

#[utoipa::path(get, path="/v1/workspaces", operation_id="workspaces_state", responses((status=200, body=WorkspaceSnapshot)))]
async fn state(
    State(product): State<Product>,
    _owner: Owner,
) -> Result<Json<WorkspaceSnapshot>, CommandError> {
    Ok(Json(snapshot(&product.pool).await?))
}

pub async fn current(pool: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!("SELECT workspace_id FROM selected_workspace WHERE owner_id='owner'")
        .fetch_one(pool)
        .await
}

pub async fn snapshot(pool: &PgPool) -> Result<WorkspaceSnapshot, sqlx::Error> {
    let mut tx = crate::pg::snapshot_tx(pool).await?;
    let current_id =
        sqlx::query_scalar!("SELECT workspace_id FROM selected_workspace WHERE owner_id='owner'")
            .fetch_one(&mut *tx)
            .await?;
    let workspaces = sqlx::query_as!(
        Workspace,
        "SELECT id,name,icon,color FROM workspaces ORDER BY CASE WHEN id='local' THEN 0 ELSE 1 END,name,id"
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(WorkspaceSnapshot {
        current_id,
        workspaces,
    })
}

#[utoipa::path(post, path="/v1/workspaces/commands", operation_id="workspaces_command", request_body=WorkspaceRequest, responses((status=200, body=WorkspaceReceipt), (status=400, body=ErrorBody), (status=404, body=ErrorBody), (status=409, body=ErrorBody)))]
async fn command(
    State(product): State<Product>,
    _owner: Owner,
    Json(request): Json<WorkspaceRequest>,
) -> Result<Json<WorkspaceReceipt>, CommandError> {
    Ok(Json(execute(&product.pool, request).await?))
}

pub async fn execute(
    pool: &PgPool,
    request: WorkspaceRequest,
) -> Result<WorkspaceReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let WorkspaceRequest { command, .. } = request;
    let payload = serde_json::to_value(&command).expect("serializable command");
    let mut tx = pool.begin().await?;
    let receipt = receipts::execute(&mut tx, Scope::Workspace, operation_id, &payload, |tx| {
        Box::pin(async move {
            let result_id = match command {
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
                    sqlx::query!(
                        "INSERT INTO workspaces(id,name,icon,color) VALUES ($1,$2,$3,$4)",
                        id,
                        name.trim(),
                        icon,
                        color
                    )
                    .execute(&mut **tx)
                    .await?;
                    sqlx::query!(
                        "UPDATE selected_workspace SET workspace_id=$1 WHERE owner_id='owner'",
                        id
                    )
                    .execute(&mut **tx)
                    .await?;
                    id
                }
                WorkspaceCommand::Rename { id, name } => {
                    validate_name(&name)?;
                    let changed = sqlx::query!(
                        "UPDATE workspaces SET name=$2 WHERE id=$1",
                        id,
                        name.trim()
                    )
                    .execute(&mut **tx)
                    .await?
                    .rows_affected();
                    if changed == 0 {
                        return Err(CommandError::NotFound);
                    }
                    id
                }
                WorkspaceCommand::Switch { id } => {
                    let changed = sqlx::query!(
                        "UPDATE selected_workspace SET workspace_id=$1 WHERE owner_id='owner' AND EXISTS(SELECT 1 FROM workspaces WHERE id=$1)",
                        id
                    )
                    .execute(&mut **tx)
                    .await?
                    .rows_affected();
                    if changed == 0 {
                        return Err(CommandError::NotFound);
                    }
                    id
                }
            };
            Ok(result_id)
        })
    })
    .await?;
    tx.commit().await?;
    Ok(WorkspaceReceipt {
        result_id: receipt.result,
    })
}

fn invalid() -> CommandError {
    CommandError::Invalid(
        "Use a name of 1 to 120 characters, up to four icon characters, and a #RRGGBB color."
            .into(),
    )
}
fn validate_name(name: &str) -> Result<(), CommandError> {
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
    use crate::todos;

    fn request(command: WorkspaceCommand) -> WorkspaceRequest {
        WorkspaceRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }

    fn add(title: &str) -> todos::TodoRequest {
        todos::TodoRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command: todos::TodoCommand::Add {
                title: title.into(),
            },
        }
    }

    #[sqlx::test]
    async fn the_first_workspace_exists_and_product_data_stays_in_its_workspace(pool: PgPool) {
        let initial = snapshot(&pool).await.unwrap();
        assert_eq!(initial.current_id, "local");
        assert_eq!(initial.workspaces[0].name, "World Wide Webb");
        let operation = add("Water the plants");
        todos::execute(&pool, "local", operation.clone())
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
        assert!(
            todos::snapshot(&pool, &created)
                .await
                .unwrap()
                .todos
                .is_empty()
        );
        // A replay in the first workspace is the same command, not a second Todo.
        todos::execute(&pool, "local", operation).await.unwrap();
        let local = todos::snapshot(&pool, "local").await.unwrap().todos;
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].title, "Water the plants");
        execute(
            &pool,
            request(WorkspaceCommand::Switch { id: "local".into() }),
        )
        .await
        .unwrap();
        assert_eq!(current(&pool).await.unwrap(), "local");
    }
}
