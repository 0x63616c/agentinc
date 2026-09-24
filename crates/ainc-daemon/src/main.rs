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
    let local = if env::var_os("DATABASE_URL").is_none() {
        Some(
            local_runtime::ManagedRuntime::start(Path::new(&discovery).with_file_name("runtime"))
                .await?,
        )
    } else {
        None
    };
    let database_url = match &local {
        Some(local) => local.database_url.clone(),
        None => env::var("DATABASE_URL")?,
    };
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
    let config: turnkeel::RuntimeConfig = if let Some(local) = &local {
        local.config.clone()
    } else if let Ok(config) = env::var("AINC_RUNTIME_CONFIG") {
        serde_json::from_str(&config).context("parse AINC_RUNTIME_CONFIG")?
    } else {
        serde_json::from_slice(
            &fs::read(Path::new(&discovery).with_file_name("runtime.json"))
                .context("configure the durable runtime in runtime.json beside daemon discovery")?,
        )?
    };
    let workspace = env::var_os("AINC_WORKSPACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&discovery).with_file_name("workspace"));
    fs::create_dir_all(&workspace)?;
    let allowed =
        serde_json::from_str(&env::var("AINC_TOOL_ALLOW").unwrap_or_else(|_| "[]".into()))
            .context("AINC_TOOL_ALLOW must be a JSON list of read_file, write_file, shell, git")?;
    let policy = ainc_daemon::coding::WorkspacePolicy::new(workspace, allowed)?;
    let models = std::sync::Arc::new(ainc_daemon::inference::CodexModels::local()?);
    let runner =
        ainc_daemon::conversations::Runner::start(pool.clone(), config.clone(), models.clone())
            .await?;
    let tickets =
        ainc_daemon::execution::Runner::start(pool.clone(), config.clone(), models, policy).await?;
    let automations = ainc_daemon::automations::Runner::start(pool.clone(), config).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    publish_address(Path::new(&discovery), address)?;
    tracing::info!(%address, "AgentInc daemon ready");
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
                ainc_daemon::product_router(product).route("/internal/drain", drain),
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
    let _ = fs::remove_file(&discovery);
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
