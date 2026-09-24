//! Infrastructure for process-recovery tests, hidden behind agent vocabulary.
use super::*;
use temporalio_client::WorkflowFetchHistoryOptions;
use temporalio_sdk::workflow_replayer::{WorkflowReplayer, WorkflowReplayerOptions};

pub(crate) struct TestServer {
    env: WorkflowEnvironment<LocalServer>,
    pub config: RuntimeConfig,
}

impl TestServer {
    pub async fn start() -> Result<Self, Error> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| Error::Connection(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::Connection(e.to_string()))?
            .port();
        drop(listener);
        let env = WorkflowEnvironment::start_local(
            LocalWorkflowEnvironmentOptions::builder()
                .port(port)
                .build(),
        )
        .await
        .map_err(|e| Error::Connection(e.to_string()))?;
        Ok(Self {
            env,
            config: RuntimeConfig {
                endpoint: format!("http://127.0.0.1:{port}"),
                scope: "default".into(),
                worker_group: format!("test-{}", uuid::Uuid::new_v4()),
            },
        })
    }

    pub async fn replay(&self, id: &RunId) -> Result<(), Error> {
        let history = self
            .env
            .client()
            .get_workflow_handle::<RunWorkflowType>(id.as_str())
            .fetch_history(WorkflowFetchHistoryOptions::default());
        // Replay futures are !Send, just like the worker itself.
        let result = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                let replayer = WorkflowReplayer::new(
                    WorkflowReplayerOptions::new()
                        .register_workflow::<AgentRunWorkflow>()?
                        .register_workflow::<SessionWorkflow>()?
                        .build(),
                )?;
                replayer.replay_workflow(history).await?;
                anyhow::Ok(())
            })
        })
        .await
        .map_err(|_| Error::Connection("replay thread panicked".into()))?;
        result.map_err(Error::Other)
    }

    pub async fn fire_rule(&self, id: &str) -> Result<(), Error> {
        self.env
            .client()
            .get_schedule_handle(id)
            .trigger(
                temporalio_client::schedules::ScheduleOverlapPolicy::Skip,
                Default::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    pub async fn shutdown(self) -> Result<(), Error> {
        self.env
            .shutdown()
            .await
            .map_err(|e| Error::Connection(e.to_string()))
    }
}
