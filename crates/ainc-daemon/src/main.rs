use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::{env, fs, net::SocketAddr, path::Path};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let discovery = env::var("AINC_DISCOVERY_FILE").context("AINC_DISCOVERY_FILE is required")?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .context("connect product database")?;
    ainc_daemon::migrate(&pool)
        .await
        .context("migrate product database")?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    publish_address(Path::new(&discovery), address)?;
    tracing::info!(%address, "AgentInc daemon ready");
    axum::serve(listener, ainc_daemon::router(pool)).await?;
    Ok(())
}

fn publish_address(path: &Path, address: SocketAddr) -> Result<()> {
    let parent = path.parent().context("discovery path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, format!("http://{address}\n"))?;
    fs::rename(temporary, path)?;
    Ok(())
}
