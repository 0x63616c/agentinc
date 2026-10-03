//! Owner-only view of the runtime's retained work: runs, sessions and occurrences.
use crate::product::{ApiError, ErrorBody, Product};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turnkeel::{RunKind, RunStatus, Runtime, RuntimeConfig};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct WorkState {
    product: Product,
    config: RuntimeConfig,
    /// Connected on first use so the daemon serves everything else while the runtime is
    /// still starting; one connection is then kept for every later page.
    runtime: Arc<tokio::sync::OnceCell<Runtime>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    /// A Ticket run: one assignment worked to completion.
    Run,
    /// A Conversation session.
    Session,
    /// One occurrence of an Automation.
    Occurrence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    status: Option<WorkStatus>,
    page: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkView {
    /// The run, session or occurrence ID.
    pub id: String,
    pub kind: WorkKind,
    pub status: WorkStatus,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkPage {
    pub work: Vec<WorkView>,
    pub next_page: Option<String>,
}

pub fn router(product: Product, config: RuntimeConfig) -> Router {
    Router::new()
        .route("/v1/work", get(list))
        .with_state(WorkState {
            product,
            config,
            runtime: Arc::default(),
        })
        .layer(axum::middleware::from_fn(crate::compatibility))
        .layer(axum::middleware::map_response(crate::server_version_header))
}

impl From<RunKind> for WorkKind {
    fn from(kind: RunKind) -> Self {
        match kind {
            RunKind::Run => Self::Run,
            RunKind::Session => Self::Session,
            RunKind::Occurrence => Self::Occurrence,
        }
    }
}

impl From<RunStatus> for WorkStatus {
    fn from(status: RunStatus) -> Self {
        match status {
            RunStatus::Running => Self::Running,
            RunStatus::Completed => Self::Completed,
            RunStatus::Failed => Self::Failed,
            RunStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<WorkStatus> for RunStatus {
    fn from(status: WorkStatus) -> Self {
        match status {
            WorkStatus::Running => Self::Running,
            WorkStatus::Completed => Self::Completed,
            WorkStatus::Failed => Self::Failed,
            WorkStatus::Cancelled => Self::Cancelled,
        }
    }
}

fn unavailable(error: turnkeel::Error) -> ApiError {
    tracing::warn!(%error, "work history unavailable");
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Work history is unavailable. Try again.",
    )
}

#[utoipa::path(get,path="/v1/work",operation_id="work",params(("status" = Option<WorkStatus>, Query, description = "Only work in this status"),("page" = Option<String>, Query, description = "Opaque next-page token")),responses((status=200,body=WorkPage),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn list(
    State(state): State<WorkState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<WorkPage>, ApiError> {
    state.product.authorize(&headers)?;
    let runtime = state
        .runtime
        .get_or_try_init(|| Runtime::observer(state.config.clone()))
        .await
        .map_err(unavailable)?;
    let page = runtime
        .work_history(query.status.map(Into::into), query.page.as_deref())
        .await
        .map_err(|error| match error {
            turnkeel::Error::InvalidInput(_) => ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Refresh the list; that page is no longer available.",
            ),
            error => unavailable(error),
        })?;
    Ok(Json(WorkPage {
        work: page
            .work
            .into_iter()
            .map(|work| WorkView {
                id: work.id,
                kind: work.kind.into(),
                status: work.status.into(),
                started_at: work.started_at,
                closed_at: work.closed_at,
            })
            .collect(),
        next_page: page.next_page,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::header::AUTHORIZATION, response::IntoResponse};
    use sqlx::postgres::PgPoolOptions;
    use turnkeel::{
        Agent,
        testing::{ScriptedModel, Server, text},
    };

    #[tokio::test]
    async fn endpoint_filters_and_pages_in_agent_vocabulary() -> anyhow::Result<()> {
        let server = Server::start().await?;
        let config = server.config();
        let agent = Agent::builder("visibility-test")
            .model(ScriptedModel::new().otherwise(text("done")))
            .build();
        let runtime = Runtime::configured(config.clone(), std::slice::from_ref(&agent)).await?;
        for _ in 0..41 {
            runtime.session(&agent).await?;
        }
        let finished = runtime.start(&agent, "hello").await?;
        assert_eq!(finished.result().await?, "done");
        let pool =
            PgPoolOptions::new().connect_lazy("postgres://agentinc:agentinc@127.0.0.1:1/unused")?;
        let product = Product::new(pool, "owner-token".into())?;
        let state = WorkState {
            product,
            config,
            runtime: Arc::default(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer owner-token".parse()?);
        let fetch = |status: Option<WorkStatus>, page: Option<String>| {
            let state = state.clone();
            let headers = headers.clone();
            async move {
                list(State(state), headers, Query(ListQuery { status, page }))
                    .await
                    .expect("work response")
                    .0
            }
        };
        // Retained history is eventually consistent; keep paging until every session shows.
        let (first, second) = loop {
            let first = fetch(Some(WorkStatus::Running), None).await;
            if let Some(token) = first.next_page.clone() {
                let second = fetch(Some(WorkStatus::Running), Some(token)).await;
                if first.work.len() + second.work.len() == 41 {
                    break (first, second);
                }
            }
            tokio::task::yield_now().await;
        };
        assert!(!first.work.is_empty() && !second.work.is_empty());
        assert!(second.next_page.is_none());
        let rows: Vec<&WorkView> = first.work.iter().chain(&second.work).collect();
        assert!(rows.iter().all(|row| row.status == WorkStatus::Running));
        assert!(rows.iter().all(|row| row.kind == WorkKind::Session));
        let completed = loop {
            let page = fetch(Some(WorkStatus::Completed), None).await;
            if !page.work.is_empty() {
                break page;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(completed.work.len(), 1);
        assert_eq!(completed.work[0].id, finished.id().as_str());
        assert_eq!(completed.work[0].kind, WorkKind::Run);
        assert!(completed.work[0].closed_at.is_some());
        let invalid = list(
            State(state),
            headers,
            Query(ListQuery {
                status: None,
                page: Some("not-a-token".into()),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(invalid.into_response().status(), StatusCode::BAD_REQUEST);
        runtime.shutdown().await?;
        server.shutdown().await?;
        Ok(())
    }
}
