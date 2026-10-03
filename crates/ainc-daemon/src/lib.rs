/// Builds an HTTP client with the ring TLS provider installed. Every reqwest client in the
/// daemon starts here, so a test or binary that forgets the provider cannot panic later.
pub fn http_client() -> reqwest::ClientBuilder {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
}

pub mod api;
pub mod automations;
pub mod coding;
pub mod connection;
pub mod conversations;
pub mod execution;
pub mod health;
pub mod inference;
pub mod legacy;
pub mod pg;
pub mod product;
pub mod receipts;
pub mod terminal_sessions;
pub mod testing;
pub mod tickets;
pub mod work;
mod worker;
pub mod workspaces;
use axum::Router;
use sqlx::PgPool;

pub fn openapi() -> serde_json::Value {
    serde_json::to_value(api::openapi()).expect("OpenAPI serialization")
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

/// The product API with a Connection to the user's local Codex, for tests and
/// fixtures that never reach it.
pub fn product_router(product: product::Product) -> Router {
    app(product, connection::Connection::local())
}

/// The product API: every endpoint once, the shared layers once.
pub fn app(product: product::Product, connection: connection::Connection) -> Router {
    api::app(product, connection, Router::new())
}

/// [`app`] plus the durable runtime's work history, as the daemon serves it.
pub fn app_with_runtime(
    product: product::Product,
    connection: connection::Connection,
    config: turnkeel::RuntimeConfig,
) -> Router {
    api::app(product.clone(), connection, work::router(product, config))
}
