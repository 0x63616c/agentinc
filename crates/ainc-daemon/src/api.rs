//! The HTTP seam every product module shares: who is calling (`Owner`, `Actor`),
//! how a refused command is reported (`CommandError` → one `ErrorBody`), and the
//! one router every endpoint is registered on exactly once.
use crate::{connection::Connection, terminals, tickets::Actor, workspaces};
use axum::{
    Json, Router,
    extract::{FromRef, FromRequestParts, Request},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower_http::{set_header::SetResponseHeaderLayer, trace::TraceLayer};
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;

/// The product database and the owner credential every endpoint checks.
#[derive(Clone)]
pub struct Product {
    pub pool: PgPool,
    token: String,
}
impl Product {
    pub fn new(pool: PgPool, token: String) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !token.trim().is_empty(),
            "owner credential must not be empty"
        );
        Ok(Self { pool, token })
    }
    pub(crate) fn authorize(&self, headers: &HeaderMap) -> Result<(), CommandError> {
        if bearer(headers) != Some(self.token.as_str()) {
            return Err(CommandError::Unauthorized);
        }
        Ok(())
    }
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

/// The owner, proven by the owner credential, acting in the current Workspace.
pub struct Owner {
    pub workspace: String,
}
impl Owner {
    pub(crate) fn actor(self) -> Actor {
        Actor::owner_in(self.workspace)
    }
}
impl<S> FromRequestParts<S> for Owner
where
    Product: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = CommandError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, CommandError> {
        let product = Product::from_ref(state);
        product.authorize(&parts.headers)?;
        Ok(Self {
            workspace: workspaces::current(&product.pool).await?,
        })
    }
}

/// Whoever holds a credential: the owner in the current Workspace, or an agent
/// named by an assignment credential. Whether that assignment is still live is
/// decided by the fence inside each read or write transaction.
impl<S> FromRequestParts<S> for Actor
where
    Product: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = CommandError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, CommandError> {
        let product = Product::from_ref(state);
        if product.authorize(&parts.headers).is_ok() {
            return Ok(Actor::owner_in(workspaces::current(&product.pool).await?));
        }
        let token = bearer(&parts.headers).ok_or(CommandError::Unauthorized)?;
        let row: Option<(String, String, i64, i64)> = sqlx::query_as("SELECT c.workspace_id,r.agent_id,r.ticket_id,r.generation FROM agent_credentials c JOIN ticket_runs r ON r.run_id=c.run_id WHERE c.token_hash=$1")
            .bind(format!("{:x}", Sha256::digest(token.as_bytes()))).fetch_optional(&product.pool).await?;
        let (workspace, id, ticket, generation) = row.ok_or(CommandError::Unauthorized)?;
        Ok(Actor {
            workspace,
            id,
            assignment: Some((ticket, generation)),
            conversation: None,
        })
    }
}

