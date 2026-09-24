use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::future::IntoFuture;
use std::{
    env, fs,
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let discovery = env::var("AINC_DISCOVERY_FILE").context("AINC_DISCOVERY_FILE is required")?;
    let lock_path = Path::new(&discovery).with_extension("lock");
    fs::create_dir_all(lock_path.parent().context("discovery directory")?)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(lock_path)?;
    lock.try_lock()
        .context("another daemon owns this discovery file")?;
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .context("connect product database")?;
    ainc_daemon::migrate(&pool)
        .await
        .context("migrate product database")?;
    let legacy = env::var_os("AINC_LEGACY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default())
                .join("Library/Application Support/Agentinc OS")
        });
    ainc_daemon::legacy::import(&pool, &legacy)
        .await
        .context("import legacy app data")?;
    let token_path = Path::new(&discovery).with_file_name("owner-token");
    fs::create_dir_all(token_path.parent().context("token directory")?)?;
    let token = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&token_path)
    {
        Ok(mut file) => {
            let token = uuid::Uuid::new_v4().to_string();
            file.write_all(token.as_bytes())?;
            token
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::read_to_string(&token_path)?
        }
        Err(error) => return Err(error.into()),
    };
    let product = ainc_daemon::product::Product::new(pool.clone(), token.trim().into())?;
    let runner = ainc_daemon::conversations::Runner::start(pool.clone()).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    publish_address(Path::new(&discovery), address)?;
    tracing::info!(%address, "AgentInc daemon ready");
    tokio::select! {
        result = axum::serve(listener, ainc_daemon::product_router(product)).into_future() => result?,
        result = runner.run() => result?,
    }
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
