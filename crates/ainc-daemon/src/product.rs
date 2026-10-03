//! Single-owner product commands. A committed receipt is the acknowledgement;
//! clients never own SQL or the lifetime of accepted turns.
use crate::api::CommandError;
pub use crate::api::{ErrorBody, Product};

/// Legacy name for [`CommandError`], kept for `temporal.rs` until WP-D5 lands.
pub(crate) type ApiError = CommandError;
impl CommandError {
    /// Legacy constructor for `temporal.rs`; new code names a variant.
    pub(crate) fn new(status: axum::http::StatusCode, _code: &str, message: &str) -> Self {
        use axum::http::StatusCode;
        match status {
            StatusCode::BAD_REQUEST => Self::Invalid(message.into()),
            StatusCode::CONFLICT => Self::Conflict(message.into()),
            StatusCode::UNAUTHORIZED => Self::Unauthorized,
            StatusCode::FORBIDDEN => Self::Forbidden,
            StatusCode::NOT_FOUND => Self::NotFound,
            StatusCode::SERVICE_UNAVAILABLE => Self::Unavailable(message.into()),
            _ => Self::Internal,
        }
    }
}
