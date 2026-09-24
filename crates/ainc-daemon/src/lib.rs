mod codex;
mod connection;
pub mod conversations;
pub mod inference;
pub mod legacy;
pub mod product;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{OpenApi, ToSchema};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
pub struct TicketContract {
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Health {
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Version {
    pub product: String,
    pub version: String,
    pub api: u32,
}

#[utoipa::path(get, path = "/health/live", operation_id = "health_live", responses((status = 200, body = Health)))]
async fn live() -> Json<Health> {
    Json(Health {
        status: "live".into(),
    })
}

#[utoipa::path(get, path = "/health/ready", operation_id = "health_ready", responses((status = 200, body = Health), (status = 503)))]
async fn ready(State(pool): State<PgPool>) -> Result<Json<Health>, StatusCode> {
    sqlx::query_scalar::<_, i32>("SELECT id FROM product_bootstrap WHERE id = 1")
        .fetch_one(&pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(Health {
        status: "ready".into(),
    }))
}

#[utoipa::path(get, path = "/version", operation_id = "get_version", responses((status = 200, body = Version)))]
async fn version() -> Json<Version> {
    Json(Version {
        product: "AgentInc".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        api: 1,
    })
}

// Phase 1 is a wire contract only. Persistence and Ticket commands arrive in phase 3.
#[utoipa::path(post, path = "/v1/tickets/contract", operation_id = "ticket_contract", request_body = TicketContract, responses((status = 200, body = TicketContract)))]
async fn ticket_contract(Json(ticket): Json<TicketContract>) -> Json<TicketContract> {
    Json(ticket)
}

#[derive(OpenApi)]
#[openapi(
    paths(
        live,
        ready,
        version,
        ticket_contract,
        product::state,
        product::command
    ),
    components(schemas(Health, Version, TicketContract))
)]
struct Api;

pub fn openapi() -> serde_json::Value {
    serde_json::to_value({
        let mut api = Api::openapi();
        api.merge(connection::openapi());
        api
    })
    .expect("OpenAPI serialization")
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/version", get(version))
        .route("/v1/tickets/contract", post(ticket_contract))
        .with_state(pool)
        .layer(axum::middleware::map_response(server_version_header))
}

async fn server_version_header(mut response: Response) -> Response {
    response.headers_mut().insert(
        "agent-inc-server",
        HeaderValue::from_static(concat!("aincd/", env!("CARGO_PKG_VERSION"), " (api 1)")),
    );
    response
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

pub fn product_router(product: product::Product) -> Router {
    router(product.pool.clone())
        .merge(product::router(product.clone()))
        .merge(connection::router(product))
        .layer(axum::middleware::map_response(server_version_header))
}
