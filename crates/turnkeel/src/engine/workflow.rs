//! One run: one message in, one reply out, then the workflow ends.

use super::conversation::{AgentSpec, Conversation, HasConversation, turn};
use crate::Message;
use serde::{Deserialize, Serialize};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowContextView, WorkflowResult};

/// The generated marker type for the `run` method, nameable from the rest of the engine.
pub(crate) type RunWorkflowType = agent_run_workflow::Run;

pub(crate) const RUN_ID_PREFIX: &str = "agentinc-run-";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunInput {
    pub agent: AgentSpec,
    pub input: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunOutput {
    pub text: String,
    pub messages: Vec<Message>,
}

#[workflow]
pub(crate) struct AgentRunWorkflow {
    conversation: Conversation,
}

impl HasConversation for AgentRunWorkflow {
    fn conversation(&mut self) -> &mut Conversation {
        &mut self.conversation
    }
}

#[workflow_methods]
impl AgentRunWorkflow {
    #[init]
    fn new(_ctx: &WorkflowContextView, input: RunInput) -> Self {
        let mut conversation = Conversation::new(input.agent);
        conversation.pending.push(input.input);
        Self { conversation }
    }

    #[run(name = "agentinc.run")]
    pub(crate) async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<RunOutput> {
        let text = turn(ctx).await?;
        let messages = ctx.state(|w| w.conversation.messages.clone());
        Ok(RunOutput { text, messages })
    }
}
