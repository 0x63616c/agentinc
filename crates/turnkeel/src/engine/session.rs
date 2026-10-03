//! One session: a conversation that stays open and takes a turn whenever a message arrives.

use super::conversation::{AgentSpec, Conversation, HasConversation, turn};
use crate::{Event, Message};
use serde::{Deserialize, Serialize};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{SyncWorkflowContext, WorkflowContext, WorkflowContextView, WorkflowResult};

/// The generated marker type for the `run` method, nameable from the rest of the engine.
pub(crate) type SessionWorkflowType = session_workflow::Run;

pub(crate) const SESSION_ID_PREFIX: &str = "turnkeel-session-";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionInput {
    pub agent: AgentSpec,
    #[serde(default)]
    pub history: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Delivery {
    pub id: String,
    pub message: Message,
}

/// The session workflow, once under its current durable names and once under the names
/// retained histories carry. Both share every line of logic.
macro_rules! session_workflow {
    ($workflow:ident, $name:expr, $model_step:expr, $call_tool:expr) => {
        #[workflow]
        pub(crate) struct $workflow {
            conversation: Conversation,
            delivered: std::collections::BTreeSet<String>,
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
            fn new(_ctx: &WorkflowContextView, input: SessionInput) -> Self {
                let mut conversation = Conversation::new(input.agent);
                conversation.messages = input.history;
                Self {
                    conversation,
                    delivered: Default::default(),
                }
            }

            #[run(name = $name)]
            pub(crate) async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<()> {
                loop {
                    ctx.wait_condition(|w| !w.conversation.pending.is_empty())
                        .await?;
                    turn(ctx).await?;
                }
            }

            #[signal]
            pub(crate) fn send(&mut self, _ctx: &mut SyncWorkflowContext<Self>, message: Message) {
                self.conversation.pending.push(message);
            }

            #[signal]
            pub(crate) fn send_once(
                &mut self,
                _ctx: &mut SyncWorkflowContext<Self>,
                delivery: Delivery,
            ) {
                if self.delivered.insert(delivery.id) {
                    self.conversation.pending.push(delivery.message);
                }
            }

            #[update]
            pub(crate) fn clear_pending(
                &mut self,
                _ctx: &mut SyncWorkflowContext<Self>,
                _input: (),
            ) -> Vec<Message> {
                std::mem::take(&mut self.conversation.pending)
            }

            /// Long poll: resolves with every event after `offset` as soon as there is at least one.
            #[update]
            pub(crate) async fn events_after(
                ctx: &mut WorkflowContext<Self>,
                offset: usize,
            ) -> Vec<Event> {
                // Only fails if the workflow is cancelled, and then there is nothing more to deliver.
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
        }
    };
}

session_workflow!(
    SessionWorkflow,
    "turnkeel.session",
    crate::engine::activities::MODEL_STEP,
    crate::engine::activities::CALL_TOOL
);
session_workflow!(
    LegacySessionWorkflow,
    crate::engine::legacy::SESSION,
    crate::engine::legacy::MODEL_STEP,
    crate::engine::legacy::CALL_TOOL
);
