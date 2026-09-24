//! Everything that knows about Temporal lives here. Nothing in this module is public.

mod activities;
mod conversation;
mod session;
mod test_server;
mod workflow;
pub(crate) use test_server::TestServer;

use crate::{Agent, Error, Event, Message, RunId, RuntimeConfig, SessionId};
use activities::{AgentActivities, Registry};
use std::sync::Arc;
use std::thread::JoinHandle;
use temporalio_client::{
    Client, ClientOptions, ConnectionOptions, WorkflowCancelOptions, WorkflowExecuteUpdateOptions,
    WorkflowGetResultOptions, WorkflowHandle, WorkflowIdReusePolicy, WorkflowSignalOptions,
    WorkflowStartOptions,
    errors::{WorkflowGetResultError, WorkflowStartError},
};
use temporalio_sdk::{
    Runtime, Worker, WorkerOptions,
    testing::{LocalServer, LocalWorkflowEnvironmentOptions, WorkflowEnvironment},
};

type ShutdownFn = Box<dyn Fn() + Send + Sync>;

/// Engine behaviour switches. Internal; surfaced through `Runtime::local()` / `Runtime::test()`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EngineOptions {
    pub check_idempotency: bool,
}
use conversation::agent_spec;
use session::{SessionInput, SessionWorkflow, SessionWorkflowType};
use workflow::{AgentRunWorkflow, RunInput, RunOutput, RunWorkflowType};

pub(crate) use session::SESSION_ID_PREFIX;
pub(crate) use workflow::RUN_ID_PREFIX;

pub(crate) struct Engine {
    client: Client,
    task_queue: String,
    registry: Registry,
    shutdown_worker: ShutdownFn,
    worker_thread: Option<JoinHandle<()>>,
    local: Option<WorkflowEnvironment<LocalServer>>,
}