/// Why a command was refused. Every module returns this; the HTTP status, the
/// wire body and the tool-facing message all derive from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// The request itself is wrong; sending it again unchanged will fail again.
    Invalid(String),
    /// The record changed or the operation ID was reused; refresh and retry.
    Conflict(String),
    /// No credential, or one this daemon does not know.
    Unauthorized,
    /// A known credential acting outside what it is allowed to touch.
    Forbidden,
    /// The record does not exist in the caller's Workspace.
    NotFound,
    /// A dependency is down; the same request may succeed later.
    Unavailable(String),
    /// The daemon failed; the cause is in its log.
    Internal,
}
impl CommandError {
    /// The common Conflict: a revision or neighbour the client read is stale.
    pub(crate) fn conflict() -> Self {
        Self::Conflict("The record changed. Refresh and try again.".into())
    }
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Invalid(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Invalid,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::Unauthorized => ErrorCode::Unauthorized,
            Self::Forbidden => ErrorCode::Forbidden,
            Self::NotFound => ErrorCode::NotFound,
            Self::Unavailable(_) => ErrorCode::Unavailable,
            Self::Internal => ErrorCode::Internal,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            Self::Invalid(message) | Self::Conflict(message) | Self::Unavailable(message) => {
                message
            }
            Self::Unauthorized => "Owner credential required.",
            Self::Forbidden => "This command is outside the current assignment.",
            Self::NotFound => "The record does not exist.",
            Self::Internal => "The daemon failed. Try again.",
        }
    }
    pub fn body(&self) -> ErrorBody {
        ErrorBody {
            code: self.code(),
            message: self.message().to_owned(),
        }
    }
}
impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for CommandError {}
impl IntoResponse for CommandError {
    fn into_response(self) -> Response {
        (self.status(), Json(self.body())).into_response()
    }
}
/// Constraint violations are the client's; anything else is the daemon's and
/// is logged, so a SQL bug never hides behind "try again".
impl From<sqlx::Error> for CommandError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref e) = error {
            if e.is_unique_violation() || e.is_foreign_key_violation() {
                return Self::conflict();
            }
            if e.is_check_violation() {
                return Self::Invalid("The value is empty or too long.".into());
            }
        }
        tracing::error!(%error, "database request failed");
        Self::Internal
    }
}
/// A refused command reaches the model with its reason intact.
impl From<CommandError> for turnkeel::ToolError {
    fn from(error: CommandError) -> Self {
        let message = error.message().to_owned();
        match error {
            CommandError::Invalid(_)
            | CommandError::Conflict(_)
            | CommandError::Forbidden
            | CommandError::NotFound => Self::InvalidArguments(message),
            CommandError::Unauthorized | CommandError::Unavailable(_) | CommandError::Internal => {
                Self::Failed(message)
            }
        }
    }
}

/// The closed set of error codes a client can switch on.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Invalid,
    Conflict,
    Unauthorized,
    Forbidden,
    NotFound,
    Unavailable,
    Internal,
    UpgradeRequired,
}
/// The one shape every non-2xx response has.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

/// Everything a handler can extract: the product, its Connection and the
/// terminal registry. Each module's state is a `FromRef` view of this.
#[derive(Clone)]
pub struct AppState {
    pub product: Product,
    pub connection: Connection,
    pub(crate) terminals: terminals::Sessions,
}
impl FromRef<AppState> for Product {
    fn from_ref(state: &AppState) -> Self {
        state.product.clone()
    }
}
impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.product.pool.clone()
    }
}

pub(crate) use ainc_release::{CLIENT_HEADER, SERVER_HEADER};

/// Every product endpoint, each registered once by its module.
fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(crate::health::routes())
        .merge(crate::conversations::routes())
        .merge(crate::connection::routes())
        .merge(crate::tickets::routes())
        .merge(crate::automations::routes())
        .merge(terminals::routes())
        .merge(workspaces::routes())
}

/// The whole document, from the same registrations the router serves.
pub fn openapi() -> utoipa::openapi::OpenApi {
    #[derive(OpenApi)]
    #[openapi(components(schemas(ErrorBody, ErrorCode)))]
    struct Shared;
    let mut api = Shared::openapi();
    api.merge(routes().into_openapi());
    api.merge(crate::work::routes().into_openapi());
    declare_shared_errors(&mut api);
    api
}

