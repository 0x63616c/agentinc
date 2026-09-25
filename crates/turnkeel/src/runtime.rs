use crate::{
    Agent, Error, Message, Run, RunId, Session, SessionId,
    engine::{Engine, EngineOptions},
};

/// Stable deployment identity. Reuse the same values and register the same versioned
/// agent definitions when replacing a process. A group shares work between its workers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeConfig {
    pub endpoint: String,
    pub scope: String,
    pub worker_group: String,
}

/// One run in the runtime's retained history.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub run_id: String,
    pub kind: String,
    pub status: String,
    pub started_at: i64,
    pub closed_at: Option<i64>,
}

/// A page of runs ordered by start time, newest first.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunPage {
    pub runs: Vec<RunRecord>,
    pub next_page: Option<String>,
}

/// The runtime. Owns the connection and runs agents.
pub struct Runtime {
    engine: Engine,
}

impl Runtime {
    /// Read one page of retained runs without starting a worker.
    pub async fn run_history(
        config: &RuntimeConfig,
        status: Option<&str>,
        page: Option<&str>,
    ) -> Result<RunPage, Error> {
        crate::engine::list_workflows(config, status, page).await
    }
    /// Start an embedded local runtime. Good for development and examples.
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
    /// Give changed definitions new names (for example `evee-v2`), and retain old
    /// definitions while their runs or sessions are still open.
    pub async fn configured(config: RuntimeConfig, agents: &[Agent]) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::configured(config, agents).await?,
        })
    }

    /// Connect a dedicated worker for recurring actions. Use a distinct worker
    /// group from ordinary agent workers and retain the same handler on restart.
    pub async fn recurring(
        config: RuntimeConfig,
        action: std::sync::Arc<dyn crate::RecurringAction>,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::configured_recurring(config, &[], Some(action)).await?,
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

    /// Attach to a retained run. Its agent must be installed on a worker.
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
