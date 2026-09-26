//! Model providers: Claude (subscription through Claude Code), ChatGPT (Codex
//! sign-in) and OpenRouter (Keychain API key). One catalog resolves canonical
//! model IDs such as `openrouter:typesafe/jev-router` for every agent.
pub mod claude;
pub mod openrouter;
pub mod secrets;
pub(crate) mod sse;

use crate::{
    codex,
    execution::{DeltaSink, ModelCatalog},
    inference::CodexModels,
    product::{ApiError, ErrorBody, Product},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use turnkeel::{Message, Model, ModelError, ModelRequest};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
    #[serde(rename = "openrouter")]
    OpenRouter,
}
impl ProviderId {
    pub const ALL: [ProviderId; 3] = [
        ProviderId::Claude,
        ProviderId::Codex,
        ProviderId::OpenRouter,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::OpenRouter => "openrouter",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == value)
    }
    pub fn title(self) -> &'static str {
        match self {
            ProviderId::Claude => "Claude",
            ProviderId::Codex => "ChatGPT",
            ProviderId::OpenRouter => "OpenRouter",
        }
    }
}

/// A model selection: provider plus the provider's own model ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: ProviderId,
    pub model: String,
}
impl ModelRef {
    /// `claude:claude-sonnet-5`, `codex:gpt-5-codex`, `openrouter:typesafe/jev-router`.
    /// Bare IDs from earlier releases belong to the ChatGPT Connection.
    pub fn parse(id: &str) -> Self {
        let id = id.trim();
        if let Some((prefix, model)) = id.split_once(':')
            && let Some(provider) = ProviderId::parse(prefix)
        {
            return Self {
                provider,
                model: model.into(),
            };
        }
        Self {
            provider: ProviderId::Codex,
            model: id.into(),
        }
    }
    pub fn canonical(&self) -> String {
        format!("{}:{}", self.provider.as_str(), self.model)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    pub featured: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectMethod {
    /// Sign in through the browser; completion is detected automatically.
    Browser,
    /// Sign in through the browser, then paste the code it shows.
    BrowserCode,
    /// Enter an API key, stored in the Keychain.
    ApiKey,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProviderStatus {
    pub id: ProviderId,
    pub name: String,
    pub description: String,
    pub connect: ConnectMethod,
    pub connected: bool,
    pub account: Option<String>,
    /// Canonical model IDs (`provider:model`), featured models first.
    pub models: Vec<ProviderModel>,
    pub error: Option<String>,
    pub signing_in: bool,
    pub auth_url: Option<String>,
    pub awaiting_code: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProvidersState {
    pub providers: Vec<ProviderStatus>,
    pub default_model: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ConnectRequest {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}
impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("code", &self.code.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct TestRequest {
    #[serde(default)]
    pub model: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProviderTest {
    pub ok: bool,
    pub model: String,
    pub reply: Option<String>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    #[serde(default)]
    refresh: bool,
}

#[derive(Clone, Default)]
struct SignIn {
    signing_in: bool,
    auth_url: Option<String>,
    awaiting_code: bool,
    error: Option<String>,
}

pub struct Providers {
    codex: Arc<CodexModels>,
    claude: Arc<claude::ClaudeCli>,
    openrouter: Arc<openrouter::OpenRouter>,
    codex_signin: Mutex<SignIn>,
    codex_cancel: Arc<AtomicBool>,
    claude_signin: Mutex<SignIn>,
    claude_login: tokio::sync::Mutex<Option<claude::Login>>,
    cache: Mutex<HashMap<ProviderId, (Instant, ProviderStatus)>>,
    openrouter_models: Mutex<Option<(Instant, Vec<ProviderModel>)>>,
}
const STATUS_TTL: Duration = Duration::from_secs(45);
const MODELS_TTL: Duration = Duration::from_secs(600);

impl Providers {
    pub fn new(
        codex: Arc<CodexModels>,
        claude: Arc<claude::ClaudeCli>,
        openrouter: Arc<openrouter::OpenRouter>,
    ) -> Self {
        Self {
            codex,
            claude,
            openrouter,
            codex_signin: Mutex::default(),
            codex_cancel: Arc::default(),
            claude_signin: Mutex::default(),
            claude_login: tokio::sync::Mutex::new(None),
            cache: Mutex::default(),
            openrouter_models: Mutex::default(),
        }
    }
    /// The signed-in user's real providers on this machine.
    pub fn local() -> anyhow::Result<Self> {
        Ok(Self::new(
            Arc::new(CodexModels::local()?),
            Arc::new(claude::ClaudeCli::local()),
            Arc::new(openrouter::OpenRouter::new(Arc::new(
                secrets::Keychain::product(),
            ))?),
        ))
    }
    /// Providers that touch no profile, Keychain or network: for tests and fixtures.
    pub fn offline(dir: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self::new(
            Arc::new(CodexModels::new(dir.join("codex"))?),
            Arc::new(claude::ClaudeCli::new(
                dir.join("claude-unavailable"),
                dir.join("claude"),
            )),
            Arc::new(openrouter::OpenRouter::new(Arc::new(
                secrets::MemoryStore::default(),
            ))?),
        ))
    }
    pub fn resolve_with(
        &self,
        id: &str,
        sink: Option<DeltaSink>,
    ) -> Result<Arc<dyn Model>, ModelError> {
        let selected = ModelRef::parse(id);
        Ok(match selected.provider {
            ProviderId::Claude => Arc::new(self.claude.model(&selected.model, sink)?),
            ProviderId::Codex => self.codex.resolve_with(&selected.model, sink)?,
            ProviderId::OpenRouter => Arc::new(self.openrouter.model(&selected.model, sink)?),
        })
    }

    async fn status(&self, id: ProviderId, refresh: bool) -> ProviderStatus {
        if refresh {
            // A refresh clears the last sign-in failure unless one is in progress.
            for signin in [&self.claude_signin, &self.codex_signin] {
                let mut signin = signin.lock().expect("sign-in");
                if !signin.signing_in {
                    signin.error = None;
                }
            }
        }
        if !refresh
            && let Some((at, status)) = self.cache.lock().expect("status cache").get(&id)
            && at.elapsed() < STATUS_TTL
        {
            return self.overlay(status.clone());
        }
        let status = match id {
            ProviderId::Claude => self.claude_status().await,
            ProviderId::Codex => self.codex_status().await,
            ProviderId::OpenRouter => self.openrouter_status(refresh).await,
        };
        self.cache
            .lock()
            .expect("status cache")
            .insert(id, (Instant::now(), status.clone()));
        self.overlay(status)
    }
    fn forget(&self, id: ProviderId) {
        self.cache.lock().expect("status cache").remove(&id);
    }
    /// Sign-in progress is live state, never cached.
    fn overlay(&self, mut status: ProviderStatus) -> ProviderStatus {
        let signin = match status.id {
            ProviderId::Claude => Some(self.claude_signin.lock().expect("sign-in").clone()),
            ProviderId::Codex => Some(self.codex_signin.lock().expect("sign-in").clone()),
            ProviderId::OpenRouter => None,
        };
        if let Some(signin) = signin {
            status.signing_in = signin.signing_in;
            status.auth_url = signin.auth_url;
            status.awaiting_code = signin.awaiting_code;
            if signin.error.is_some() {
                status.error = signin.error;
            }
        }
        status
    }
    fn blank(id: ProviderId) -> ProviderStatus {
        let (description, connect) = match id {
            ProviderId::Claude => (
                "Your Claude subscription through Claude Code's sign-in.",
                ConnectMethod::BrowserCode,
            ),
            ProviderId::Codex => (
                "Your ChatGPT subscription through Codex's sign-in.",
                ConnectMethod::Browser,
            ),
            ProviderId::OpenRouter => (
                "Hundreds of models with one API key. Jev is the cheap, capable default.",
                ConnectMethod::ApiKey,
            ),
        };
        ProviderStatus {
            id,
            name: id.title().into(),
            description: description.into(),
            connect,
            connected: false,
            account: None,
            models: Vec::new(),
            error: None,
            signing_in: false,
            auth_url: None,
            awaiting_code: false,
        }
    }
    fn canonical(id: ProviderId, models: Vec<ProviderModel>) -> Vec<ProviderModel> {
        models
            .into_iter()
            .map(|model| ProviderModel {
                id: ModelRef {
                    provider: id,
                    model: model.id,
                }
                .canonical(),
                name: model.name,
                featured: model.featured,
            })
            .collect()
    }
    async fn claude_status(&self) -> ProviderStatus {
        let mut status = Self::blank(ProviderId::Claude);
        status.models = Self::canonical(ProviderId::Claude, claude::models());
        match self.claude.status().await {
            Ok(state) => {
                status.account = state.account();
                status.connected = state.logged_in;
            }
            Err(error) => status.error = Some(error.to_string()),
        }
        status
    }
    async fn codex_status(&self) -> ProviderStatus {
        let mut status = Self::blank(ProviderId::Codex);
        let home = self.codex.profile().to_path_buf();
        match tokio::task::spawn_blocking(move || codex::status_at(&home)).await {
            Ok(Ok((account, models))) => {
                status.connected = account.is_some();
                status.account = account;
                status.models = Self::canonical(
                    ProviderId::Codex,
                    models
                        .into_iter()
                        .map(|model| ProviderModel {
                            id: model.id,
                            name: model.name,
                            featured: false,
                        })
                        .collect(),
                );
            }
            Ok(Err(error)) => status.error = Some(error.to_string()),
            Err(_) => status.error = Some("Codex status is unavailable.".into()),
        }
        status
    }
    async fn openrouter_status(&self, refresh: bool) -> ProviderStatus {
        let mut status = Self::blank(ProviderId::OpenRouter);
        let cached = self
            .openrouter_models
            .lock()
            .expect("model cache")
            .clone()
            .filter(|(at, _)| at.elapsed() < MODELS_TTL)
            .map(|(_, models)| models);
        let models = match cached {
            Some(models) => models,
            None => match self.openrouter.models().await {
                Ok(models) => {
                    *self.openrouter_models.lock().expect("model cache") =
                        Some((Instant::now(), models.clone()));
                    models
                }
                Err(_) => openrouter::FEATURED
                    .iter()
                    .map(|(id, name)| ProviderModel {
                        id: (*id).into(),
                        name: (*name).into(),
                        featured: true,
                    })
                    .collect(),
            },
        };
        status.models = Self::canonical(ProviderId::OpenRouter, models);
        match self.openrouter.key() {
            Ok(Some(_)) => {
                if refresh {
                    match self.openrouter.account().await {
                        Ok(Some(account)) => {
                            status.connected = true;
                            status.account = Some(account.label);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            status.connected = true;
                            status.account = Some("API key stored".into());
                            status.error = Some(error.to_string());
                        }
                    }
                } else {
                    status.connected = true;
                    status.account = Some("API key stored in Keychain".into());
                }
            }
            Ok(None) => {}
            Err(error) => status.error = Some(error.message),
        }
        status
    }

    async fn connect(
        self: &Arc<Self>,
        id: ProviderId,
        request: ConnectRequest,
    ) -> Result<(), ApiError> {
        match id {
            ProviderId::OpenRouter => {
                let key = request.api_key.unwrap_or_default();
                self.openrouter.store_key(&key).map_err(|error| {
                    ApiError::new(StatusCode::BAD_REQUEST, "invalid", &error.to_string())
                })?;
                if let Err(error) = self.openrouter.account().await {
                    // Only a refused key is discarded; an unreachable service keeps it.
                    if error.downcast_ref::<openrouter::Rejected>().is_none() {
                        return Err(ApiError::new(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "unavailable",
                            &error.to_string(),
                        ));
                    }
                    let _ = self.openrouter.forget_key();
                    return Err(ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "rejected",
                        &error.to_string(),
                    ));
                }
                self.forget(id);
                Ok(())
            }
            ProviderId::Claude => {
                if let Some(code) = request.code {
                    let login = self.claude_login.lock().await.take().ok_or_else(|| {
                        ApiError::new(
                            StatusCode::CONFLICT,
                            "not_signing_in",
                            "Start the Claude sign-in first.",
                        )
                    })?;
                    let result = login.submit(&code).await;
                    let mut signin = self.claude_signin.lock().expect("sign-in");
                    *signin = SignIn::default();
                    if let Err(error) = result {
                        signin.error = Some(error.to_string());
                    }
                    self.forget(id);
                    return Ok(());
                }
                {
                    let mut signin = self.claude_signin.lock().expect("sign-in");
                    if signin.signing_in {
                        return Ok(());
                    }
                    *signin = SignIn {
                        signing_in: true,
                        ..Default::default()
                    };
                }
                let providers = self.clone();
                let result = self
                    .claude
                    .login(move |url| {
                        providers.claude_signin.lock().expect("sign-in").auth_url = Some(url);
                    })
                    .await;
                match result {
                    Ok(login) => {
                        // Cancelled while the CLI was starting: drop the child instead.
                        let still_wanted = self.claude_signin.lock().expect("sign-in").signing_in;
                        if !still_wanted {
                            login.cancel().await;
                            return Ok(());
                        }
                        *self.claude_login.lock().await = Some(login);
                        self.claude_signin.lock().expect("sign-in").awaiting_code = true;
                    }
                    Err(error) => {
                        *self.claude_signin.lock().expect("sign-in") = SignIn {
                            error: Some(error.to_string()),
                            ..Default::default()
                        };
                    }
                }
                Ok(())
            }
            ProviderId::Codex => {
                {
                    let mut signin = self.codex_signin.lock().expect("sign-in");
                    if signin.signing_in {
                        return Ok(());
                    }
                    *signin = SignIn {
                        signing_in: true,
                        ..Default::default()
                    };
                }
                self.codex_cancel.store(false, Ordering::Relaxed);
                let providers = self.clone();
                let home = self.codex.profile().to_path_buf();
                tokio::task::spawn_blocking(move || {
                    let cancel = providers.codex_cancel.clone();
                    let opener = providers.clone();
                    let result = codex::login_at(&home, cancel, move |url| {
                        opener.codex_signin.lock().expect("sign-in").auth_url = Some(url);
                    });
                    let mut signin = providers.codex_signin.lock().expect("sign-in");
                    *signin = SignIn::default();
                    if let Err(error) = result {
                        signin.error = Some(error.to_string());
                    }
                    drop(signin);
                    providers.forget(ProviderId::Codex);
                });
                Ok(())
            }
        }
    }
    async fn cancel(&self, id: ProviderId) {
        match id {
            ProviderId::Codex => self.codex_cancel.store(true, Ordering::Relaxed),
            ProviderId::Claude => {
                if let Some(login) = self.claude_login.lock().await.take() {
                    login.cancel().await;
                }
                *self.claude_signin.lock().expect("sign-in") = SignIn::default();
            }
            ProviderId::OpenRouter => {}
        }
    }
    async fn disconnect(&self, id: ProviderId, pool: &sqlx::PgPool) -> Result<(), ApiError> {
        match id {
            ProviderId::OpenRouter => self.openrouter.forget_key().map_err(|error| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unavailable",
                    &error.to_string(),
                )
            })?,
            ProviderId::Claude => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "shared_sign_in",
                    "Claude uses your Claude Code sign-in. Sign out in Terminal with `claude auth logout`.",
                ));
            }
            ProviderId::Codex => {
                let pending: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM turns WHERE state IN ('queued','running'))",
                )
                .fetch_one(pool)
                .await?;
                if pending {
                    return Err(ApiError::new(
                        StatusCode::CONFLICT,
                        "busy",
                        "Wait for accepted replies before signing out.",
                    ));
                }
                let home = self.codex.profile().to_path_buf();
                tokio::task::spawn_blocking(move || codex::logout_at(&home))
                    .await
                    .map_err(|_| {
                        ApiError::new(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "unavailable",
                            "Codex is unavailable.",
                        )
                    })?
                    .map_err(|error| {
                        ApiError::new(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "unavailable",
                            &error.to_string(),
                        )
                    })?;
                *self.codex_signin.lock().expect("sign-in") = SignIn::default();
            }
        }
        self.forget(id);
        Ok(())
    }
    /// One small real exchange through the selected model.
    pub async fn test(&self, model: &str) -> ProviderTest {
        let started = Instant::now();
        let outcome = async {
            let model = self.resolve_with(model, None)?;
            tokio::time::timeout(
                Duration::from_secs(120),
                model.complete(ModelRequest {
                    instructions:
                        "You are a connection test for AgentInc. Reply with exactly the word OK."
                            .into(),
                    messages: vec![Message::user("Connection test: reply with OK.")],
                    tools: vec![],
                }),
            )
            .await
            .map_err(|_| ModelError::retryable("The model did not answer within two minutes."))?
        }
        .await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match outcome {
            Ok(response) => ProviderTest {
                ok: true,
                model: model.into(),
                reply: Some(Message::assistant(response.content).text()),
                elapsed_ms,
                error: None,
            },
            Err(error) => ProviderTest {
                ok: false,
                model: model.into(),
                reply: None,
                elapsed_ms,
                error: Some(error.message),
            },
        }
    }
}
impl ModelCatalog for Providers {
    fn resolve(&self, id: &str) -> Result<Arc<dyn Model>, ModelError> {
        self.resolve_with(id, None)
    }
    fn resolve_streaming(&self, id: &str, sink: DeltaSink) -> Result<Arc<dyn Model>, ModelError> {
        self.resolve_with(id, Some(sink))
    }
}

