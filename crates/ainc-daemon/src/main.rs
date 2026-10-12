mod local_runtime;
use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::{
    env, fs,
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// Everything the daemon takes from its environment, read once at start.
/// Every variable is `AINC_*` except `DATABASE_URL`, which is sqlx's name.
struct Config {
    /// Where the daemon publishes its URL; the lock, token and database
    /// defaults live beside it.
    discovery: PathBuf,
    /// An external Postgres; without it the daemon manages its own database.
    database_url: Option<String>,
}
impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            discovery: env::var_os("AINC_DISCOVERY_FILE")
                .map(PathBuf::from)
                .context("AINC_DISCOVERY_FILE is required")?,
            database_url: env::var("DATABASE_URL").ok(),
        })
    }
}

fn main() -> Result<()> {
    ainc_release::process::reset_inherited_signals()?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--local-runtime" {
        return local_runtime::helper(Path::new(&args[1])).await;
    }
    anyhow::ensure!(args.is_empty(), "usage: aincd");
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Config::from_env()?;
    let discovery = config.discovery.as_path();
    let lock_path = discovery.with_extension("lock");
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
    let local = if config.database_url.is_none() {
        Some(local_runtime::ManagedRuntime::start(discovery.with_file_name("runtime")).await?)
    } else {
        None
    };
    let database_url = match &local {
        Some(local) => local.database_url.clone(),
        None => config.database_url.clone().expect("checked above"),
    };
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .context("connect product database")?;
    ainc_daemon::migrate(&pool)
        .await
        .context("migrate product database")?;
    let token_path = discovery.with_file_name("owner-token");
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    publish_address(discovery, address)?;
    tracing::info!(%address, "{}", ainc_release::DAEMON_READY);
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let drain_signal = shutdown.clone();
    let auth = format!("Bearer {}", token.trim());
    let drain = axum::routing::post(move |headers: axum::http::HeaderMap| {
        let shutdown = drain_signal.clone();
        let auth = auth.clone();
        async move {
            if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(auth.as_str()) {
                return axum::http::StatusCode::UNAUTHORIZED;
            }
            let _ = shutdown.send(true);
            axum::http::StatusCode::ACCEPTED
        }
    });
    let signal_task = tokio::spawn(async move {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
        let _ = shutdown.send(true);
        anyhow::Ok(())
    });
    let result = axum::serve(
        listener,
        ainc_daemon::app(product).route("/internal/drain", drain),
    )
    .with_graceful_shutdown(stopping(receiver))
    .await;
    signal_task.abort();
    let _ = fs::remove_file(discovery);
    result?;
    pool.close().await;
    drop(local);
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

async fn stopping(mut receiver: tokio::sync::watch::Receiver<bool>) {
    let _ = receiver.wait_for(|stopping| *stopping).await;
}
