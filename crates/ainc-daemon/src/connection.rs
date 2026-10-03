//! The ChatGPT Connection. Codex owns credentials; the daemon owns one Codex
//! process for its whole lifetime, the sign-in state machine and the HTTP routes.
mod codex;
use crate::api::{AppState, CommandError, ErrorBody, Owner, Product};
use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json,
    extract::{FromRef, State},
};
pub use codex::Model;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc::RecvTimeoutError,
    },
    time::{Duration, Instant},
};
use turnkeel::ModelError;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Clone, Default, Debug, Deserialize, Serialize, ToSchema)]
pub struct ConnectionStatus {
    /// Who is signed in, as "email · plan", or `None`.
    pub signed_in_as: Option<String>,
    pub models: Vec<Model>,
    pub signing_in: bool,
    pub auth_url: Option<String>,
    pub error: Option<String>,
}

/// Bearer credentials Codex keeps in its `auth.json`.
#[derive(Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub account_id: String,
}
#[derive(Deserialize)]
struct AuthFile {
    tokens: Tokens,
}

/// One shared Connection: the HTTP routes, the model catalog and every model
/// step go through the same long-lived Codex client.
#[derive(Clone)]
pub struct Connection {
    inner: Arc<Inner>,
}
struct Inner {
    home: PathBuf,
    executable: PathBuf,
    client: Mutex<Option<codex::Client>>,
    default_model: Mutex<Option<String>>,
    status: Mutex<ConnectionStatus>,
    cancel: AtomicBool,
    sign_in: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Connection {
    /// The user's Codex profile and the installed or bundled Codex CLI.
    pub fn local() -> Self {
        Self::local_with(None, None)
    }
    /// [`Self::local`] with the daemon's configured overrides for either path.
    pub fn local_with(home: Option<PathBuf>, executable: Option<PathBuf>) -> Self {
        Self::new(
            home.unwrap_or_else(codex::default_home),
            executable.unwrap_or_else(codex::default_executable),
        )
    }
    pub fn new(home: PathBuf, executable: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                home,
                executable,
                client: Mutex::default(),
                default_model: Mutex::default(),
                status: Mutex::default(),
                cancel: AtomicBool::new(false),
                sign_in: Mutex::default(),
            }),
        }
    }
    pub fn home(&self) -> &Path {
        &self.inner.home
    }
    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }
    /// Run `op` against the one Codex process, starting it on first use and
    /// replacing it only if it has exited.
    fn with_client<T>(&self, op: impl FnOnce(&mut codex::Client) -> Result<T>) -> Result<T> {
        let mut slot = Self::lock(&self.inner.client);
        if slot.as_mut().is_none_or(|client| !client.alive()) {
            *slot = Some(codex::Client::start_at(
                &self.inner.home,
                &self.inner.executable,
            )?);
        }
        let client = slot.as_mut().expect("client started above");
        let result = op(client);
        if result.is_err() && !client.alive() {
            *slot = None;
        }
        result
    }
    fn probe(&self) -> Result<(Option<String>, Vec<Model>)> {
        self.with_client(|client| {
            let signed_in_as = client.signed_in_as(false)?;
            let models = if signed_in_as.is_some() {
                let (models, default) = client.models()?;
                *Self::lock(&self.inner.default_model) = default;
                models
            } else {
                Vec::new()
            };
            Ok((signed_in_as, models))
        })
    }

    /// Current status. While signing in, or once after a failed sign-in, the
    /// cached snapshot is returned; otherwise Codex is asked.
    pub fn status(&self) -> ConnectionStatus {
        let snapshot = {
            let mut current = Self::lock(&self.inner.status);
            let snapshot = current.clone();
            if !current.signing_in {
                current.error = None;
            }
            snapshot
        };
        if snapshot.signing_in || snapshot.error.is_some() {
            return snapshot;
        }
        let result = self.probe();
        let mut current = Self::lock(&self.inner.status);
        match result {
            Ok((signed_in_as, models)) => {
                current.signed_in_as = signed_in_as;
                current.models = models;
                current.clone()
            }
            Err(error) => {
                let mut failed = current.clone();
                failed.error = Some(error.to_string());
                failed
            }
        }
    }
    /// Begin a browser sign-in in the background; `status()` carries the link
    /// and then the outcome.
    pub fn login(&self) -> ConnectionStatus {
        {
            let mut status = Self::lock(&self.inner.status);
            if status.signing_in {
                return status.clone();
            }
            *status = ConnectionStatus {
                signing_in: true,
                ..Default::default()
            };
        }
        self.inner.cancel.store(false, Ordering::Relaxed);
        let connection = self.clone();
        let handle = std::thread::spawn(move || {
            let result = connection.sign_in().and_then(|_| connection.probe());
            let mut status = Self::lock(&connection.inner.status);
            status.signing_in = false;
            status.auth_url = None;
            match result {
                Ok((signed_in_as, models)) => {
                    status.signed_in_as = signed_in_as;
                    status.models = models;
                    status.error = None;
                }
                Err(error) => status.error = Some(error.to_string()),
            }
        });
        *Self::lock(&self.inner.sign_in) = Some(handle);
        Self::lock(&self.inner.status).clone()
    }
    fn sign_in(&self) -> Result<()> {
        *Self::lock(&self.inner.default_model) = None;
        let (url, login_id, events) = self.with_client(|client| {
            let (url, login_id) = client.login_start()?;
            Ok((url, login_id, client.events()))
        })?;
        Self::lock(&self.inner.status).auth_url = Some(url);
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            if self.inner.cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                // A completed callback can race cancellation; re-read Codex's authoritative state.
                let signed_in = self.with_client(|client| {
                    let _ = client.login_cancel(&login_id);
                    client.signed_in_as(false)
                })?;
                if signed_in.is_some() {
                    return Ok(());
                }
                bail!("Sign-in cancelled. You can try again when ready.");
            }
            let event = Self::lock(&events).recv_timeout(Duration::from_millis(250));
            let event: Value = match event {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => bail!("Codex sign-in stopped. Try again."),
            };
            if event["method"] == "account/login/completed"
                && (event["params"]["loginId"].is_null() || event["params"]["loginId"] == login_id)
            {
                if event["params"]["success"] == true {
                    return Ok(());
                }
                bail!("Sign-in was not completed. Try again.");
            }
        }
    }
    /// Stop a sign-in in progress; the outcome arrives through `status()`.
    pub fn cancel(&self) -> ConnectionStatus {
        self.inner.cancel.store(true, Ordering::Relaxed);
        Self::lock(&self.inner.status).clone()
    }
    #[cfg(test)]
    fn join_sign_in(&self) {
        if let Some(handle) = Self::lock(&self.inner.sign_in).take() {
            let _ = handle.join();
        }
    }
    pub fn logout(&self) -> Result<()> {
        self.with_client(|client| client.logout())?;
        *Self::lock(&self.inner.default_model) = None;
        *Self::lock(&self.inner.status) = ConnectionStatus::default();
        Ok(())
    }
    /// Bearer tokens for a model step and the model to use for `model`
    /// (`connection-default` resolves to the model Codex marks default).
    pub fn credentials(&self, model: &str) -> Result<(Tokens, String), ModelError> {
        let selected = self
            .with_client(|client| {
                if client
                    .signed_in_as(true)
                    .map_err(|_| anyhow!("Refresh the ChatGPT Connection in Settings."))?
                    .is_none()
                {
                    bail!("Sign in with a ChatGPT subscription in Settings.");
                }
                if model != "connection-default" {
                    return Ok(model.to_owned());
                }
                let cached = Self::lock(&self.inner.default_model).clone();
                if let Some(default) = cached {
                    return Ok(default);
                }
                let (_, default) = client
                    .models()
                    .map_err(|_| anyhow!("Could not read the Connection's default model."))?;
                let default = default.context("Select a model in Settings.")?;
                *Self::lock(&self.inner.default_model) = Some(default.clone());
                Ok(default)
            })
            .map_err(|error| ModelError::fatal(error.to_string()))?;
        let data = std::fs::read(self.inner.home.join("auth.json")).map_err(|_| {
            ModelError::fatal("Sign in with a Codex file credential store for this profile.")
        })?;
        let auth: AuthFile = serde_json::from_slice(&data)
            .map_err(|_| ModelError::fatal("ChatGPT Connection credentials are unavailable."))?;
        if auth.tokens.access_token.is_empty() || auth.tokens.account_id.is_empty() {
            return Err(ModelError::fatal("Sign in to ChatGPT in Settings."));
        }
        Ok((auth.tokens, selected))
    }
}

