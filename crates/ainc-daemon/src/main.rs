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
    /// Where the daemon publishes its URL; the lock, token, runtime and
    /// workspace defaults live beside it.
    discovery: PathBuf,
    /// An external Postgres; without it the daemon manages its own runtime.
    database_url: Option<String>,
    #[cfg(feature = "legacy-import")]
    legacy_dir: Option<PathBuf>,
    /// The durable runtime as JSON; otherwise `runtime.json` beside discovery.
    runtime_config: Option<String>,
    workspace_dir: Option<PathBuf>,
    tool_allow: Option<String>,
    codex_home: Option<PathBuf>,
    codex_path: Option<PathBuf>,
}
impl Config {
    fn from_env() -> Result<Self> {
        let path = |name| env::var_os(name).map(PathBuf::from);
        Ok(Self {
            discovery: path("AINC_DISCOVERY_FILE").context("AINC_DISCOVERY_FILE is required")?,
            database_url: env::var("DATABASE_URL").ok(),
            #[cfg(feature = "legacy-import")]
            legacy_dir: path("AINC_LEGACY_DIR"),
            runtime_config: env::var("AINC_RUNTIME_CONFIG").ok(),
            workspace_dir: path("AINC_WORKSPACE_DIR"),
            tool_allow: env::var("AINC_TOOL_ALLOW").ok(),
            codex_home: path("AINC_CODEX_HOME"),
            codex_path: path("AINC_CODEX_PATH"),
        })
    }
}

fn runtime_config(bytes: &[u8]) -> Result<(turnkeel::RuntimeConfig, Option<String>)> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let ui_url = value
        .get("ui_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    Ok((serde_json::from_value(value)?, ui_url))
}

fn main() -> Result<()> {
    ainc_release::process::reset_inherited_signals()?;
    // TLS: reqwest links rustls without a provider; ring is the one Temporal already uses.
    let _ = rustls::crypto::ring::default_provider().install_default();
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
    #[cfg(feature = "legacy-import")]
    {
        let legacy = config
            .legacy_dir
            .clone()
            .unwrap_or_else(ainc_release::identity::support_dir);
        ainc_daemon::legacy::import_once(&pool, &legacy)
            .await
            .context("import legacy app data")?;
    }
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
    let (runtime, _ui_url): (turnkeel::RuntimeConfig, Option<String>) =
        if let Some(local) = &local {
            (local.config.clone(), Some(local.ui_url.clone()))
        } else if let Some(runtime) = &config.runtime_config {
            runtime_config(runtime.as_bytes()).context("parse AINC_RUNTIME_CONFIG")?
        } else {
            runtime_config(&fs::read(discovery.with_file_name("runtime.json")).context(
                "configure the durable runtime in runtime.json beside daemon discovery",
            )?)?
        };
    let workspace = config
        .workspace_dir
        .clone()
        .unwrap_or_else(|| discovery.with_file_name("workspace"));
    fs::create_dir_all(&workspace)?;
    let allowed = serde_json::from_str(config.tool_allow.as_deref().unwrap_or("[]"))
        .context("AINC_TOOL_ALLOW must be a JSON list of read_file, write_file, shell, git")?;
    let policy = ainc_daemon::coding::WorkspacePolicy::new(workspace, allowed)?;
    let connection = ainc_daemon::connection::Connection::local_with(
        config.codex_home.clone(),
        config.codex_path.clone(),
    );
    let models = std::sync::Arc::new(ainc_daemon::inference::CodexModels::new(
        connection.clone(),
    )?);
    let runner =
        ainc_daemon::conversations::Runner::start(pool.clone(), runtime.clone(), models.clone())
            .await?;
    let tickets =
        ainc_daemon::execution::Runner::start(pool.clone(), runtime.clone(), models, policy)
            .await?;
    let automations =
        ainc_daemon::automations::Runner::start(pool.clone(), runtime.clone()).await?;
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
    let result = tokio::try_join!(
        async {
            axum::serve(
                listener,
                ainc_daemon::app_with_runtime(product, connection, runtime)
                    .route("/internal/drain", drain),
            )
            .with_graceful_shutdown(stopping(receiver.clone()))
            .await
            .map_err(anyhow::Error::from)
        },
        runner.run_until(stopping(receiver.clone())),
        tickets.run_until(stopping(receiver.clone())),
        automations.run_until(stopping(receiver.clone())),
    );
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

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn runtime_ui_address_stays_in_daemon_configuration() {
        let (config, ui_url) = runtime_config(
            br#"{"endpoint":"http://127.0.0.1:7233","scope":"agentinc-dev","worker_group":"personal","ui_url":"http://127.0.0.1:8233"}"#,
        )
        .unwrap();
        assert_eq!(config.scope, "agentinc-dev");
        assert_eq!(ui_url.as_deref(), Some("http://127.0.0.1:8233"));
    }
}
