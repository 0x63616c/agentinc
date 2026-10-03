//! The real daemon over HTTP: find it (or start it), authenticate once, and
//! keep that client until a transport failure says to connect again.
use super::transport::{ConnectionAction, DaemonError, Envelope, Reply, Request, Slice, Transport};
use ainc_client::Client;
use anyhow::{Context, Result};
use std::{
    future::Future,
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

/// Run a future to completion on the app's one HTTP runtime.
pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> T {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Runtime::new().expect("HTTP runtime"))
        .block_on(future)
}

/// Where the daemon publishes its URL: `AINC_DISCOVERY_FILE`, else the installed location.
pub(crate) fn discovery_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AINC_DISCOVERY_FILE") {
        return Ok(path.into());
    }
    Ok(ainc_identity::identity::discovery_file())
}

pub(crate) async fn connect() -> Result<Client> {
    anyhow::ensure!(
        !crate::updates::installation_pending(),
        "Update installation is preparing; companion requests are paused"
    );
    let discovery = discovery_path()?;
    let url = if let Ok(url) = std::env::var("AINC_API_URL") {
        url
    } else {
        let existing = std::fs::read_to_string(&discovery).ok();
        let ready = match &existing {
            Some(url) => ainc_client::ready(url.trim()).await,
            None => false,
        };
        if !ready {
            let _launch = crate::updates::companion_launch_guard().await?;
            spawn_daemon(&discovery)?;
            for _ in 0..1200 {
                if let Ok(url) = std::fs::read_to_string(&discovery)
                    && ainc_client::ready(url.trim()).await
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
        std::fs::read_to_string(&discovery)
            .context("Daemon unavailable. Start the local stack and refresh.")?
            .trim()
            .to_owned()
    };
    let token_path = std::env::var_os("AINC_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| discovery.with_file_name("owner-token"));
    let token =
        std::fs::read_to_string(token_path).context("Daemon owner credential unavailable")?;
    Ok(ainc_client::connect(&url, &token)?)
}

fn spawn_daemon(discovery: &std::path::Path) -> Result<()> {
    let binary = std::env::current_exe()?.with_file_name("aincd");
    anyhow::ensure!(
        binary.exists(),
        "Companion daemon missing. Start the development stack or reinstall AgentInc."
    );
    let mut command = daemon_command(binary, discovery, std::env::var_os("AINC_DATABASE_URL"));
    std::fs::create_dir_all(discovery.parent().context("discovery directory")?)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(discovery.with_file_name("daemon.log"))?;
    ainc_release::process::prepare_child(&mut command)
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    Ok(())
}

/// The companion command: the app's `AINC_DATABASE_URL` reaches `aincd` as
/// `DATABASE_URL`; without it the inherited `DATABASE_URL` applies unchanged.
fn daemon_command(
    binary: PathBuf,
    discovery: &std::path::Path,
    database: Option<std::ffi::OsString>,
) -> std::process::Command {
    let mut command = std::process::Command::new(binary);
    if let Some(database) = database {
        command.env("DATABASE_URL", database);
    }
    command.env("AINC_DISCOVERY_FILE", discovery);
    command
}

#[derive(Default)]
pub struct HttpTransport {
    client: Mutex<Option<Client>>,
}
impl HttpTransport {
    async fn client(&self) -> Result<Client, DaemonError> {
        if let Some(client) = self.client.lock().expect("daemon client").clone() {
            return Ok(client);
        }
        let client = connect()
            .await
            .map_err(|error| DaemonError::Unavailable(format!("{error:#}")))?;
        *self.client.lock().expect("daemon client") = Some(client.clone());
        Ok(client)
    }
}
impl Transport for HttpTransport {
    fn call(&self, request: Request) -> Result<Reply, DaemonError> {
        block_on(async {
            let client = self.client().await?;
            let result = perform(&client, request).await;
            if matches!(result, Err(DaemonError::Retryable(_))) {
                // The next request discovers and authenticates again.
                *self.client.lock().expect("daemon client") = None;
            }
            result
        })
    }
}

async fn perform(client: &Client, request: Request) -> Result<Reply, DaemonError> {
    use ainc_client::classify;
    Ok(match request {
        Request::Fetch(Slice::Workspaces) => Reply::Workspaces(
            client
                .workspaces_state()
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Fetch(Slice::Product) => Reply::Product(
            client
                .product_state()
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Fetch(Slice::Tickets) => Reply::Tickets(
            client
                .tickets_state()
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Fetch(Slice::Automations) => Reply::Automations(
            client
                .automations_state()
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Fetch(Slice::Activity { id }) => Reply::Activity(
            client
                .tickets_activity()
                .id(id)
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Fetch(Slice::Executions { status, page }) => {
            let mut request = client.work_list();
            if let Ok(status) = status.parse::<ainc_client::types::WorkStatus>() {
                request = request.status(status);
            }
            if let Some(page) = page {
                request = request.page(page);
            }
            Reply::Executions(request.send().await.map_err(classify)?.into_inner())
        }
        Request::Command(Envelope::Workspace(body)) => Reply::Receipt(
            client
                .workspaces_command()
                .body(body)
                .send()
                .await
                .map_err(classify)?
                .into_inner()
                .result_id,
        ),
        Request::Command(Envelope::Automation(body)) => Reply::Receipt(
            client
                .automations_command()
                .body(body)
                .send()
                .await
                .map_err(classify)?
                .into_inner()
                .result_id,
        ),
        Request::Command(Envelope::Ticket(body)) => Reply::Acknowledgement(
            client
                .tickets_command()
                .body(body)
                .send()
                .await
                .map_err(classify)?
                .into_inner()
                .result_id,
        ),
        Request::Command(Envelope::Product(body)) => Reply::Acknowledgement(
            client
                .product_command()
                .body(body)
                .send()
                .await
                .map_err(classify)?
                .into_inner()
                .result_id,
        ),
        Request::ConnectionStatus => Reply::ConnectionStatus(
            client
                .connection_status()
                .send()
                .await
                .map_err(classify)?
                .into_inner(),
        ),
        Request::Connection(ConnectionAction::Login) => {
            client.connection_login().send().await.map_err(classify)?;
            Reply::Done
        }
        Request::Connection(ConnectionAction::Cancel) => {
            client.connection_cancel().send().await.map_err(classify)?;
            Reply::Done
        }
        Request::Connection(ConnectionAction::Logout) => {
            client.connection_logout().send().await.map_err(classify)?;
            Reply::Done
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child_environment(database: Option<&str>) -> String {
        let output = daemon_command(
            "/usr/bin/env".into(),
            std::path::Path::new("/isolated/api-url"),
            database.map(Into::into),
        )
        .output()
        .expect("run companion command");
        assert!(output.status.success());
        String::from_utf8(output.stdout).expect("environment")
    }

    #[test]
    fn configured_external_database_reaches_the_spawned_daemon() {
        let environment = child_environment(Some("postgres://external/db"));
        assert!(
            environment
                .lines()
                .any(|line| line == "DATABASE_URL=postgres://external/db")
        );
        assert!(
            environment
                .lines()
                .any(|line| line == "AINC_DISCOVERY_FILE=/isolated/api-url")
        );
    }

    #[test]
    fn without_an_app_override_the_daemon_inherits_its_database() {
        let inherited = std::env::var("DATABASE_URL").ok();
        let child = child_environment(None)
            .lines()
            .find_map(|line| line.strip_prefix("DATABASE_URL=").map(str::to_owned));
        assert_eq!(child, inherited);
    }
}
