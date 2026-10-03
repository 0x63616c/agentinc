//! Everything that knows about Temporal lives here. Nothing in this module is public.

mod activities;
mod conversation;
mod handle;
mod legacy;
mod recurring;
mod session;
#[cfg(feature = "testing")]
mod test_server;
mod visibility;
mod workflow;

pub(crate) use handle::{RunHandle, SessionHandle};
#[cfg(feature = "testing")]
pub(crate) use test_server::TestServer;

#[allow(deprecated)]
use crate::runtime::RunPage;
use crate::{
    Agent, AgentSource, Error, Message, RunId, RuntimeConfig, SessionId,
    runtime::{RunStatus, WorkPage},
};
use activities::{AgentActivities, Registry};
use conversation::agent_spec;
use session::{LegacySessionWorkflow, SessionInput, SessionWorkflow, SessionWorkflowType};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
use temporalio_client::{
    Client, ClientOptions, ConnectionOptions, WorkflowIdReusePolicy, WorkflowStartOptions,
    errors::WorkflowStartError,
};
#[cfg(feature = "testing")]
use temporalio_sdk::testing::{LocalServer, LocalWorkflowEnvironmentOptions, WorkflowEnvironment};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};
use workflow::{AgentRunWorkflow, LegacyRunWorkflow, RunInput, RunWorkflowType};

/// The local dev server an engine may own. Without `testing` nothing can construct one, so
/// the field is always `None` and the shutdown path is unreachable.
#[cfg(feature = "testing")]
type LocalEnvironment = WorkflowEnvironment<LocalServer>;
#[cfg(not(feature = "testing"))]
enum LocalEnvironment {}
#[cfg(not(feature = "testing"))]
impl LocalEnvironment {
    async fn shutdown(self) -> Result<(), String> {
        match self {}
    }
}

pub(crate) use session::SESSION_ID_PREFIX;
pub(crate) use workflow::RUN_ID_PREFIX;

type ShutdownFn = Box<dyn Fn() + Send + Sync>;

/// Engine behaviour switches. Internal; surfaced through `Runtime::local()` / `Runtime::test()`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EngineOptions {
    pub check_idempotency: bool,
}

/// Set once a `testing::Server` starts in this process: every runtime configured
/// afterwards runs with the test-only checks on, as `Runtime::test()` does.
static TESTING: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "testing")]
pub(crate) fn enable_testing() {
    TESTING.store(true, Ordering::Relaxed);
}

/// What a worker needs besides a connection. Absent: connect as an observer only.
#[derive(Default)]
struct WorkerSetup<'a> {
    options: EngineOptions,
    agents: &'a [Agent],
    source: Option<Arc<dyn AgentSource>>,
    action: Option<Arc<dyn crate::RecurringAction>>,
}

struct WorkerLink {
    shutdown: ShutdownFn,
    thread: Option<JoinHandle<()>>,
}

pub(crate) struct Engine {
    client: Client,
    task_queue: String,
    registry: Registry,
    worker: Option<WorkerLink>,
    local: Option<LocalEnvironment>,
}