#[derive(Clone)]
struct ProvidersApi {
    product: Product,
    providers: Arc<Providers>,
}
pub fn router(product: Product, providers: Arc<Providers>) -> Router {
    Router::new()
        .route("/v1/providers", get(state))
        .route("/v1/providers/{id}/connect", post(connect))
        .route("/v1/providers/{id}/cancel", post(cancel))
        .route("/v1/providers/{id}/disconnect", post(disconnect))
        .route("/v1/providers/{id}/test", post(test))
        .with_state(ProvidersApi { product, providers })
}
fn provider(id: &str) -> Result<ProviderId, ApiError> {
    ProviderId::parse(id).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown_provider",
            "Choose Claude, ChatGPT or OpenRouter.",
        )
    })
}
async fn snapshot(api: &ProvidersApi, refresh: bool) -> Result<ProvidersState, ApiError> {
    let workspace = crate::workspaces::current(&api.product.pool).await?;
    let default_model: Option<String> = sqlx::query_scalar(
        "SELECT value FROM assistant_settings WHERE workspace_id=$1 AND key='model'",
    )
    .bind(&workspace)
    .fetch_optional(&api.product.pool)
    .await?;
    let providers = futures::future::join_all(
        ProviderId::ALL
            .into_iter()
            .map(|id| api.providers.status(id, refresh)),
    )
    .await;
    Ok(ProvidersState {
        providers,
        default_model: default_model.filter(|m| !m.is_empty() && m != "connection-default"),
    })
}

