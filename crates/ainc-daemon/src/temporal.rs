//! Owner-only view of the configured runtime's workflow visibility.
use crate::product::{ApiError, ErrorBody, Product};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde::{Deserialize, Serialize};
use turnkeel::{RunRecord, Runtime, RuntimeConfig};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct TemporalState {
    product: Product,
    config: RuntimeConfig,
    ui_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    status: Option<String>,
    page: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionView {
    pub workflow_id: String,
    pub run_id: String,
    pub workflow_type: String,
    pub status: String,
    pub started_at: i64,
    pub closed_at: Option<i64>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecutionPage {
    pub executions: Vec<ExecutionView>,
    pub next_page: Option<String>,
    pub ui_available: bool,
}

pub fn router(product: Product, config: RuntimeConfig, ui_url: Option<String>) -> Router {
    Router::new()
        .route("/v1/temporal/executions", get(list))
        .with_state(TemporalState {
            product,
            config,
            ui_url,
        })
        .layer(axum::middleware::from_fn(crate::compatibility))
        .layer(axum::middleware::map_response(crate::server_version_header))
}

fn execution_url(base: Option<&str>, namespace: &str, execution: &RunRecord) -> Option<String> {
    let mut url = reqwest::Url::parse(base?).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.path_segments_mut().ok()?.extend([
        "namespaces",
        namespace,
        "workflows",
        &execution.id,
        &execution.run_id,
        "history",
    ]);
    Some(url.to_string())
}

#[utoipa::path(get,path="/v1/temporal/executions",operation_id="temporal_executions",params(("status" = Option<String>, Query, description = "All, Running, Completed, Failed, Canceled, Terminated, TimedOut, ContinuedAsNew, or Paused"),("page" = Option<String>, Query, description = "Opaque next-page token")),responses((status=200,body=ExecutionPage),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=503,body=ErrorBody)))]
pub async fn list(
    State(state): State<TemporalState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<ExecutionPage>, ApiError> {
    state.product.authorize(&headers)?;
    let page = Runtime::run_history(
        &state.config,
        query.status.as_deref(),
        query.page.as_deref(),
    )
    .await
    .map_err(|error| {
        let message = error.to_string();
        if message.contains("invalid page token") || message.contains("invalid execution status") {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose a valid status or refresh the page.",
            )
        } else {
            tracing::warn!(%error, "Temporal visibility unavailable");
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Temporal is unavailable. Try again.",
            )
        }
    })?;
    let base = state.ui_url.as_deref();
    Ok(Json(ExecutionPage {
        executions: page
            .runs
            .into_iter()
            .map(|execution| {
                let url = execution_url(base, &state.config.scope, &execution);
                ExecutionView {
                    workflow_id: execution.id,
                    run_id: execution.run_id,
                    workflow_type: execution.kind,
                    status: execution.status,
                    started_at: execution.started_at,
                    closed_at: execution.closed_at,
                    url,
                }
            })
            .collect(),
        next_page: page.next_page,
        ui_available: base
            .and_then(|url| reqwest::Url::parse(url).ok())
            .is_some_and(|url| matches!(url.scheme(), "http" | "https")),
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
    #[test]
    fn exact_execution_url_encodes_ids_and_respects_missing_ui() {
        let execution = RunRecord {
            id: "ticket/a b".into(),
            run_id: "run-1".into(),
            kind: "Run".into(),
            status: "Running".into(),
            started_at: 0,
            closed_at: None,
        };
        assert_eq!(execution_url(None, "owner", &execution), None);
        assert_eq!(
            execution_url(Some("http://127.0.0.1:8080/"), "owner", &execution).unwrap(),
            "http://127.0.0.1:8080/namespaces/owner/workflows/ticket%2Fa%20b/run-1/history"
        );
    }

    #[tokio::test]
    async fn endpoint_filters_and_pages_owner_namespace() -> anyhow::Result<()> {
        let server = Server::start().await?;
        let config = server.config();
        let agent = Agent::builder("visibility-test")
            .model(ScriptedModel::new().otherwise(text("done")))
            .build();
        let runtime = Runtime::configured(config.clone(), std::slice::from_ref(&agent)).await?;
        for _ in 0..41 {
            runtime.session(&agent).await?;
        }
        let pool =
            PgPoolOptions::new().connect_lazy("postgres://agentinc:agentinc@127.0.0.1:1/unused")?;
        let product = Product::new(pool, "owner-token".into())?;
        let state = TemporalState {
            product,
            config,
            ui_url: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer owner-token".parse()?);
        let mut first = None;
        for _ in 0..200 {
            let page = list(
                State(state.clone()),
                headers.clone(),
                Query(ListQuery {
                    status: Some("Running".into()),
                    page: None,
                }),
            )
            .await
            .expect("visibility response")
            .0;
            if page.next_page.is_some() {
                first = Some(page);
                break;
            }
            tokio::task::yield_now().await;
        }
        let first = first.expect("all sessions entered visibility history");
        assert!(!first.executions.is_empty());
        assert!(first.next_page.is_some());
        assert!(first.executions.iter().all(|row| row.status == "Running"));
        let second = list(
            State(state.clone()),
            headers.clone(),
            Query(ListQuery {
                status: Some("Running".into()),
                page: first.next_page,
            }),
        )
        .await
        .expect("visibility response")
        .0;
        assert_eq!(first.executions.len() + second.executions.len(), 41);
        assert!(second.next_page.is_none());
        let completed = list(
            State(state.clone()),
            headers.clone(),
            Query(ListQuery {
                status: Some("Completed".into()),
                page: None,
            }),
        )
        .await
        .expect("visibility response")
        .0;
        assert!(completed.executions.is_empty());
        let invalid = list(
            State(state),
            headers,
            Query(ListQuery {
                status: Some("Running' OR 1=1".into()),
                page: None,
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