#[derive(Clone)]
struct ConnectionState {
    product: Product,
    connection: Connection,
}
impl FromRef<AppState> for ConnectionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            product: state.product.clone(),
            connection: state.connection.clone(),
        }
    }
}
pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(login))
        .routes(routes!(cancel))
        .routes(routes!(logout))
}
fn unavailable() -> CommandError {
    CommandError::Unavailable("Connection is unavailable. Try refreshing.".into())
}

#[utoipa::path(get, path = "/v1/connection", operation_id = "connection_status", responses((status = 200, body = ConnectionStatus), (status = 503, body = ErrorBody)))]
async fn status(
    State(state): State<ConnectionState>,
    _owner: Owner,
) -> Result<Json<ConnectionStatus>, CommandError> {
    let connection = state.connection;
    let status = tokio::task::spawn_blocking(move || connection.status())
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(status))
}

#[utoipa::path(post, path = "/v1/connection/login", operation_id = "connection_login", responses((status = 200, body = ConnectionStatus), (status = 503, body = ErrorBody)))]
async fn login(
    State(state): State<ConnectionState>,
    _owner: Owner,
) -> Result<Json<ConnectionStatus>, CommandError> {
    Ok(Json(state.connection.login()))
}
#[utoipa::path(post, path = "/v1/connection/cancel", operation_id = "connection_cancel", responses((status = 200, body = ConnectionStatus), (status = 503, body = ErrorBody)))]
async fn cancel(
    State(state): State<ConnectionState>,
    _owner: Owner,
) -> Result<Json<ConnectionStatus>, CommandError> {
    Ok(Json(state.connection.cancel()))
}
#[utoipa::path(post, path = "/v1/connection/logout", operation_id = "connection_logout", responses((status = 200, body = ConnectionStatus), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn logout(
    State(state): State<ConnectionState>,
    _owner: Owner,
) -> Result<Json<ConnectionStatus>, CommandError> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM turns WHERE state IN ('queued','running'))",
    )
    .fetch_one(&state.product.pool)
    .await?;
    if pending {
        return Err(CommandError::Conflict(
            "Wait for accepted replies before signing out.".into(),
        ));
    }
    let connection = state.connection;
    tokio::task::spawn_blocking(move || connection.logout())
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
    Ok(Json(ConnectionStatus::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fake_codex;

    fn connection() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let connection = Connection::new(dir.path().join("codex"), fake_codex::executable());
        (dir, connection)
    }

    #[test]
    fn one_codex_process_serves_status_and_every_model_step() {
        let (_dir, connection) = connection();
        fake_codex::sign_in(connection.home()).unwrap();
        for _ in 0..2 {
            let status = connection.status();
            assert_eq!(
                status.signed_in_as.as_deref(),
                Some(fake_codex::SIGNED_IN_AS)
            );
            assert_eq!(status.models[0].id, fake_codex::MODEL);
            assert!(status.error.is_none());
        }
        for model in ["connection-default", "fixture", "connection-default"] {
            let (tokens, selected) = connection.credentials(model).unwrap();
            assert_eq!(selected, fake_codex::MODEL);
            assert_eq!(tokens.access_token, fake_codex::ACCESS_TOKEN);
            assert_eq!(tokens.account_id, fake_codex::ACCOUNT_ID);
        }
        assert_eq!(fake_codex::spawns(connection.home()), 1);
    }

    #[test]
    fn signed_out_profile_has_no_models_and_refuses_model_steps() {
        let (_dir, connection) = connection();
        let status = connection.status();
        assert_eq!(status.signed_in_as, None);
        assert!(status.models.is_empty());
        let error = connection.credentials("fixture").err().expect("refused");
        assert!(!error.retryable);
        assert!(error.message.contains("Sign in"));
    }

    #[test]
    fn missing_codex_is_reported_and_probed_again() {
        let dir = tempfile::tempdir().unwrap();
        let connection = Connection::new(dir.path().join("codex"), dir.path().join("absent"));
        let failed = connection.status();
        assert!(failed.error.is_some());
        assert_eq!(failed.signed_in_as, None);
        // Probe failures are never cached: the next read asks Codex again.
        assert!(connection.status().error.is_some());
        assert!(
            connection
                .credentials("fixture")
                .err()
                .expect("refused")
                .message
                .contains("Codex")
        );
    }

    #[test]
    fn login_completes_then_logout_forgets_the_identity() {
        let (_dir, connection) = connection();
        assert_eq!(connection.status().signed_in_as, None);
        let started = connection.login();
        assert!(started.signing_in);
        // A second request while signing in changes nothing.
        assert!(connection.login().signing_in);
        connection.join_sign_in();
        let status = connection.status();
        assert!(!status.signing_in);
        assert_eq!(status.auth_url, None);
        assert_eq!(
            status.signed_in_as.as_deref(),
            Some(fake_codex::SIGNED_IN_AS)
        );
        assert_eq!(status.error, None);
        connection.logout().unwrap();
        let status = connection.status();
        assert_eq!(status.signed_in_as, None);
        assert!(connection.credentials("fixture").is_err());
        assert_eq!(fake_codex::spawns(connection.home()), 1);
    }

    #[test]
    fn cancelled_login_error_is_reported_once() {
        let (_dir, connection) = connection();
        fake_codex::hold_sign_in(connection.home()).unwrap();
        connection.login();
        let cancelled = connection.cancel();
        assert!(cancelled.signing_in);
        connection.join_sign_in();
        let status = connection.status();
        assert!(!status.signing_in);
        assert_eq!(status.signed_in_as, None);
        assert_eq!(
            status.error.as_deref(),
            Some("Sign-in cancelled. You can try again when ready.")
        );
        // The sign-in error is sticky for exactly one read, then status probes again.
        assert_eq!(connection.status().error, None);
        assert_eq!(fake_codex::spawns(connection.home()), 1);
    }
}
