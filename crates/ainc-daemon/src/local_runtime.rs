//! Private, single-user runtime. Binaries come from the signed app bundle;
//! mutable databases live beside the daemon discovery file, never in the app.
use anyhow::{Context, Result, ensure};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct LocalRuntime {
    _owner: fs::File,
    postgres: Child,
    temporal: Child,
    pub database_url: String,
    pub config: turnkeel::RuntimeConfig,
}
fn port() -> Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}
fn log(root: &Path, name: &str) -> Result<fs::File> {
    Ok(fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(root.join(name))?)
}
fn command(binary: &Path, root: &Path, name: &str) -> Result<Command> {
    let mut command = Command::new(binary);
    command
        .stdin(Stdio::null())
        .stdout(log(root, name)?)
        .stderr(log(root, name)?);
    Ok(command)
}
impl LocalRuntime {
    pub async fn start(root: &Path, resources: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let owner = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root.join("owner.lock"))?;
        // Serializes restart against the previous helper flushing its children.
        owner.lock().context("lock bundled runtime ownership")?;
        let bin = resources.join("postgres/bin");
        let data = root.join("postgres");
        let password_path = root.join("postgres-password");
        let password = match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&password_path)
        {
            Ok(mut file) => {
                use std::io::Write;
                let password = uuid::Uuid::new_v4().to_string();
                file.write_all(password.as_bytes())?;
                password
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::read_to_string(&password_path)?
            }
            Err(e) => return Err(e.into()),
        };
        if !data.join("PG_VERSION").exists() {
            // initdb refuses a nonempty interrupted initialization. Preserve it
            // for diagnosis instead of deleting potentially valuable data.
            let status = command(&bin.join("initdb"), root, "postgres.log")?
                .arg("-D")
                .arg(&data)
                .arg("-L")
                .arg(resources.join("postgres/share"))
                .args([
                    "--username=agentinc",
                    "--encoding=UTF8",
                    "--locale=C",
                    "--auth=scram-sha-256",
                ])
                .arg("--pwfile")
                .arg(&password_path)
                .status()?;
            ensure!(
                status.success(),
                "local database initialization failed; see postgres.log"
            );
        }
        ensure!(
            fs::read_to_string(data.join("PG_VERSION"))?.trim() == "16",
            "local database requires Postgres 16; refusing an incompatible data upgrade"
        );
        let pg_port = port()?;
        let postgres = command(&bin.join("postgres"), root, "postgres.log")?
            .arg("-D")
            .arg(&data)
            .args(["-h", "127.0.0.1", "-p", &pg_port.to_string(), "-k", ""])
            .spawn()
            .context("start bundled database")?;
        let runtime_port = port()?;
        let mut temporal_command = command(&resources.join("temporal"), root, "runtime.log")?;
        temporal_command
            .args([
                "server",
                "start-dev",
                "--headless",
                "--ip",
                "127.0.0.1",
                "--port",
                &runtime_port.to_string(),
                "--namespace",
                "agentinc",
            ])
            .arg("--db-filename")
            .arg(root.join("runtime.sqlite"));
        for name in [
            "WorkspaceId",
            "TicketId",
            "ConversationId",
            "AutomationId",
            "AgentId",
        ] {
            temporal_command.args(["--search-attribute", &format!("{name}=Keyword")]);
        }
        let temporal = match temporal_command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let mut postgres = postgres;
                let _ = postgres.kill();
                let _ = postgres.wait();
                return Err(error.into());
            }
        };
        let mut runtime = Self {
            _owner: owner,
            postgres,
            temporal,
            database_url: format!("postgres://agentinc:{password}@127.0.0.1:{pg_port}/postgres"),
            config: serde_json::from_value(
                serde_json::json!({"endpoint":format!("http://127.0.0.1:{runtime_port}"),"scope":"agentinc","worker_group":"agentinc-personal"}),
            )?,
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            ensure!(
                runtime.postgres.try_wait()?.is_none(),
                "bundled database exited; see postgres.log"
            );
            ensure!(
                runtime.temporal.try_wait()?.is_none(),
                "bundled runtime exited; see runtime.log"
            );
            let db_ready = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(1))
                .connect(&runtime.database_url)
                .await;
            if let Ok(pool) = db_ready {
                pool.close().await;
                let healthy = tokio::process::Command::new(resources.join("temporal"))
                    .args([
                        "--address",
                        &format!("127.0.0.1:{runtime_port}"),
                        "operator",
                        "cluster",
                        "health",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await?;
                if healthy.success() {
                    return Ok(runtime);
                }
            }
            ensure!(
                Instant::now() < deadline,
                "local runtime did not become ready; see runtime.log"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
impl Drop for LocalRuntime {
    fn drop(&mut self) {
        // Children belong exclusively to this daemon under its discovery lock.
        // PostgreSQL fast shutdown disconnects residual pool sockets, rolls back
        // unfinished transactions and checkpoints. Smart shutdown could wait
        // forever on a detached SDK pool after its owner has drained.
        for (child, signal) in [
            (&mut self.temporal, libc::SIGTERM),
            (&mut self.postgres, libc::SIGINT),
        ] {
            // SAFETY: these PIDs are owned children, not values from a PID file.
            unsafe {
                libc::kill(child.id() as i32, signal);
            }
            let _ = child.wait();
        }
    }
}
pub fn bundled_resources() -> Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .context("daemon executable parent")?
        .join("../Resources/runtime"))
}

/// The helper owns services while its parent holds stdin open. Kernel EOF also
/// covers SIGKILL/crash, without trusting stale process IDs or signaling strangers.
pub async fn helper(root: &Path) -> Result<()> {
    use std::io::Write;
    use tokio::io::AsyncReadExt;
    let runtime = LocalRuntime::start(root, &bundled_resources()?).await?;
    let identity = serde_json::json!({"database_url":runtime.database_url,"config":runtime.config});
    writeln!(std::io::stdout(), "{}", identity)?;
    std::io::stdout().flush()?;
    let mut ignored = Vec::new();
    tokio::io::stdin().read_to_end(&mut ignored).await?;
    drop(runtime);
    Ok(())
}

pub struct ManagedRuntime {
    child: Child,
    lifetime: Option<std::process::ChildStdin>,
    pub database_url: String,
    pub config: turnkeel::RuntimeConfig,
}
impl ManagedRuntime {
    pub async fn start(root: PathBuf) -> Result<Self> {
        tokio::task::spawn_blocking(move || {
            use std::io::BufRead;
            let mut child =
                ainc_release::process::prepare_child(&mut Command::new(std::env::current_exe()?))
                    .arg("--local-runtime")
                    .arg(root)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()?;
            let lifetime = child.stdin.take();
            let output = child.stdout.take().context("runtime identity pipe")?;
            let mut line = String::new();
            let result = (|| -> Result<(String, turnkeel::RuntimeConfig)> {
                ensure!(
                    std::io::BufReader::new(output).read_line(&mut line)? > 0,
                    "bundled runtime helper exited before readiness"
                );
                let mut identity: serde_json::Value =
                    serde_json::from_str(&line).context("runtime identity")?;
                let database = identity["database_url"]
                    .as_str()
                    .context("runtime database")?
                    .to_owned();
                let config = serde_json::from_value(identity["config"].take())?;
                Ok((database, config))
            })();
            match result {
                Ok((database_url, config)) => Ok(Self {
                    child,
                    lifetime,
                    database_url,
                    config,
                }),
                Err(error) => {
                    drop(lifetime);
                    let _ = child.wait();
                    Err(error)
                }
            }
        })
        .await?
    }
}
impl Drop for ManagedRuntime {
    fn drop(&mut self) {
        drop(self.lifetime.take());
        let _ = self.child.wait();
    }
}
