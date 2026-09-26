use super::Engine;
use crate::{Error, Task, TaskError, TaskHandler};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use temporalio_client::{WorkflowIdReusePolicy, WorkflowStartOptions, errors::WorkflowStartError};
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    ActivityOptions, ApplicationFailure, WorkflowContext, WorkflowContextView, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};

pub(crate) struct TaskActivities(pub Option<Arc<dyn TaskHandler>>);
#[activities]
impl TaskActivities {
    #[activity(name = "turnkeel.task")]
    pub(crate) async fn run(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Task,
    ) -> Result<(), ActivityError> {
        let handler = self.0.as_ref().ok_or_else(|| {
            ActivityError::application(ApplicationFailure::new("task worker unavailable"))
        })?;
        match super::activities::cancellable(&ctx, handler.run(input)).await? {
            Ok(()) => Ok(()),
            Err(TaskError::Retry(message)) => {
                Err(ActivityError::application(ApplicationFailure::new(message)))
            }
            Err(TaskError::Permanent(message)) => Err(ActivityError::application(
                ApplicationFailure::non_retryable(message),
            )),
        }
    }
}
#[workflow]
pub(crate) struct TaskWorkflow {
    input: Value,
}
#[workflow_methods]
impl TaskWorkflow {
    #[init]
    fn new(_: &WorkflowContextView, input: Value) -> Self {
        Self { input }
    }
    #[run(name = "turnkeel.task")]
    pub(crate) async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<()> {
        ctx.execute_activity(
            TaskActivities::run,
            Task {
                id: ctx.workflow_id().to_string(),
                input: ctx.state(|s| s.input.clone()),
            },
            ActivityOptions::with_start_to_close_timeout(Duration::from_secs(86400))
                .heartbeat_timeout(Duration::from_secs(10))
                .build(),
        )
        .await?;
        Ok(())
    }
}
impl Engine {
    pub(crate) async fn start_task(&self, task: Task) -> Result<(), Error> {
        match self
            .client
            .start_workflow(
                TaskWorkflow::run,
                task.input,
                WorkflowStartOptions::new(self.task_queue.clone(), task.id)
                    .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
                    .build(),
            )
            .await
        {
            Ok(_) | Err(WorkflowStartError::AlreadyStarted { .. }) => Ok(()),
            Err(e) => Err(Error::Other(e.into())),
        }
    }
}
