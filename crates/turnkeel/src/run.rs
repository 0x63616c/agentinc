use crate::{Error, Event, Message, engine::RunHandle};
use futures::{StreamExt, stream, stream::BoxStream};
use serde::{Deserialize, Serialize};

/// Identifies an agent run. Stable for the run's whole life, including across restarts and
/// retries. (Not to be confused with Temporal's per-attempt run id, which is never exposed.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub(crate) String);

impl RunId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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
    /// Request cancellation. Already completed external effects are not undone.
    pub async fn cancel(&self) -> Result<(), Error> {
        self.handle.cancel().await
    }

    pub fn id(&self) -> &RunId {
        &self.id
    }

    /// Recorded messages and tool results, followed by live events until the run ends.
    /// Each new subscriber starts at the beginning; terminal failures end with an error.
    pub fn events(&self) -> BoxStream<'static, Result<Event, Error>> {
        let handle = self.handle.clone();
        stream::unfold(Some((handle, 0usize)), |state| async move {
            let (handle, offset) = state?;
            match handle.events_after(offset).await {
                Ok((events, done)) => {
                    let next = offset + events.len();
                    let batch: Vec<Result<Event, Error>> = events.into_iter().map(Ok).collect();
                    Some((
                        stream::iter(batch),
                        if done { None } else { Some((handle, next)) },
                    ))
                }
                Err(error) => Some((stream::iter(vec![Err(error)]), None)),
            }
        })
        .flatten()
        .boxed()
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
