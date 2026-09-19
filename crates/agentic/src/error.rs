/// Errors returned by the SDK.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The run finished with a failure.
    #[error("run failed: {0}")]
    RunFailed(String),
    /// The run was cancelled.
    #[error("run cancelled")]
    Cancelled,
    /// The agent hit its turn limit before producing an answer.
    #[error("run exceeded {0} turns")]
    TurnLimit(u32),
    /// Could not reach or start the runtime.
    #[error("connection error: {0}")]
    Connection(String),
    /// Something else went wrong.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
