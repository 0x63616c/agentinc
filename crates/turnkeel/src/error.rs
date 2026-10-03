/// Errors returned by the SDK.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The requested run or session does not exist in retained history.
    #[error("run or session not found")]
    NotFound,
    /// The run finished with a failure.
    #[error("run failed: {0}")]
    RunFailed(String),
    /// The run was cancelled.
    #[error("run cancelled")]
    Cancelled,
    /// Could not reach or start the runtime.
    #[error("connection error: {0}")]
    Connection(String),
    /// The caller passed something the runtime cannot act on, such as a page token
    /// from another deployment. Retrying the same input will not help.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Something else went wrong.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
