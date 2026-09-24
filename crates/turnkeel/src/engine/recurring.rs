use super::Engine;
use crate::{Error, Occurrence, OccurrenceRecord, RecurringAction, RecurringRule, RecurringState};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};
use temporalio_client::{
    WorkflowIdReusePolicy, WorkflowStartOptions,
    errors::WorkflowStartError,
    schedules::{CreateScheduleOptions, ScheduleAction, ScheduleOverlapPolicy, ScheduleSpec},
};
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    ActivityOptions, ApplicationFailure, WorkflowContext, WorkflowContextView, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};

pub(crate) struct RecurringActivities(pub Option<Arc<dyn RecurringAction>>);
#[activities]
impl RecurringActivities {
    #[activity(name = "turnkeel.occurrence")]
    pub(crate) async fn execute(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Occurrence,
    ) -> Result<(), ActivityError> {
        let action = self.0.as_ref().ok_or_else(|| {
            ActivityError::application(ApplicationFailure::new(
                "recurring action worker unavailable",
            ))
        })?;
        super::activities::cancellable(&ctx, action.execute(input))
            .await?
            .map_err(|e| ActivityError::application(ApplicationFailure::new(e.to_string())))
    }
}
#[workflow]
pub(crate) struct OccurrenceWorkflow {
    input: Value,
}
#[workflow_methods]
impl OccurrenceWorkflow {
    #[init]
    fn new(_: &WorkflowContextView, input: Value) -> Self {
        Self { input }
    }
    #[run(name = "turnkeel.occurrence")]
    pub(crate) async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<()> {
        ctx.execute_activity(
            RecurringActivities::execute,
            Occurrence {
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
    pub(crate) async fn apply_rule(&self, rule: RecurringRule) -> Result<(), Error> {
        if rule.every < Duration::from_secs(60) {
            return Err(Error::Connection(
                "recurrence must be at least one minute".into(),
            ));
        }
        let handle = self.client.get_schedule_handle(&rule.id);
        let action = ScheduleAction::start_workflow(
            OccurrenceWorkflow::run,
            rule.input,
            self.task_queue.clone(),
            format!("occurrence-{}", rule.id),
        );
        // Create paused so a crash cannot leave a rule firing with default catch-up policy.
        let create = self
            .client
            .create_schedule(
                &rule.id,
                CreateScheduleOptions::builder()
                    .action(action.clone())
                    .spec(ScheduleSpec::from_interval(rule.every))
                    .paused(true)
                    .overlap_policy(ScheduleOverlapPolicy::Skip)
                    .build(),
            )
            .await;
        if let Err(error) = create {
            // Describe distinguishes a retained definition from a failed create.
            handle
                .describe(Default::default())
                .await
                .map_err(|_| Error::Other(error.into()))?;
        }
        handle
            .update(
                move |update| {
                    update
                        .set_spec(ScheduleSpec::from_interval(rule.every))
                        .set_action(action)
                        .set_overlap_policy(ScheduleOverlapPolicy::Skip)
                        .set_catchup_window(Duration::from_secs(10))
                        .set_paused(rule.paused);
                },
                Default::default(),
            )
            .await
            .map_err(|e| Error::Other(e.into()))
    }
    pub(crate) async fn recurring_state(&self, id: &str) -> Result<RecurringState, Error> {
        let description = self
            .client
            .get_schedule_handle(id)
            .describe(Default::default())
            .await
            .map_err(|e| Error::Other(e.into()))?;
        Ok(RecurringState {
            paused: description.paused(),
            missed: description.missed_catchup_window(),
            overlap_skipped: description.overlap_skipped(),
            recent: description
                .recent_actions()
                .into_iter()
                .map(|a| OccurrenceRecord {
                    id: a.workflow_id,
                    scheduled_at: a
                        .schedule_time
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs() as i64),
                })
                .collect(),
        })
    }
    pub(crate) async fn run_occurrence(&self, occurrence: Occurrence) -> Result<(), Error> {
        match self
            .client
            .start_workflow(
                OccurrenceWorkflow::run,
                occurrence.input,
                WorkflowStartOptions::new(self.task_queue.clone(), occurrence.id)
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
