use crate::{Error, Message, engine::SessionHandle};
use serde::{Deserialize, Serialize};

/// Identifies a session. Stable for its whole life, including across restarts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub(crate) String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A conversation with one agent that stays open across many messages.
///
/// Messages are delivered immediately. If a turn is in flight the model sees the new message
/// at its next step; otherwise a new turn starts.
pub struct Session {
    pub(crate) id: SessionId,
    pub(crate) handle: SessionHandle,
}

impl Session {
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Deliver a message. Returns once it is accepted, not when the reply is ready.
    pub async fn send(&self, message: impl Into<Message>) -> Result<(), Error> {
        self.handle.send(message.into()).await
    }

    /// Drop every message the model has not yet seen. Returns them, in send order.
    pub async fn clear_pending(&self) -> Result<Vec<Message>, Error> {
        self.handle.clear_pending().await
    }

    /// Wait until the agent has nothing left to do: no turn running, nothing pending.
    pub async fn wait_idle(&self) -> Result<(), Error> {
        self.handle.wait_idle().await
    }

    /// Everything the model has seen so far.
    pub async fn transcript(&self) -> Result<Vec<Message>, Error> {
        self.handle.transcript().await
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("id", &self.id).finish()
    }
}
