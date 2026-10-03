//! Client-side handles to one run or one session. Cloneable, cheap.

use super::{
    session::{Delivery, SessionWorkflow, SessionWorkflowType},
    workflow::{AgentRunWorkflow, RunOutput, RunWorkflowType},
};
use crate::{Error, Event, Message};
use futures::{StreamExt, stream, stream::BoxStream};
use std::sync::Arc;
use temporalio_client::{
    Client, WorkflowCancelOptions, WorkflowExecuteUpdateOptions, WorkflowGetResultOptions,
    WorkflowHandle, WorkflowSignalOptions,
    errors::{WorkflowGetResultError, WorkflowInteractionError},
};

/// Recorded events from `offset`, then live events, as one stream. `poll` returns the next
/// batch and whether the execution has closed; a closed execution ends the stream.
pub(crate) fn event_stream<F, Fut>(
    offset: usize,
    poll: F,
) -> BoxStream<'static, Result<Event, Error>>
where
    F: Fn(usize) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(Vec<Event>, bool), Error>> + Send,
{
    stream::unfold(Some((poll, offset)), |state| async move {
        let (poll, offset) = state?;
        match poll(offset).await {
            Ok((events, done)) => {
                let next = offset + events.len();
                let batch: Vec<Result<Event, Error>> = events.into_iter().map(Ok).collect();
                Some((
                    stream::iter(batch),
                    if done { None } else { Some((poll, next)) },
                ))
            }
            Err(error) => Some((stream::iter(vec![Err(error)]), None)),
        }
    })
    .flatten()
    .boxed()
}

async fn cancel<T: temporalio_common::HasWorkflowDefinition>(
    handle: &WorkflowHandle<Client, T>,
) -> Result<(), Error> {
    handle
        .cancel(WorkflowCancelOptions::default())
        .await
        .map_err(|e| match e {
            WorkflowInteractionError::NotFound(_) => Error::NotFound,
            other => Error::Other(other.into()),
        })
}

#[derive(Clone)]
pub(crate) struct RunHandle {
    inner: Arc<WorkflowHandle<Client, RunWorkflowType>>,
}

impl RunHandle {
    pub(crate) fn new(handle: WorkflowHandle<Client, RunWorkflowType>) -> Self {
        Self {
            inner: Arc::new(handle),
        }
    }

    pub(crate) fn events(&self) -> BoxStream<'static, Result<Event, Error>> {
        let handle = self.clone();
        event_stream(0, move |offset| {
            let handle = handle.clone();
            async move { handle.events_after(offset).await }
        })
    }

    async fn events_after(&self, offset: usize) -> Result<(Vec<Event>, bool), Error> {
        let completed = |output: RunOutput| {
            // Old completed histories predate the event field.
            let log = if output.events.is_empty() {
                output
                    .messages
                    .into_iter()
                    .map(Event::Message)
                    .chain([Event::TurnEnded])
                    .collect()
            } else {
                output.events
            };
            (log.get(offset..).unwrap_or_default().to_vec(), true)
        };
        tokio::select! {
            output = self.output() => output.map(completed),
            events = self.inner.execute_update(AgentRunWorkflow::events_after, offset, WorkflowExecuteUpdateOptions::default()) => {
                match events {
                    Ok(events) => Ok((events, false)),
                    // An execution can close between starting the long poll and its acceptance.
                    Err(_) => self.output().await.map(completed),
                }
            }
        }
    }

    pub(crate) async fn cancel(&self) -> Result<(), Error> {
        cancel(&self.inner).await
    }

    pub(crate) async fn output(&self) -> Result<RunOutput, Error> {
        self.inner
            .get_result(WorkflowGetResultOptions::default())
            .await
            .map_err(result_error)
    }
}

#[derive(Clone)]
pub(crate) struct SessionHandle {
    inner: Arc<WorkflowHandle<Client, SessionWorkflowType>>,
}

impl SessionHandle {
    pub(crate) fn new(handle: WorkflowHandle<Client, SessionWorkflowType>) -> Self {
        Self {
            inner: Arc::new(handle),
        }
    }

    pub(crate) async fn send_once(&self, id: String, message: Message) -> Result<(), Error> {
        self.inner
            .signal(
                SessionWorkflow::send_once,
                Delivery { id, message },
                WorkflowSignalOptions::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    pub(crate) async fn cancel(&self) -> Result<(), Error> {
        cancel(&self.inner).await
    }

    pub(crate) async fn send(&self, message: Message) -> Result<(), Error> {
        self.inner
            .signal(
                SessionWorkflow::send,
                message,
                WorkflowSignalOptions::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    pub(crate) async fn clear_pending(&self) -> Result<Vec<Message>, Error> {
        self.inner
            .execute_update(
                SessionWorkflow::clear_pending,
                (),
                WorkflowExecuteUpdateOptions::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    pub(crate) fn events_from(&self, offset: usize) -> BoxStream<'static, Result<Event, Error>> {
        let handle = self.clone();
        event_stream(offset, move |offset| {
            let handle = handle.clone();
            async move { handle.events_after(offset).await }
        })
    }

    async fn events_after(&self, offset: usize) -> Result<(Vec<Event>, bool), Error> {
        let closed = async {
            self.inner
                .get_result(WorkflowGetResultOptions::default())
                .await
                .map_err(result_error)?;
            Ok((Vec::new(), true))
        };
        tokio::pin!(closed);
        tokio::select! {
            result = &mut closed => result,
            events = self.inner.execute_update(SessionWorkflow::events_after, offset, WorkflowExecuteUpdateOptions::default()) => {
                match events { Ok(events) => Ok((events, false)), Err(_) => closed.await }
            }
        }
    }
}

/// Walks the error chain to the innermost message, which is the one the user wrote.
fn root_message(err: &dyn std::error::Error) -> String {
    let mut cur = err;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}

fn result_error(error: WorkflowGetResultError) -> Error {
    match error {
        WorkflowGetResultError::Cancelled { .. } => Error::Cancelled,
        WorkflowGetResultError::NotFound(_) => Error::NotFound,
        other if other.is_workflow_outcome() => Error::RunFailed(root_message(&other)),
        other => Error::Connection(root_message(&other)),
    }
}
