//! One run: one message in, one reply out, then the workflow ends.

use super::conversation::{AgentSpec, Conversation, HasConversation, turn};
use crate::{Event, Message};
use serde::{Deserialize, Serialize};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowContextView, WorkflowResult};

/// The generated marker type for the `run` method, nameable from the rest of the engine.
pub(crate) type RunWorkflowType = agent_run_workflow::Run;

pub(crate) const RUN_ID_PREFIX: &str = "turnkeel-run-";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunInput {
    pub agent: AgentSpec,
    pub input: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunOutput {
    pub text: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub events: Vec<Event>,
}

/// The run workflow, once under its current durable names and once under the names
/// retained histories carry. Both share every line of logic.
macro_rules! run_workflow {
    ($workflow:ident, $name:expr, $model_step:expr, $call_tool:expr) => {
        #[workflow]
        pub(crate) struct $workflow {
            conversation: Conversation,
        }

        impl HasConversation for $workflow {
            const MODEL_STEP: &'static str = $model_step;
            const CALL_TOOL: &'static str = $call_tool;
            fn conversation(&mut self) -> &mut Conversation {
                &mut self.conversation
            }
        }

        #[workflow_methods]
        impl $workflow {
            #[init]
            fn new(_ctx: &WorkflowContextView, input: RunInput) -> Self {
                let mut conversation = Conversation::new(input.agent);
                conversation.pending.push(input.input);
                Self { conversation }
            }

            #[update]
            pub(crate) async fn events_after(
                ctx: &mut WorkflowContext<Self>,
                offset: usize,
            ) -> Vec<Event> {
                let _ = ctx
                    .wait_condition(|w| w.conversation.log.len() > offset)
                    .await;
                ctx.state(|w| {
                    w.conversation
                        .log
                        .get(offset..)
                        .unwrap_or_default()
                        .to_vec()
                })
            }

            #[run(name = $name)]
            pub(crate) async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<RunOutput> {
                let text = turn(ctx).await?;
                let messages = ctx.state(|w| w.conversation.messages.clone());
                let events = ctx.state(|w| w.conversation.log.clone());
                Ok(RunOutput {
                    text,
                    messages,
                    events,
                })
            }
        }
    };
}

run_workflow!(
    AgentRunWorkflow,
    "turnkeel.run",
    crate::engine::activities::MODEL_STEP,
    crate::engine::activities::CALL_TOOL
);
run_workflow!(
    LegacyRunWorkflow,
    crate::engine::legacy::RUN,
    crate::engine::legacy::MODEL_STEP,
    crate::engine::legacy::CALL_TOOL
);
