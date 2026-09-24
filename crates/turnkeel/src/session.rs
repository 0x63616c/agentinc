use crate::{Error, Event, Message, engine::SessionHandle};
use futures::{StreamExt, stream, stream::BoxStream};
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

    /// Drop every message the model has not yet seen. Returns them, in send order.
    pub async fn clear_pending(&self) -> Result<Vec<Message>, Error> {
        self.handle.clear_pending().await
    }

    /// Everything that has happened in this session, then everything that happens next.
    ///
    /// Starts from the beginning, so a fresh subscriber catches up on history first. Never
    /// ends on its own; stop reading when you have what you need.
    pub fn events(&self) -> BoxStream<'static, Result<Event, Error>> {
        let handle = self.handle.clone();
        stream::unfold(Some((handle, 0usize)), |state| async move {
            let (handle, offset) = state?;
            match handle.events_after(offset).await {
                Ok(events) => {
                    let next = offset + events.len();
                    let batch: Vec<Result<Event, Error>> = events.into_iter().map(Ok).collect();
                    Some((stream::iter(batch), Some((handle, next))))
                }
                Err(e) => Some((stream::iter(vec![Err(e)]), None)),
            }
        })
        .flatten()
        .boxed()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("id", &self.id).finish()
    }
}
