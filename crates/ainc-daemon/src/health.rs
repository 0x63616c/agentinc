//! Liveness, readiness and the server's version. Unversioned: no credential,
//! no client compatibility check.
use crate::api::{CommandError, ErrorBody};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

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

pub(crate) fn routes() -> OpenApiRouter<crate::api::AppState> {
    OpenApiRouter::new()
        .routes(routes!(live))
        .routes(routes!(ready))
        .routes(routes!(version))
}

#[utoipa::path(get, path = "/health/live", operation_id = "health_live", responses((status = 200, body = Health)))]
async fn live() -> Json<Health> {
    Json(Health {
        status: "live".into(),
    })
}

#[utoipa::path(get, path = "/health/ready", operation_id = "health_ready", responses((status = 200, body = Health), (status = 503, body = ErrorBody)))]
async fn ready(State(pool): State<PgPool>) -> Result<Json<Health>, CommandError> {
    sqlx::query_scalar::<_, i32>("SELECT id FROM product_bootstrap WHERE id = 1")
        .fetch_one(&pool)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "product database is not ready");
            CommandError::Unavailable("The product database is not ready.".into())
        })?;
    Ok(Json(Health {
        status: "ready".into(),
    }))
}

#[utoipa::path(get, path = "/version", operation_id = "version_show", responses((status = 200, body = Version)))]
async fn version() -> Json<Version> {
    Json(Version {
        product: "AgentInc".into(),
        version: ainc_release::VERSION.into(),
        api: ainc_release::API,
    })
}
