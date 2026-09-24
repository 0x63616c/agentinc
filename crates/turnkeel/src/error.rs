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
    /// Something else went wrong.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
