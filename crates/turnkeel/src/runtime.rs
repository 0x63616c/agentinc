#[cfg(feature = "testing")]
use crate::engine::EngineOptions;
use crate::{Agent, AgentSource, Error, Message, Run, RunId, Session, SessionId, engine::Engine};
use std::sync::Arc;

/// Stable deployment identity. Reuse the same values and register the same versioned
/// agent definitions when replacing a process. A group shares work between its workers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeConfig {
    /// The address of the deployed runtime to connect to, for example `http://localhost:7233`.
    pub endpoint: String,
    /// A name that keeps this deployment's work separate from other deployments on the same endpoint.
    pub scope: String,
    /// The group of workers that share work with each other.
    pub worker_group: String,
}

/// What a piece of retained work is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// One message in, one reply out.
    Run,
    /// A conversation that stays open across many messages.
    Session,
    /// One invocation of a recurring action.
    Occurrence,
}

/// Where a piece of retained work stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The work is still in progress.
    Running,
    /// The work finished successfully.
    Completed,
    /// The work ended with an error.
    Failed,
    /// Stopped on request, whether by its caller or by an operator.
    Cancelled,
}

/// One run, session or occurrence in the runtime's retained history.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkRecord {
    /// The run, session or occurrence ID.
    pub id: String,
    /// What kind of work this is.
    pub kind: RunKind,
    /// Where the work stands.
    pub status: RunStatus,
    /// Milliseconds since the Unix epoch.
    pub started_at: i64,
    /// Milliseconds since the Unix epoch when the work ended, or `None` while it is open.
    pub closed_at: Option<i64>,
}

/// A page of work, open work first, then newest first.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkPage {
    /// The records on this page.
    pub work: Vec<WorkRecord>,
    /// Pass back to [`Runtime::work_history`] for the next page.
    pub next_page: Option<String>,
}

/// One run in the runtime's retained history.
#[deprecated(note = "use `Runtime::work_history` and `WorkRecord`")]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunRecord {
    /// The run's ID.
    pub id: String,
    /// Identifies this particular attempt of the run; it differs between attempts.
    pub run_id: String,
    /// The kind of work, as a name.
    pub kind: String,
    /// Where the run stands, as a name.
    pub status: String,
    /// Milliseconds since the Unix epoch when the run started.
    pub started_at: i64,
    /// Milliseconds since the Unix epoch when the run ended, or `None` while it is open.
    pub closed_at: Option<i64>,
}

/// A page of runs.
#[deprecated(note = "use `Runtime::work_history` and `WorkPage`")]
#[allow(deprecated)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunPage {
    /// The runs on this page.
    pub runs: Vec<RunRecord>,
    /// Pass back to [`Runtime::run_history`] for the next page.
    pub next_page: Option<String>,
}

/// The runtime. Owns the connection and runs agents.
pub struct Runtime {
    engine: Engine,
}

impl Runtime {
    /// Read one page of retained runs without starting a worker.
    #[deprecated(note = "use `Runtime::observer(config)` and `work_history`")]
    #[allow(deprecated)]
    pub async fn run_history(
        config: &RuntimeConfig,
        status: Option<&str>,
        page: Option<&str>,
    ) -> Result<RunPage, Error> {
        Engine::observer(config.clone())
            .await?
            .run_history(status, page)
            .await
    }

