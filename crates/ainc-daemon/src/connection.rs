//! Legacy ChatGPT Connection endpoints. They remain for older clients and the
//! generated CLI; the providers API is the current surface for the same state.
use crate::{
    codex,
    product::{ApiError, ErrorBody, Product},
    providers::Providers,
};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    providers: Arc<Providers>,
}
pub fn router(product: Product, providers: Arc<Providers>) -> Router {
    Router::new()
        .route("/v1/connection", get(status))
        .route("/v1/connection/login", post(login))
        .route("/v1/connection/cancel", post(cancel))
        .route("/v1/connection/logout", post(logout))
        .with_state(ConnectionState { product, providers })
}

#[utoipa::path(get, path = "/v1/connection", operation_id = "connection_status", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn status(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    Ok(Json(state.providers.codex_connection(false).await))
}

#[utoipa::path(post, path = "/v1/connection/login", operation_id = "connection_login", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn login(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    state.providers.codex_connect().await;
    Ok(Json(state.providers.codex_connection(false).await))
}
#[utoipa::path(post, path = "/v1/connection/cancel", operation_id = "connection_cancel", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn cancel(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    state.providers.codex_cancel().await;
    Ok(Json(state.providers.codex_connection(false).await))
}
#[utoipa::path(post, path = "/v1/connection/logout", operation_id = "connection_logout", responses((status = 200, body = ConnectionStatus), (status = 401, body = ErrorBody), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn logout(
    State(state): State<ConnectionState>,
    headers: HeaderMap,
) -> Result<Json<ConnectionStatus>, ApiError> {
    state.product.authorize(&headers)?;
    state
        .providers
        .codex_disconnect(&state.product.pool)
        .await?;
    Ok(Json(ConnectionStatus::default()))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    #[derive(utoipa::OpenApi)]
    #[openapi(paths(status, login, cancel, logout))]
    struct Api;
    <Api as utoipa::OpenApi>::openapi()
}