impl Engine {
    pub(crate) async fn local(options: EngineOptions) -> Result<Self, Error> {
        // SDK default: download the pinned Temporal CLI once, cache it in the OS temp dir.
        let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::default())
            .await
            .map_err(|e| Error::Connection(e.to_string()))?;
        let client = env.client().clone();
        Self::with_client(
            client,
            Some(env),
            options,
            format!("agentinc-{}", uuid::Uuid::new_v4()),
            &[],
        )
        .await
    }

    pub(crate) async fn connect(url: &str) -> Result<Self, Error> {
        Self::configured(
            RuntimeConfig {
                endpoint: url.into(),
                scope: "default".into(),
                worker_group: format!("agentinc-{}", uuid::Uuid::new_v4()),
            },
            &[],
        )
        .await
    }

    pub(crate) async fn configured(config: RuntimeConfig, agents: &[Agent]) -> Result<Self, Error> {
        if config.scope.trim().is_empty() || config.worker_group.trim().is_empty() {
            return Err(Error::Connection(
                "runtime scope and worker group must be nonempty".into(),
            ));
        }
        let target: temporalio_client::Url = config
            .endpoint
            .parse()
            .map_err(|e| Error::Connection(format!("invalid runtime endpoint: {e}")))?;
        let client = Client::connect(
            ConnectionOptions::new(target).identity("turnkeel").build(),
            ClientOptions::new(config.scope).build(),
        )
        .await
        .map_err(|e| Error::Connection(e.to_string()))?;
        Self::with_client(
            client,
            None,
            EngineOptions::default(),
            config.worker_group,
            agents,
        )
        .await
    }

    async fn with_client(
        client: Client,
        local: Option<WorkflowEnvironment<LocalServer>>,
        options: EngineOptions,
        task_queue: String,
        agents: &[Agent],
    ) -> Result<Self, Error> {
        let registry = Registry::default();
        for agent in agents {
            registry.register(agent);
        }

        // The worker future is !Send, so it gets its own thread and single-threaded runtime.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let worker_client = client.clone();
        let worker_queue = task_queue.clone();
        let worker_registry = registry.clone();
        let worker_thread = std::thread::Builder::new()
            .name("agentinc-worker".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                rt.block_on(async move {
                    let worker =
                        build_worker(worker_client, worker_queue, worker_registry, options);
                    let mut worker = match worker {
                        Ok(w) => w,
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                            return;
                        }
                    };
                    let shutdown = worker.shutdown_handle();
                    let _ = ready_tx.send(Ok(Box::new(shutdown) as ShutdownFn));
                    if let Err(e) = worker.run().await {
                        tracing::error!("agentinc worker stopped: {e}");
                    }
                });
            })
            .map_err(|e| Error::Connection(e.to_string()))?;
        let shutdown_worker = ready_rx
            .await
            .map_err(|_| Error::Connection("worker thread died during startup".into()))?
            .map_err(Error::Connection)?;

        Ok(Self {
            client,
            task_queue,
            registry,
            shutdown_worker,
            worker_thread: Some(worker_thread),
            local,
        })
    }

    pub(crate) async fn start(
        &self,
        agent: &Agent,
        input: Message,
    ) -> Result<(RunId, RunHandle), Error> {
        let id = RunId(format!("{RUN_ID_PREFIX}{}", uuid::Uuid::new_v4()));
        let handle = self.start_with_id(&id, agent, input).await?;
        Ok((id, handle))
    }

    pub(crate) async fn start_with_id(
        &self,
        id: &RunId,
        agent: &Agent,
        input: Message,
    ) -> Result<RunHandle, Error> {
        self.registry.register(agent);
        match self
            .client
            .start_workflow(
                AgentRunWorkflow::run,
                RunInput {
                    agent: agent_spec(agent),
                    input,
                },
                WorkflowStartOptions::new(self.task_queue.clone(), id.0.clone())
                    .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
                    .build(),
            )
            .await
        {
            Ok(handle) => Ok(RunHandle {
                inner: Arc::new(handle),
            }),
            Err(WorkflowStartError::AlreadyStarted { .. }) => Ok(self.run_handle(id)),
            Err(error) => Err(Error::Other(error.into())),
        }
    }

    pub(crate) fn run_handle(&self, id: &RunId) -> RunHandle {
        RunHandle {
            inner: Arc::new(
                self.client
                    .get_workflow_handle::<RunWorkflowType>(id.0.clone()),
            ),
        }
    }

    pub(crate) async fn start_session(
        &self,
        agent: &Agent,
    ) -> Result<(SessionId, SessionHandle), Error> {
        self.registry.register(agent);
        let id = SessionId(format!("{SESSION_ID_PREFIX}{}", uuid::Uuid::new_v4()));
        let handle = self
            .client
            .start_workflow(
                SessionWorkflow::run,
                SessionInput {
                    agent: agent_spec(agent),
                },
                WorkflowStartOptions::new(self.task_queue.clone(), id.0.clone()).build(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))?;
        Ok((
            id,
            SessionHandle {
                inner: Arc::new(handle),
            },
        ))
    }

    /// Attach to an existing session. The agent must be registered here so its model and
    /// tools can be resolved when a turn runs on this worker.
    pub(crate) fn session_handle(&self, agent: &Agent, id: &SessionId) -> SessionHandle {
        self.registry.register(agent);
        SessionHandle {
            inner: Arc::new(
                self.client
                    .get_workflow_handle::<SessionWorkflowType>(id.0.clone()),
            ),
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), Error> {
        self.stop_worker().await;
        if let Some(env) = self.local.take() {
            env.shutdown()
                .await
                .map_err(|e| Error::Connection(e.to_string()))?;
        }
        Ok(())
    }

    async fn stop_worker(&mut self) {
        (self.shutdown_worker)();
        if let Some(thread) = self.worker_thread.take() {
            let _ = tokio::task::spawn_blocking(move || thread.join()).await;
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Ask the worker to stop; the thread exits on its own once polling winds down.
        (self.shutdown_worker)();
    }
}

fn build_worker(
    client: Client,
    task_queue: String,
    registry: Registry,
    options: EngineOptions,
) -> Result<Worker, String> {
    let runtime = Runtime::from_current_tokio(Default::default()).map_err(|e| e.to_string())?;
    let options = WorkerOptions::new(task_queue)
        .register_workflow::<AgentRunWorkflow>()
        .map_err(|e| e.to_string())?
        .register_workflow::<SessionWorkflow>()
        .map_err(|e| e.to_string())?
        .register_activities(AgentActivities {
            registry,
            check_idempotency: options.check_idempotency,
        })
        .build();
    Worker::new(&runtime, client, options).map_err(|e| e.to_string())
}

type RunWorkflowHandle = WorkflowHandle<Client, RunWorkflowType>;

/// Handle to one workflow execution. Cloneable, cheap.
#[derive(Clone)]
pub(crate) struct RunHandle {
    inner: Arc<RunWorkflowHandle>,
}

impl RunHandle {
    pub(crate) async fn events_after(&self, offset: usize) -> Result<(Vec<Event>, bool), Error> {
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
        self.inner
            .cancel(WorkflowCancelOptions::default())
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    pub(crate) async fn output(&self) -> Result<RunOutput, Error> {
        self.inner
            .get_result(WorkflowGetResultOptions::default())
            .await
            .map_err(|e| match e {
                WorkflowGetResultError::Cancelled { .. } => Error::Cancelled,
                other => Error::RunFailed(root_message(&other)),
            })
    }
}

type SessionWorkflowHandle = WorkflowHandle<Client, SessionWorkflowType>;

/// Handle to one session workflow. Cloneable, cheap.
#[derive(Clone)]
pub(crate) struct SessionHandle {
    inner: Arc<SessionWorkflowHandle>,
}

impl SessionHandle {
    pub(crate) async fn cancel(&self) -> Result<(), Error> {
        self.inner
            .cancel(WorkflowCancelOptions::default())
            .await
            .map_err(|e| Error::Other(e.into()))
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

    pub(crate) async fn events_after(&self, offset: usize) -> Result<Vec<Event>, Error> {
        self.inner
            .execute_update(
                SessionWorkflow::events_after,
                offset,
                WorkflowExecuteUpdateOptions::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
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