    /// Start an embedded local runtime. Good for development and examples.
    #[cfg(feature = "testing")]
    pub async fn local() -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::local(EngineOptions::default()).await?,
        })
    }

    /// Connect to a deployed runtime, e.g. `http://localhost:7233`.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::connect(url).await?,
        })
    }

    /// Connect with a stable identity and install definitions before accepting work.
    /// Give changed definitions new names (for example `assistant-v2`), and retain old
    /// definitions while their runs or sessions are still open.
    pub async fn configured(config: RuntimeConfig, agents: &[Agent]) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::configured(config, agents, None).await?,
        })
    }

    /// Connect with a stable identity and look definitions up as runs need them.
    ///
    /// The source is consulted on this worker the first time a run or session names an
    /// agent it has not seen, so a replacement process needs no list of in-flight agents
    /// and new definition versions need no restart. Resolved definitions are cached.
    pub async fn configured_with(
        config: RuntimeConfig,
        source: impl AgentSource,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::configured(config, &[], Some(Arc::new(source))).await?,
        })
    }

    /// Connect without accepting work: read history, attach to runs and sessions, start
    /// work for other workers in the group. Workers in the group do the running.
    pub async fn observer(config: RuntimeConfig) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::observer(config).await?,
        })
    }

    /// One page of retained work, optionally only work in one status. Pass the previous
    /// page's `next_page` to continue; a token from elsewhere is [`Error::InvalidInput`].
    pub async fn work_history(
        &self,
        status: Option<RunStatus>,
        page: Option<&str>,
    ) -> Result<WorkPage, Error> {
        self.engine.work_history(status, page).await
    }

    /// Connect a dedicated worker for recurring actions. Use a distinct worker
    /// group from ordinary agent workers and retain the same handler on restart.
    pub async fn recurring(
        config: RuntimeConfig,
        action: Arc<dyn crate::RecurringAction>,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::configured_recurring(config, action).await?,
        })
    }

    /// Reconcile a recurring rule. Overlap is skipped; catch-up is limited to ten
    /// seconds. Acknowledgement means the durable definition has been applied.
    pub async fn apply_rule(&self, rule: crate::RecurringRule) -> Result<(), Error> {
        self.engine.apply_rule(rule).await
    }

    /// Read counters and recent occurrences, even when no action worker is online.
    pub async fn recurring_state(&self, id: &str) -> Result<crate::RecurringState, Error> {
        self.engine.recurring_state(id).await
    }

    /// Request an immediate occurrence once under a retained caller-owned ID.
    pub async fn run_occurrence(&self, occurrence: crate::Occurrence) -> Result<(), Error> {
        self.engine.run_occurrence(occurrence).await
    }

    /// An isolated runtime for tests. Like [`Runtime::local`], plus checks that would be too
    /// expensive in production: every idempotent tool is called twice and must agree.
    #[cfg(feature = "testing")]
    pub async fn test() -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::local(EngineOptions {
                check_idempotency: true,
            })
            .await?,
        })
    }

    /// Start a run and return immediately.
    pub async fn start(&self, agent: &Agent, input: impl Into<Message>) -> Result<Run, Error> {
        let (id, handle) = self.engine.start(agent, input.into()).await?;
        Ok(Run { id, handle })
    }

    /// Start once under a caller-owned ID, or attach if it already exists.
    /// The caller must persist the ID and bind it to one immutable input/agent.
    /// Deduplication lasts for the deployment's retained run history.
    pub async fn start_with_id(
        &self,
        id: RunId,
        agent: &Agent,
        input: impl Into<Message>,
    ) -> Result<Run, Error> {
        let handle = self.engine.start_with_id(&id, agent, input.into()).await?;
        Ok(Run { id, handle })
    }

    /// Attach to a retained run. Its agent must be installed on, or resolvable by, a worker.
    pub fn run_by_id(&self, id: RunId) -> Run {
        let handle = self.engine.run_handle(&id);
        Run { id, handle }
    }

    /// Start a run and wait for the final answer.
    pub async fn run(&self, agent: &Agent, input: impl Into<Message>) -> Result<String, Error> {
        self.start(agent, input).await?.result().await
    }

    /// Open a session: a conversation that stays open across many messages.
    pub async fn session(&self, agent: &Agent) -> Result<Session, Error> {
        let (id, handle) = self.engine.start_session(agent).await?;
        Ok(Session { id, handle })
    }

    /// Open once under a persisted ID, restoring prior dialogue when creating it.
    /// Existing sessions retain their own history. Keep each ID bound to one agent
    /// definition; use a fresh ID after a terminal failure or a model change.
    pub async fn open_session(
        &self,
        id: SessionId,
        agent: &Agent,
        history: Vec<Message>,
    ) -> Result<Session, Error> {
        let handle = self.engine.open_session(&id, agent, history).await?;
        Ok(Session { id, handle })
    }

    /// Attach to a session opened earlier, possibly by another process.
    pub fn session_by_id(&self, agent: &Agent, id: SessionId) -> Session {
        let handle = self.engine.session_handle(agent, &id);
        Session { id, handle }
    }

    /// Stop the runtime. Also happens on drop.
    pub async fn shutdown(self) -> Result<(), Error> {
        self.engine.shutdown().await
    }
}
