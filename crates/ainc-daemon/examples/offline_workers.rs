//! Native outage acceptance harness: serve an existing disposable product database
//! without starting any workers. Use only with an explicit fixture credential.
use std::{env, fs, path::PathBuf};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = sqlx::PgPool::connect(&env::var("DATABASE_URL")?).await?;
    let token = env::var("AINC_FIXTURE_TOKEN")?;
    let discovery = PathBuf::from(env::var("AINC_DISCOVERY_FILE")?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    fs::write(&discovery, format!("http://{}", listener.local_addr()?))?;
    fs::write(discovery.with_file_name("owner-token"), &token)?;
    axum::serve(
        listener,
        ainc_daemon::product_router(
            ainc_daemon::product::Product::new(pool, token)?,
            ainc_daemon::home::Home::new(std::sync::Arc::new(
                ainc_daemon::secrets::MemoryStore::default(),
            )),
        ),
    )
    .await?;
    Ok(())
}
