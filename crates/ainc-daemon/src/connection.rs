//! Codex owns credentials; the daemon owns process lifetimes and account state.
use crate::{
    codex,
    product::{ApiError, ErrorBody, Product},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use utoipa::ToSchema;

#[derive(Clone, Default, Debug, Deserialize, Serialize, ToSchema)]
pub struct ConnectionStatus {
    pub account: Option<String>,
    pub models: Vec<codex::Model>,
    pub signing_in: bool,
    pub auth_url: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone)]
struct ConnectionState {
    product: Product,
    status: Arc<Mutex<ConnectionStatus>>,
    cancel: Arc<AtomicBool>,
}
pub fn router(product: Product) -> Router {
    Router::new()
        .route("/v1/connection", get(status))
        .route("/v1/connection/login", post(login))
        .route("/v1/connection/cancel", post(cancel))
        .route("/v1/connection/logout", post(logout))
        .with_state(ConnectionState {
            product,
            status: Arc::default(),
            cancel: Arc::default(),
        })
}
fn unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "connection_unavailable",
        "Connection is unavailable. Try refreshing.",
    )
}

#[utoipa::path(get, path = "/v1/connection", operation_id = "connection_status", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn status(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    let current = {
        let mut current = state.status.lock().map_err(|_| unavailable())?;
        let snapshot = current.clone();
        if !current.signing_in {
            current.error = None;
        }
        snapshot
    };
    if current.signing_in || current.error.is_some() {
        return Ok(Json(current));
    }
    let result = tokio::task::spawn_blocking(codex::status)
        .await
        .map_err(|_| unavailable())?;
    let mut current = state.status.lock().map_err(|_| unavailable())?;
    match result {
        Ok((account, models)) => {
            current.account = account;
            current.models = models;
        }
        Err(error) => {
            let mut failed = current.clone();
            failed.error = Some(error.to_string());
            return Ok(Json(failed));
        }
    }
    Ok(Json(current.clone()))
}

#[utoipa::path(post, path = "/v1/connection/login", operation_id = "connection_login", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn login(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    {
        let mut status = state.status.lock().map_err(|_| unavailable())?;
        if status.signing_in {
            return Ok(Json(status.clone()));
        }
        *status = ConnectionStatus {
            signing_in: true,
            ..Default::default()
        };
    }
    state.cancel.store(false, Ordering::Relaxed);
    let background = state.clone();
    tokio::task::spawn_blocking(move || {
        let result = codex::login(background.cancel.clone(), |url| {
            if let Ok(mut status) = background.status.lock() {
                status.auth_url = Some(url);
            }
        })
        .and_then(|_| codex::status());
        if let Ok(mut status) = background.status.lock() {
            status.signing_in = false;
            status.auth_url = None;
            match result {
                Ok((account, models)) => {
                    status.account = account;
                    status.models = models;
                    status.error = None;
                }
                Err(error) => status.error = Some(error.to_string()),
            }
        }
    });
    let status = state.status.lock().map_err(|_| unavailable())?.clone();
    Ok(Json(status))
}
#[utoipa::path(post, path = "/v1/connection/cancel", operation_id = "connection_cancel", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn cancel(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    state.cancel.store(true, Ordering::Relaxed);
    let status = state.status.lock().map_err(|_| unavailable())?.clone();
    Ok(Json(status))
}
#[utoipa::path(post, path = "/v1/connection/logout", operation_id = "connection_logout", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn logout(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM turns WHERE state IN ('queued','running'))",
    )
    .fetch_one(&state.product.pool)
    .await?;
    if pending {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "Wait for accepted replies before signing out.",
        ));
    }
    tokio::task::spawn_blocking(codex::logout)
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
    *state.status.lock().map_err(|_| unavailable())? = ConnectionStatus::default();
    Ok(Json(ConnectionStatus::default()))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    #[derive(utoipa::OpenApi)]
    #[openapi(paths(status, login, cancel, logout))]
    struct Api;
    <Api as utoipa::OpenApi>::openapi()
}
