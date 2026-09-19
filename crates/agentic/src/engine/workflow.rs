//! The agent loop, as a Temporal workflow. Deterministic: no I/O here, only activity calls.

use super::activities::{AgentActivities, StepInput, ToolCallInput};
use crate::{Agent, Content, Message, Role, StopReason, ToolSpec};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use temporalio_common::RetryPolicy;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ActivityOptions, ApplicationFailure, WorkflowContext, WorkflowResult};

/// The generated marker type for the `run` method, nameable from the rest of the engine.
pub(crate) type RunWorkflowType = agent_run_workflow::Run;

pub(crate) const RUN_ID_PREFIX: &str = "agentic-run-";

/// The serializable part of an [`Agent`]. Model and tool implementations stay in the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentSpec {
    pub name: String,
    pub instructions: String,
    pub tools: Vec<ToolSpec>,
}

pub(crate) fn agent_spec(agent: &Agent) -> AgentSpec {
    let mut tools: Vec<ToolSpec> = agent
        .tools
        .iter()
        .map(|t| ToolSpec {
            name: t.name().to_owned(),
            description: t.description().to_owned(),
            input_schema: t.schema(),
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    AgentSpec {
        name: agent.name.clone(),
        instructions: agent.instructions.clone(),
        tools,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunInput {
    pub agent: AgentSpec,
    pub input: Message,
    pub max_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunOutput {
    pub text: String,
    pub messages: Vec<Message>,
}

#[workflow]
#[derive(Default)]
pub(crate) struct AgentRunWorkflow;

#[workflow_methods]
impl AgentRunWorkflow {
    #[run(name = "agentic.run")]
    pub(crate) async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: RunInput,
    ) -> WorkflowResult<RunOutput> {
        let RunInput {
            agent,
            input,
            max_turns,
        } = input;
        let mut messages = vec![input];

        for turn in 0..max_turns {
            let response = ctx
                .execute_activity(
                    AgentActivities::model_step,
                    StepInput {
                        agent: agent.name.clone(),
                        messages: messages.clone(),
                    },
                    ActivityOptions::with_start_to_close_timeout(Duration::from_secs(300))
                        .retry_policy(RetryPolicy::builder().maximum_attempts(5).build())
                        .summary(format!("turn {}", turn + 1))
                        .build(),
                )
                .await?;

            let assistant = Message::assistant(response.content);
            let calls: Vec<(String, String, serde_json::Value)> = assistant
                .tool_uses()
                .map(|(id, name, args)| (id.to_owned(), name.to_owned(), args.clone()))
                .collect();
            messages.push(assistant);

            if response.stop_reason == StopReason::EndTurn || calls.is_empty() {
                let text = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::Assistant)
                    .map(Message::text)
                    .unwrap_or_default();
                return Ok(RunOutput { text, messages });
            }

            let mut results = Vec::with_capacity(calls.len());
            for (id, name, args) in calls {
                let result = ctx
                    .execute_activity(
                        AgentActivities::call_tool,
                        ToolCallInput {
                            agent: agent.name.clone(),
                            name: name.clone(),
                            args,
                        },
                        ActivityOptions::with_start_to_close_timeout(Duration::from_secs(120))
                            .retry_policy(RetryPolicy::builder().maximum_attempts(3).build())
                            .summary(name)
                            .build(),
                    )
                    .await?;
                results.push(Content::ToolResult {
                    tool_use_id: id,
                    content: result.content,
                    is_error: result.is_error,
                });
            }
            messages.push(Message {
                role: Role::User,
                content: results,
            });
        }

        Err(ApplicationFailure::non_retryable(format!("run exceeded {max_turns} turns")).into())
    }
}