impl Engine {
    #[cfg(feature = "testing")]
    pub(crate) async fn local(options: EngineOptions) -> Result<Self, Error> {
        // SDK default: download the pinned Temporal CLI once, cache it in the OS temp dir.
        let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::default())
            .await
            .map_err(|e| Error::Connection(e.to_string()))?;
        let client = env.client().clone();
        Self::build(
            client,
            Some(env),
            format!("turnkeel-{}", uuid::Uuid::new_v4()),
            Some(WorkerSetup {
                options,
                ..Default::default()
            }),
        )
        .await
    }

    pub(crate) async fn connect(url: &str) -> Result<Self, Error> {
        Self::configured(
            RuntimeConfig {
                endpoint: url.into(),
                scope: "default".into(),
                worker_group: format!("turnkeel-{}", uuid::Uuid::new_v4()),
            },
            &[],
            None,
        )
        .await
    }

    /// Connect a worker with a stable identity.
    pub(crate) async fn configured(
        config: RuntimeConfig,
        agents: &[Agent],
        source: Option<Arc<dyn AgentSource>>,
    ) -> Result<Self, Error> {
        let client = Self::client(&config).await?;
        Self::build(
            client,
            None,
            config.worker_group,
            Some(WorkerSetup {
                options: Self::configured_options(),
                agents,
                source,
                action: None,
            }),
        )
        .await
    }

    pub(crate) async fn configured_recurring(
        config: RuntimeConfig,
        action: Arc<dyn crate::RecurringAction>,
    ) -> Result<Self, Error> {
        let client = Self::client(&config).await?;
        Self::build(
            client,
            None,
            config.worker_group,
            Some(WorkerSetup {
                options: Self::configured_options(),
                action: Some(action),
                ..Default::default()
            }),
        )
        .await
    }

    /// Connect without accepting work: for reading history and attaching to runs.
    pub(crate) async fn observer(config: RuntimeConfig) -> Result<Self, Error> {
        let client = Self::client(&config).await?;
        Self::build(client, None, config.worker_group, None).await
    }

    fn configured_options() -> EngineOptions {
        EngineOptions {
            check_idempotency: TESTING.load(Ordering::Relaxed),
        }
    }

    async fn client(config: &RuntimeConfig) -> Result<Client, Error> {
        if config.scope.trim().is_empty() || config.worker_group.trim().is_empty() {
            return Err(Error::Connection(
                "runtime scope and worker group must be nonempty".into(),
            ));
        }
        let target: temporalio_client::Url = config
            .endpoint
            .parse()
            .map_err(|e| Error::Connection(format!("invalid runtime endpoint: {e}")))?;
        Client::connect(
            ConnectionOptions::new(target).identity("turnkeel").build(),
            ClientOptions::new(config.scope.clone()).build(),
        )
        .await
        .map_err(|e| Error::Connection(e.to_string()))
    }

    async fn build(
        client: Client,
        local: Option<LocalEnvironment>,
        task_queue: String,
        worker: Option<WorkerSetup<'_>>,
    ) -> Result<Self, Error> {
        let Some(setup) = worker else {
            return Ok(Self {
                client,
                task_queue,
                registry: Registry::default(),
                worker: None,
                local,
            });
        };
        let registry = Registry::new(setup.agents, setup.source);
        let options = setup.options;
        let action = setup.action;

        // The worker future is !Send, so it gets its own thread and single-threaded runtime.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let worker_client = client.clone();
        let worker_queue = task_queue.clone();
        let worker_registry = registry.clone();
        let thread = std::thread::Builder::new()
            .name("turnkeel-worker".into())
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
                    let worker = build_worker(
                        worker_client,
                        worker_queue,
                        worker_registry,
                        options,
                        action,
                    );
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
                        tracing::error!("turnkeel worker stopped: {e}");
                    }
                });
            })
            .map_err(|e| Error::Connection(e.to_string()))?;
        let shutdown = ready_rx
            .await
            .map_err(|_| Error::Connection("worker thread died during startup".into()))?
            .map_err(Error::Connection)?;

        Ok(Self {
            client,
            task_queue,
            registry,
            worker: Some(WorkerLink {
                shutdown,
                thread: Some(thread),
            }),
            local,
        })
    }

    pub(crate) async fn work_history(
        &self,
        status: Option<RunStatus>,
        page: Option<&str>,
    ) -> Result<WorkPage, Error> {
        visibility::work_history(&self.client, status, page).await
    }

    #[allow(deprecated)]
    pub(crate) async fn run_history(
        &self,
        status: Option<&str>,
        page: Option<&str>,
    ) -> Result<RunPage, Error> {
        visibility::run_history(&self.client, status, page).await
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
            Ok(handle) => Ok(RunHandle::new(handle)),
            Err(WorkflowStartError::AlreadyStarted { .. }) => Ok(self.run_handle(id)),
            Err(error) => Err(Error::Other(error.into())),
        }
    }

    pub(crate) fn run_handle(&self, id: &RunId) -> RunHandle {
        RunHandle::new(
            self.client
                .get_workflow_handle::<RunWorkflowType>(id.0.clone()),
        )
    }

    pub(crate) async fn start_session(
        &self,
        agent: &Agent,
    ) -> Result<(SessionId, SessionHandle), Error> {
        let id = SessionId(format!("{SESSION_ID_PREFIX}{}", uuid::Uuid::new_v4()));
        let handle = self.open_session(&id, agent, Vec::new()).await?;
        Ok((id, handle))
    }

    pub(crate) async fn open_session(
        &self,
        id: &SessionId,
        agent: &Agent,
        history: Vec<Message>,
    ) -> Result<SessionHandle, Error> {
        self.registry.register(agent);
        match self
            .client
            .start_workflow(
                SessionWorkflow::run,
                SessionInput {
                    agent: agent_spec(agent),
                    history,
                },
                WorkflowStartOptions::new(self.task_queue.clone(), id.0.clone())
                    .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
                    .build(),
            )
            .await
        {
            Ok(handle) => Ok(SessionHandle::new(handle)),
            Err(WorkflowStartError::AlreadyStarted { .. }) => Ok(self.session_handle(agent, id)),
            Err(error) => Err(Error::Other(error.into())),
        }
    }

    /// Attach to an existing session. The agent is registered here so its model and
    /// tools can be resolved when a turn runs on this worker.
    pub(crate) fn session_handle(&self, agent: &Agent, id: &SessionId) -> SessionHandle {
        self.registry.register(agent);
        SessionHandle::new(
            self.client
                .get_workflow_handle::<SessionWorkflowType>(id.0.clone()),
        )
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
        if let Some(worker) = &mut self.worker {
            (worker.shutdown)();
            if let Some(thread) = worker.thread.take() {
                let _ = tokio::task::spawn_blocking(move || thread.join()).await;
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Ask the worker to stop; the thread exits on its own once polling winds down.
        if let Some(worker) = &self.worker {
            (worker.shutdown)();
        }
    }
}

fn build_worker(
    client: Client,
    task_queue: String,
    registry: Registry,
    options: EngineOptions,
    action: Option<Arc<dyn crate::RecurringAction>>,
) -> Result<Worker, String> {
    // Workflows schedule activities by name; the registered names must be those names.
    debug_assert_eq!(AgentActivities::model_step.name(), activities::MODEL_STEP);
    debug_assert_eq!(AgentActivities::call_tool.name(), activities::CALL_TOOL);
    debug_assert_eq!(
        AgentActivities::legacy_model_step.name(),
        legacy::MODEL_STEP
    );
    debug_assert_eq!(AgentActivities::legacy_call_tool.name(), legacy::CALL_TOOL);
    let runtime = Runtime::from_current_tokio(Default::default()).map_err(|e| e.to_string())?;
    let options = WorkerOptions::new(task_queue)
        .register_workflow::<AgentRunWorkflow>()
        .map_err(|e| e.to_string())?
        .register_workflow::<LegacyRunWorkflow>()
        .map_err(|e| e.to_string())?
        .register_workflow::<SessionWorkflow>()
        .map_err(|e| e.to_string())?
        .register_workflow::<LegacySessionWorkflow>()
        .map_err(|e| e.to_string())?
        .register_workflow::<recurring::OccurrenceWorkflow>()
        .map_err(|e| e.to_string())?
        .register_activities(recurring::RecurringActivities(action))
        .register_activities(AgentActivities {
            registry,
            check_idempotency: options.check_idempotency,
        })
        .build();
    Worker::new(&runtime, client, options).map_err(|e| e.to_string())
}
