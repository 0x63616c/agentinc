use crate::{
    Agent, Error, Message, Run,
    engine::{Engine, EngineOptions},
};

/// The runtime. Owns the connection and runs agents.
pub struct Agentic {
    engine: Engine,
}

impl Agentic {
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

    /// An isolated runtime for tests. Like [`Agentic::local`], plus checks that would be too
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

    /// Start a run and wait for the final answer.
    pub async fn run(&self, agent: &Agent, input: impl Into<Message>) -> Result<String, Error> {
        self.start(agent, input).await?.result().await
    }

    /// Stop the runtime. Also happens on drop.
    pub async fn shutdown(self) -> Result<(), Error> {
        self.engine.shutdown().await
    }
}