/// Every `/v1/` operation can be refused by the credential check, the client
/// compatibility check and a daemon failure. They are declared once here.
fn declare_shared_errors(api: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::{Ref, RefOr, Response, ResponseBuilder, Responses};
    let shared = [
        ("Unauthorized", "Owner credential required."),
        ("UpgradeRequired", "Update to continue."),
        ("Internal", "The daemon failed; see its log."),
    ];
    let components = api.components.get_or_insert_with(Default::default);
    for (name, description) in shared {
        components.responses.insert(
            name.to_owned(),
            RefOr::T(
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        utoipa::openapi::ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name("ErrorBody")))
                            .build(),
                    )
                    .build(),
            ),
        );
    }
    let reference = |name: &str| -> RefOr<Response> { RefOr::Ref(Ref::from_response_name(name)) };
    for (path, item) in api.paths.paths.iter_mut() {
        if !path.starts_with("/v1/") {
            continue;
        }
        for operation in [
            &mut item.get,
            &mut item.post,
            &mut item.put,
            &mut item.patch,
            &mut item.delete,
        ]
        .into_iter()
        .flatten()
        {
            let responses: &mut Responses = &mut operation.responses;
            for (status, name) in [
                ("401", "Unauthorized"),
                ("426", "UpgradeRequired"),
                ("500", "Internal"),
            ] {
                responses
                    .responses
                    .insert(status.to_owned(), reference(name));
            }
        }
    }
}

/// The served application: every module's routes plus `extra` (the daemon's
/// runtime visibility), with the compatibility check, the server version header
/// and request tracing applied exactly once.
pub fn app(product: Product, connection: Connection, extra: Router) -> Router {
    let state = AppState {
        product,
        connection,
        terminals: terminals::Sessions::default(),
    };
    let (router, _) = routes().with_state(state).split_for_parts();
    router
        .merge(extra)
        .layer(axum::middleware::from_fn(compatibility))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static(SERVER_HEADER),
            HeaderValue::from_str(&ainc_release::server_header()).expect("valid product header"),
        ))
        .layer(TraceLayer::new_for_http())
}

async fn compatibility(request: Request, next: Next) -> Response {
    if request.uri().path().starts_with("/v1/") {
        let header = request
            .headers()
            .get(CLIENT_HEADER)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if ainc_release::check_client(header, ainc_release::MIN_CLIENT, ainc_release::API).is_err()
        {
            return (
                StatusCode::UPGRADE_REQUIRED,
                Json(ErrorBody {
                    code: ErrorCode::UpgradeRequired,
                    message: "Update to continue".into(),
                }),
            )
                .into_response();
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_a_closed_snake_case_set() {
        assert_eq!(
            serde_json::to_value(CommandError::NotFound.body()).unwrap(),
            serde_json::json!({"code":"not_found","message":"The record does not exist."})
        );
        assert_eq!(
            serde_json::to_value(ErrorCode::UpgradeRequired).unwrap(),
            "upgrade_required"
        );
        let body: ErrorBody =
            serde_json::from_str(r#"{"code":"conflict","message":"stale"}"#).unwrap();
        assert_eq!(body.code, ErrorCode::Conflict);
        assert!(serde_json::from_str::<ErrorBody>(r#"{"code":"busy","message":""}"#).is_err());
    }

    #[test]
    fn tool_errors_keep_the_reason() {
        let refused: turnkeel::ToolError =
            CommandError::Conflict("revision 3 is stale".into()).into();
        assert_eq!(
            refused.to_string(),
            "invalid arguments: revision 3 is stale"
        );
        let failed: turnkeel::ToolError = CommandError::Internal.into();
        assert_eq!(failed.to_string(), "The daemon failed. Try again.");
    }

    #[test]
    fn database_bugs_are_internal_and_constraints_are_the_clients() {
        assert_eq!(
            CommandError::from(sqlx::Error::RowNotFound),
            CommandError::Internal
        );
        assert_eq!(
            CommandError::Internal.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn every_v1_operation_declares_the_shared_errors_once() {
        let api = serde_json::to_value(openapi()).unwrap();
        for (path, item) in api["paths"].as_object().unwrap() {
            for (method, operation) in item.as_object().unwrap() {
                let responses = &operation["responses"];
                if path.starts_with("/v1/") {
                    for status in ["401", "426", "500"] {
                        assert!(
                            responses[status]["$ref"].is_string(),
                            "{method} {path} lacks the shared {status} response"
                        );
                    }
                } else {
                    assert!(
                        responses["426"].is_null(),
                        "{method} {path} is not versioned"
                    );
                }
            }
        }
    }
}