#[utoipa::path(get, path = "/v1/providers", operation_id = "providers_state", params(("refresh" = Option<bool>, Query, description = "Re-check every provider instead of using the recent snapshot")), responses((status = 200, body = ProvidersState), (status = 401, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn state(
    State(api): State<ProvidersApi>,
    headers: HeaderMap,
    Query(query): Query<StatusQuery>,
) -> Result<Json<ProvidersState>, ApiError> {
    api.product.authorize(&headers)?;
    Ok(Json(snapshot(&api, query.refresh).await?))
}
#[utoipa::path(post, path = "/v1/providers/{id}/connect", operation_id = "provider_connect", params(("id" = String, Path, description = "claude, codex or openrouter")), request_body = ConnectRequest, responses((status = 200, body = ProvidersState), (status = 400, body = ErrorBody), (status = 401, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn connect(
    State(api): State<ProvidersApi>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ConnectRequest>,
) -> Result<Json<ProvidersState>, ApiError> {
    api.product.authorize(&headers)?;
    let id = provider(&id)?;
    api.providers.connect(id, request).await?;
    Ok(Json(snapshot(&api, false).await?))
}
#[utoipa::path(post, path = "/v1/providers/{id}/cancel", operation_id = "provider_cancel", params(("id" = String, Path, description = "claude, codex or openrouter")), responses((status = 200, body = ProvidersState), (status = 401, body = ErrorBody), (status = 404, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn cancel(
    State(api): State<ProvidersApi>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ProvidersState>, ApiError> {
    api.product.authorize(&headers)?;
    let id = provider(&id)?;
    api.providers.cancel(id).await;
    Ok(Json(snapshot(&api, false).await?))
}
#[utoipa::path(post, path = "/v1/providers/{id}/disconnect", operation_id = "provider_disconnect", params(("id" = String, Path, description = "claude, codex or openrouter")), responses((status = 200, body = ProvidersState), (status = 401, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn disconnect(
    State(api): State<ProvidersApi>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ProvidersState>, ApiError> {
    api.product.authorize(&headers)?;
    let id = provider(&id)?;
    api.providers.disconnect(id, &api.product.pool).await?;
    Ok(Json(snapshot(&api, false).await?))
}
#[utoipa::path(post, path = "/v1/providers/{id}/test", operation_id = "provider_test", params(("id" = String, Path, description = "claude, codex or openrouter")), request_body = TestRequest, responses((status = 200, body = ProviderTest), (status = 400, body = ErrorBody), (status = 401, body = ErrorBody), (status = 404, body = ErrorBody), (status = 503, body = ErrorBody)))]
async fn test(
    State(api): State<ProvidersApi>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<TestRequest>,
) -> Result<Json<ProviderTest>, ApiError> {
    api.product.authorize(&headers)?;
    let id = provider(&id)?;
    let model = match request.model {
        Some(model) if ModelRef::parse(&model).provider == id => model,
        Some(_) => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose a model from this provider.",
            ));
        }
        None => {
            let status = api.providers.status(id, false).await;
            status
                .models
                .first()
                .map(|model| model.id.clone())
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid",
                        "Choose a model to test.",
                    )
                })?
        }
    };
    Ok(Json(api.providers.test(&model).await))
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    #[derive(utoipa::OpenApi)]
    #[openapi(
        paths(state, connect, cancel, disconnect, test),
        components(schemas(
            ProviderId,
            ProviderModel,
            ConnectMethod,
            ProviderStatus,
            ProvidersState,
            ConnectRequest,
            TestRequest,
            ProviderTest
        ))
    )]
    struct Api;
    <Api as utoipa::OpenApi>::openapi()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_references_are_namespaced_with_legacy_fallback() {
        let jev = ModelRef::parse("openrouter:typesafe/jev-router");
        assert_eq!(jev.provider, ProviderId::OpenRouter);
        assert_eq!(jev.model, "typesafe/jev-router");
        assert_eq!(jev.canonical(), "openrouter:typesafe/jev-router");
        assert_eq!(
            ModelRef::parse("claude:claude-sonnet-5").provider,
            ProviderId::Claude
        );
        let legacy = ModelRef::parse("gpt-5-codex");
        assert_eq!(legacy.provider, ProviderId::Codex);
        assert_eq!(legacy.model, "gpt-5-codex");
        assert_eq!(
            ModelRef::parse("connection-default").model,
            "connection-default"
        );
        assert_eq!(ModelRef::parse("mystery:x").model, "mystery:x");
    }
    #[tokio::test]
    async fn offline_providers_report_disconnected_and_refuse_work_safely() {
        let dir = tempfile::tempdir().unwrap();
        let providers = Arc::new(Providers::offline(dir.path()).unwrap());
        let openrouter = providers.status(ProviderId::OpenRouter, false).await;
        assert!(!openrouter.connected);
        assert!(
            openrouter
                .models
                .iter()
                .any(|m| m.id == "openrouter:typesafe/jev-router" && m.featured)
        );
        let claude = providers.status(ProviderId::Claude, false).await;
        assert!(!claude.connected);
        assert!(claude.error.is_some());
        assert_eq!(claude.models[0].id, "claude:claude-fable-5-1");
        let test = providers.test("openrouter:typesafe/jev-router").await;
        assert!(!test.ok);
        assert!(test.error.unwrap().contains("Connect OpenRouter"));
        assert!(
            providers
                .disconnect(
                    ProviderId::Claude,
                    &sqlx::PgPool::connect_lazy("postgres://x@127.0.0.1:1/x").unwrap()
                )
                .await
                .is_err()
        );
    }
}
