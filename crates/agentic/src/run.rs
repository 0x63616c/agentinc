use crate::{Error, Message, engine::RunHandle};
use serde::{Deserialize, Serialize};

/// Identifies a run. Stable across restarts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub(crate) String);

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A handle to a running or finished agent run.
pub struct Run {
    pub(crate) id: RunId,
    pub(crate) handle: RunHandle,
}

impl Run {
    pub fn id(&self) -> &RunId {
        &self.id
    }

    /// Wait for the run to finish and return the final answer.
    pub async fn result(&self) -> Result<String, Error> {
        Ok(self.handle.output().await?.text)
    }

    /// Wait for the run to finish and return the full conversation.
    pub async fn transcript(&self) -> Result<Vec<Message>, Error> {
        Ok(self.handle.output().await?.messages)
    }
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run").field("id", &self.id).finish()
    }
}
