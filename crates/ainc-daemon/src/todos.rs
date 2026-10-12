//! Todos: the Owner's simple checklist, one list per Workspace.
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
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TodoSnapshot {
    pub todos: Vec<Todo>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TodoCommand {
    Add { title: String },
    Complete { id: i64, completed: bool },
    Delete { id: i64 },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TodoRequest {
    pub operation_id: String,
    pub command: TodoCommand,
}

/// The id of the Todo the command touched.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct TodoReceipt {
    pub result_id: i64,
}

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

#[utoipa::path(get, path="/v1/todos", operation_id="todos_state", responses((status=200, body=TodoSnapshot)))]
async fn state(
    State(product): State<Product>,
    owner: Owner,
) -> Result<Json<TodoSnapshot>, CommandError> {
    Ok(Json(snapshot(&product.pool, &owner.workspace).await?))
}

pub async fn snapshot(pool: &PgPool, workspace: &str) -> Result<TodoSnapshot, sqlx::Error> {
    let todos = sqlx::query_as!(
        Todo,
        "SELECT id,title,completed FROM todos WHERE workspace_id=$1 ORDER BY completed,id",
        workspace
    )
    .fetch_all(pool)
    .await?;
    Ok(TodoSnapshot { todos })
}

#[utoipa::path(post, path="/v1/todos/commands", operation_id="todos_command", request_body=TodoRequest, responses((status=200, body=TodoReceipt), (status=400, body=ErrorBody), (status=404, body=ErrorBody), (status=409, body=ErrorBody)))]
async fn command(
    State(product): State<Product>,
    owner: Owner,
    Json(request): Json<TodoRequest>,
) -> Result<Json<TodoReceipt>, CommandError> {
    Ok(Json(
        execute(&product.pool, &owner.workspace, request).await?,
    ))
}

pub async fn execute(
    pool: &PgPool,
    workspace: &str,
    request: TodoRequest,
) -> Result<TodoReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let TodoRequest { command, .. } = request;
    let payload = serde_json::to_value(&command).expect("serializable command");
    let scope = Scope::Todo {
        workspace: workspace.to_owned(),
    };
    let workspace = workspace.to_owned();
    let mut tx = pool.begin().await?;
    let receipt = receipts::execute(&mut tx, scope, operation_id, &payload, |tx| {
        Box::pin(async move {
            let id = match command {
                TodoCommand::Add { title } => {
                    let title = title.trim();
                    if title.is_empty() || title.chars().count() > 500 {
                        return Err(CommandError::Invalid(
                            "Use a title of 1 to 500 characters.".into(),
                        ));
                    }
                    sqlx::query_scalar!(
                        "INSERT INTO todos(workspace_id,title) VALUES ($1,$2) RETURNING id",
                        workspace,
                        title
                    )
                    .fetch_one(&mut **tx)
                    .await?
                }
                TodoCommand::Complete { id, completed } => {
                    let changed = sqlx::query!(
                        "UPDATE todos SET completed=$3 WHERE workspace_id=$1 AND id=$2",
                        workspace,
                        id,
                        completed
                    )
                    .execute(&mut **tx)
                    .await?
                    .rows_affected();
                    if changed == 0 {
                        return Err(CommandError::NotFound);
                    }
                    id
                }
                TodoCommand::Delete { id } => {
                    let changed = sqlx::query!(
                        "DELETE FROM todos WHERE workspace_id=$1 AND id=$2",
                        workspace,
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
            Ok(id)
        })
    })
    .await?;
    tx.commit().await?;
    Ok(TodoReceipt {
        result_id: receipt.result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command: TodoCommand) -> TodoRequest {
        TodoRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }

    #[sqlx::test]
    async fn adds_completes_and_deletes_in_one_workspace(pool: PgPool) {
        let id = execute(
            &pool,
            "local",
            request(TodoCommand::Add {
                title: "  Pay rent ".into(),
            }),
        )
        .await
        .unwrap()
        .result_id;
        let snapshot = snapshot(&pool, "local").await.unwrap();
        assert_eq!(snapshot.todos.len(), 1);
        assert_eq!(snapshot.todos[0].title, "Pay rent");
        assert!(!snapshot.todos[0].completed);

        execute(
            &pool,
            "local",
            request(TodoCommand::Complete {
                id,
                completed: true,
            }),
        )
        .await
        .unwrap();
        assert!(super::snapshot(&pool, "local").await.unwrap().todos[0].completed);

        let elsewhere = execute(&pool, "other", request(TodoCommand::Delete { id })).await;
        assert!(matches!(elsewhere, Err(CommandError::NotFound)));

        execute(&pool, "local", request(TodoCommand::Delete { id }))
            .await
            .unwrap();
        assert!(
            super::snapshot(&pool, "local")
                .await
                .unwrap()
                .todos
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn an_empty_title_is_invalid(pool: PgPool) {
        let result = execute(
            &pool,
            "local",
            request(TodoCommand::Add {
                title: "   ".into(),
            }),
        )
        .await;
        assert!(matches!(result, Err(CommandError::Invalid(_))));
    }

    #[sqlx::test]
    async fn the_same_operation_replays_without_a_second_todo(pool: PgPool) {
        let request = request(TodoCommand::Add {
            title: "Water plants".into(),
        });
        let first = execute(&pool, "local", request.clone()).await.unwrap();
        let again = execute(&pool, "local", request).await.unwrap();
        assert_eq!(first.result_id, again.result_id);
        assert_eq!(snapshot(&pool, "local").await.unwrap().todos.len(), 1);
    }
}
