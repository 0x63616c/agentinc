//! The Calendar: the Owner's Events, one Calendar per Workspace.
use crate::{
    api::{AppState, CommandError, ErrorBody, Owner, Product},
    receipts::{self, OperationId, Scope},
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Times are epoch seconds.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct CalendarEvent {
    pub id: i64,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarSnapshot {
    pub events: Vec<CalendarEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarCommand {
    Create {
        title: String,
        starts_at: i64,
        ends_at: i64,
    },
    Delete {
        id: i64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarRequest {
    pub operation_id: String,
    pub command: CalendarCommand,
}

/// The id of the Event the command touched.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct CalendarReceipt {
    pub result_id: i64,
}

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(command))
}

#[utoipa::path(get, path="/v1/calendar", operation_id="calendar_state", responses((status=200, body=CalendarSnapshot)))]
async fn state(
    State(product): State<Product>,
    owner: Owner,
) -> Result<Json<CalendarSnapshot>, CommandError> {
    Ok(Json(snapshot(&product.pool, &owner.workspace).await?))
}

pub async fn snapshot(pool: &PgPool, workspace: &str) -> Result<CalendarSnapshot, sqlx::Error> {
    let events = sqlx::query_as!(
        CalendarEvent,
        "SELECT id,title,starts_at,ends_at FROM calendar_events WHERE workspace_id=$1 ORDER BY starts_at,id",
        workspace
    )
    .fetch_all(pool)
    .await?;
    Ok(CalendarSnapshot { events })
}

#[utoipa::path(post, path="/v1/calendar/commands", operation_id="calendar_command", request_body=CalendarRequest, responses((status=200, body=CalendarReceipt), (status=400, body=ErrorBody), (status=404, body=ErrorBody), (status=409, body=ErrorBody)))]
async fn command(
    State(product): State<Product>,
    owner: Owner,
    Json(request): Json<CalendarRequest>,
) -> Result<Json<CalendarReceipt>, CommandError> {
    Ok(Json(
        execute(&product.pool, &owner.workspace, request).await?,
    ))
}

pub async fn execute(
    pool: &PgPool,
    workspace: &str,
    request: CalendarRequest,
) -> Result<CalendarReceipt, CommandError> {
    let operation_id = OperationId::parse(&request.operation_id)?;
    let CalendarRequest { command, .. } = request;
    let payload = serde_json::to_value(&command).expect("serializable command");
    let scope = Scope::Calendar {
        workspace: workspace.to_owned(),
    };
    let workspace = workspace.to_owned();
    let mut tx = pool.begin().await?;
    let receipt = receipts::execute(&mut tx, scope, operation_id, &payload, |tx| {
        Box::pin(async move {
            let id = match command {
                CalendarCommand::Create {
                    title,
                    starts_at,
                    ends_at,
                } => {
                    let title = title.trim();
                    if title.is_empty() || title.chars().count() > 120 || ends_at < starts_at {
                        return Err(CommandError::Invalid(
                            "Use a title of 1 to 120 characters and an end at or after the start."
                                .into(),
                        ));
                    }
                    sqlx::query_scalar!(
                        "INSERT INTO calendar_events(workspace_id,title,starts_at,ends_at) VALUES ($1,$2,$3,$4) RETURNING id",
                        workspace,
                        title,
                        starts_at,
                        ends_at
                    )
                    .fetch_one(&mut **tx)
                    .await?
                }
                CalendarCommand::Delete { id } => {
                    let changed = sqlx::query!(
                        "DELETE FROM calendar_events WHERE workspace_id=$1 AND id=$2",
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
    Ok(CalendarReceipt {
        result_id: receipt.result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command: CalendarCommand) -> CalendarRequest {
        CalendarRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }

    #[sqlx::test]
    async fn creates_lists_by_start_and_deletes(pool: PgPool) {
        let later = execute(
            &pool,
            "local",
            request(CalendarCommand::Create {
                title: "Dentist".into(),
                starts_at: 2_000,
                ends_at: 3_000,
            }),
        )
        .await
        .unwrap()
        .result_id;
        execute(
            &pool,
            "local",
            request(CalendarCommand::Create {
                title: "Standup".into(),
                starts_at: 1_000,
                ends_at: 1_000,
            }),
        )
        .await
        .unwrap();
        let events = snapshot(&pool, "local").await.unwrap().events;
        assert_eq!(
            events.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(),
            ["Standup", "Dentist"]
        );
        assert!(snapshot(&pool, "other").await.unwrap().events.is_empty());

        execute(
            &pool,
            "local",
            request(CalendarCommand::Delete { id: later }),
        )
        .await
        .unwrap();
        assert_eq!(snapshot(&pool, "local").await.unwrap().events.len(), 1);
        let missing = execute(
            &pool,
            "local",
            request(CalendarCommand::Delete { id: later }),
        )
        .await;
        assert!(matches!(missing, Err(CommandError::NotFound)));
    }

    #[sqlx::test]
    async fn an_end_before_the_start_is_invalid(pool: PgPool) {
        let result = execute(
            &pool,
            "local",
            request(CalendarCommand::Create {
                title: "Backwards".into(),
                starts_at: 10,
                ends_at: 5,
            }),
        )
        .await;
        assert!(matches!(result, Err(CommandError::Invalid(_))));
    }
}
