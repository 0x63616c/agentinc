use crate::{Error, Event, Message, engine::SessionHandle};
use futures::stream::BoxStream;
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
    /// Request cancellation. Already completed external effects are not undone.
    pub async fn cancel(&self) -> Result<(), Error> {
        self.handle.cancel().await
    }

    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Deliver a message. Returns once it is accepted, not when the reply is ready.
    pub async fn send(&self, message: impl Into<Message>) -> Result<(), Error> {
        self.handle.send(message.into()).await
    }

    /// Deliver once under a caller-owned ID. Bind the ID to an immutable message
    /// in your command receipt; retries with the same ID are ignored.
    pub async fn send_once(
        &self,
        id: impl Into<String>,
        message: impl Into<Message>,
    ) -> Result<(), Error> {
        self.handle.send_once(id.into(), message.into()).await
    }

    /// Drop every message the model has not yet seen. Returns them, in send order.
    pub async fn clear_pending(&self) -> Result<Vec<Message>, Error> {
        self.handle.clear_pending().await
    }

    /// Everything that has happened in this session, then everything that happens next.
    ///
    /// Starts from the beginning, so a fresh subscriber catches up on history first. Ends
    /// only when the session itself ends; stop reading when you have what you need.
    pub fn events(&self) -> BoxStream<'static, Result<Event, Error>> {
        self.events_from(0)
    }

    /// Resume reading at a previously persisted event offset.
    pub fn events_from(&self, offset: usize) -> BoxStream<'static, Result<Event, Error>> {
        self.handle.events_from(offset)
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("id", &self.id).finish()
    }
}
