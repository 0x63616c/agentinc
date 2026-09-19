//! Everything that knows about Temporal lives here. Nothing in this module is public.

mod activities;
mod workflow;

use crate::{Agent, Error, Message, RunId};
use activities::{AgentActivities, Registry};
use std::sync::Arc;
use std::thread::JoinHandle;
use temporalio_client::{
    Client, ClientOptions, ConnectionOptions, WorkflowGetResultOptions, WorkflowHandle,
    WorkflowStartOptions, errors::WorkflowGetResultError,
};
use temporalio_sdk::{
    Runtime, Worker, WorkerOptions,
    testing::{LocalServer, LocalWorkflowEnvironmentOptions, WorkflowEnvironment},
};

type ShutdownFn = Box<dyn Fn() + Send + Sync>;

/// Engine behaviour switches. Internal; surfaced through `Agentic::local()` / `Agentic::test()`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EngineOptions {
    pub check_idempotency: bool,
}
use workflow::{AgentRunWorkflow, RunInput, RunOutput, RunWorkflowType, agent_spec};

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
        Self::with_client(client, Some(env), options).await
    }

    pub(crate) async fn connect(url: &str) -> Result<Self, Error> {
        let target: temporalio_client::Url = url
            .parse()
            .map_err(|e| Error::Connection(format!("bad url {url}: {e}")))?;
        let client = Client::connect(
            ConnectionOptions::new(target).identity("agentic").build(),
            ClientOptions::new("default").build(),
        )
        .await
        .map_err(|e| Error::Connection(e.to_string()))?;
        Self::with_client(client, None, EngineOptions::default()).await
    }

    async fn with_client(
        client: Client,
        local: Option<WorkflowEnvironment<LocalServer>>,
        options: EngineOptions,
    ) -> Result<Self, Error> {
        let task_queue = format!("agentic-{}", uuid::Uuid::new_v4());
        let registry = Registry::default();

        // The worker future is !Send, so it gets its own thread and single-threaded runtime.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let worker_client = client.clone();
        let worker_queue = task_queue.clone();
        let worker_registry = registry.clone();
        let worker_thread = std::thread::Builder::new()
            .name("agentic-worker".into())
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
                        tracing::error!("agentic worker stopped: {e}");
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
        self.registry.register(agent);
        let run_id = RunId(format!("{RUN_ID_PREFIX}{}", uuid::Uuid::new_v4()));
        let handle = self
            .client
            .start_workflow(
                AgentRunWorkflow::run,
                RunInput {
                    agent: agent_spec(agent),
                    input,
                    max_turns: agent.max_turns,
                },
                WorkflowStartOptions::new(self.task_queue.clone(), run_id.0.clone()).build(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))?;
        Ok((
            run_id,
            RunHandle {
                inner: Arc::new(handle),
            },
        ))
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

/// Walks the error chain to the innermost message, which is the one the user wrote.
fn root_message(err: &dyn std::error::Error) -> String {
    let mut cur = err;
    while let Some(next) = cur.source() {
        cur = next;
    }
    cur.to_string()
}
