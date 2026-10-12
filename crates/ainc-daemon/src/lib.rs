pub mod api;
pub mod calendar;
pub mod health;
pub mod pg;
pub mod process;
pub mod product;
pub mod receipts;
pub mod todos;
pub mod workspaces;
use axum::Router;
use sqlx::PgPool;

pub fn openapi() -> serde_json::Value {
    serde_json::to_value(api::openapi()).expect("OpenAPI serialization")
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

/// The product API: every endpoint once, the shared layers once.
pub fn app(product: product::Product) -> Router {
    api::app(product)
}
