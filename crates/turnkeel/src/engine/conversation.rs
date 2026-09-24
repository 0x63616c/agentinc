//! The agent loop, shared by every workflow. Deterministic: no I/O here, only activity calls.

use super::activities::{AgentActivities, StepInput, ToolCallInput};
use crate::{Agent, Content, Event, Message, Role, StopReason, ToolSpec};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use temporalio_common::RetryPolicy;
use temporalio_sdk::{ActivityOptions, WorkflowContext, WorkflowResult};

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
            idempotent: t.idempotent(),
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    AgentSpec {
        name: agent.name.clone(),
        instructions: agent.instructions.clone(),
        tools,
    }
}

/// Workflow state for one conversation with one agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Conversation {
    pub agent: AgentSpec,
    /// Everything the model has seen.
    pub messages: Vec<Message>,
    /// Sent, but not yet shown to the model. Drained before every model call.
    pub pending: Vec<Message>,
    /// Everything that happened, in order. What `events()` streams.
    pub log: Vec<Event>,
    /// Model calls so far. Never resets; part of every idempotency key.
    steps: u32,
}

impl Conversation {
    pub(crate) fn new(agent: AgentSpec) -> Self {
        Self {
            agent,
            messages: Vec::new(),
            pending: Vec::new(),
            log: Vec::new(),
            steps: 0,
        }
    }

    /// Show a message to the model from now on, and record it.
    fn append(&mut self, message: Message) {
        self.messages.push(message.clone());
        self.log.push(Event::Message(message));
    }
}

/// A workflow whose state holds a [`Conversation`].
pub(crate) trait HasConversation {
    fn conversation(&mut self) -> &mut Conversation;
}

/// Run one turn: show the model everything pending, run the tools it asks for, repeat until
/// it stops. Returns the final reply text.
///
/// State is read and written through `ctx` at each step rather than held across awaits, so
/// handlers can push into `pending` while a turn is in flight and be seen at the next step.
pub(crate) async fn turn<W: HasConversation>(ctx: &WorkflowContext<W>) -> WorkflowResult<String> {
    loop {
        let (agent, messages, step) = ctx.state_mut(|w| {
            let c = w.conversation();
            for m in std::mem::take(&mut c.pending) {
                c.append(m);
            }
            let step = c.steps;
            c.steps += 1;
            (c.agent.clone(), c.messages.clone(), step)
        });

        let response = ctx
            .execute_activity(
                AgentActivities::model_step,
                StepInput {
                    agent: agent.name.clone(),
                    messages,
                },
                ActivityOptions::with_start_to_close_timeout(Duration::from_secs(300))
                    .heartbeat_timeout(Duration::from_secs(10))
                    .summary(format!("step {}", step + 1))
                    .build(),
            )
            .await?;

        let assistant = Message::assistant(response.content);
        let calls: Vec<(String, String, serde_json::Value)> = assistant
            .tool_uses()
            .map(|(id, name, args)| (id.to_owned(), name.to_owned(), args.clone()))
            .collect();
        let text = assistant.text();
        ctx.state_mut(|w| w.conversation().append(assistant));

        if response.stop_reason == StopReason::EndTurn || calls.is_empty() {
            ctx.state_mut(|w| w.conversation().log.push(Event::TurnEnded));
            return Ok(text);
        }

        let mut results = Vec::with_capacity(calls.len());
        for (index, (id, name, args)) in calls.into_iter().enumerate() {
            // Deterministic and readable: derived from loop counters, not the model's ids.
            let idempotency_key = format!("{}/t{step}/c{index}", ctx.workflow_id());
            // A non-idempotent tool must never run twice, so it gets exactly one attempt.
            let idempotent = agent
                .tools
                .iter()
                .find(|t| t.name == name)
                .is_none_or(|t| t.idempotent);
            let base = ActivityOptions::with_start_to_close_timeout(Duration::from_secs(120))
                .heartbeat_timeout(Duration::from_secs(10))
                .summary(name.clone());
            let options = if idempotent {
                base.build()
            } else {
                base.retry_policy(RetryPolicy::builder().maximum_attempts(1).build())
                    .build()
            };
            let result = ctx
                .execute_activity(
                    AgentActivities::call_tool,
                    ToolCallInput {
                        agent: agent.name.clone(),
                        idempotency_key,
                        name,
                        args,
                    },
                    options,
                )
                .await?;
            results.push(Content::ToolResult {
                tool_use_id: id,
                content: result.content,
                is_error: result.is_error,
            });
        }
        ctx.state_mut(|w| {
            w.conversation().append(Message {
                role: Role::User,
                content: results,
            })
        });
    }
}
