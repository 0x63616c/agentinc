pub mod automations;
pub mod calendar;
mod codex;
pub mod coding;
mod connection;
mod control_center;
mod conversation_tools;
pub mod conversations;
pub mod durable;
pub mod execution;
pub mod home;
pub mod inference;
pub mod legacy;
pub mod product;
pub mod secrets;
pub mod temporal;
pub mod terminal_sessions;
pub mod tickets;
pub mod workspaces;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
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
        version: ainc_release::VERSION.into(),
        api: ainc_release::API,
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
        product::command,
        tickets::state,
        tickets::command,
        automations::state,
        automations::command,
        home::state,
        home::command,
        home::connect,
        home::disconnect,
        calendar::state,
        calendar::command,
        calendar::import,
        terminal_sessions::list,
        terminal_sessions::create,
        terminal_sessions::close,
        workspaces::state,
        workspaces::command,
        temporal::list
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
        .layer(axum::middleware::from_fn(compatibility))
        .layer(axum::middleware::map_response(server_version_header))
}

async fn server_version_header(mut response: Response) -> Response {
    response.headers_mut().insert(
        "agent-inc-server",
        HeaderValue::from_str(&ainc_release::server_header()).expect("valid product header"),
    );
    response
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

pub fn product_router(product: product::Product) -> Router {
    router(product.pool.clone())
        .merge(product::router(product.clone()))
        .merge(connection::router(product.clone()))
        .merge(tickets::router(product.clone()))
        .merge(automations::router(product.clone()))
        .merge(calendar::router(product.clone()))
        .merge(terminal_sessions::router(product.clone()))
        .merge(workspaces::router(product))
        .layer(axum::middleware::from_fn(compatibility))
        .layer(axum::middleware::map_response(server_version_header))
}

async fn compatibility(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    if request.uri().path().starts_with("/v1/") {
        let header = request
            .headers()
            .get("agent-inc-client")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if let Err(error) =
            ainc_release::check_client(header, ainc_release::MIN_CLIENT, ainc_release::API)
        {
            return (
                StatusCode::UPGRADE_REQUIRED,
                Json(serde_json::json!({"code":error,"message":"Update to continue"})),
            )
                .into_response();
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn rejects_before_handler_and_always_returns_server_version() {
        let app = Router::new()
            .route("/v1/probe", get(|| async { "accepted" }))
            .layer(axum::middleware::from_fn(compatibility))
            .layer(axum::middleware::map_response(server_version_header));
        for (header, status) in [
            ("mac/0.0.1 (api 1)", 426),
            ("mac/9.0.0 (api 2)", 426),
            ("", 426),
            (&ainc_release::client_header(), 200),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::get("/v1/probe")
                        .header("agent-inc-client", header)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            assert!(response.headers().contains_key("agent-inc-server"));
            if status == 426 {
                let body = to_bytes(response.into_body(), 4096).await.unwrap();
                assert!(String::from_utf8_lossy(&body).contains("Update to continue"));
            }
        }
    }
}
