//! Side effects: model calls and tool calls. The only place user code runs.

use crate::{Agent, Message, ModelRequest, ModelResponse, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use temporalio_macros::activities;
use temporalio_sdk::{
    ApplicationFailure,
    activities::{ActivityContext, ActivityError},
};

/// Agents currently known to this worker, by name.
#[derive(Clone, Default)]
pub(crate) struct Registry {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
}

impl Registry {
    pub(crate) fn register(&self, agent: &Agent) {
        self.agents
            .write()
            .unwrap()
            .insert(agent.name.clone(), agent.clone());
    }

    fn get(&self, name: &str) -> Result<Agent, ActivityError> {
        self.agents
            .read()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| {
                ActivityError::application(ApplicationFailure::non_retryable(format!(
                    "agent {name:?} is not registered on this worker"
                )))
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StepInput {
    pub agent: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolCallInput {
    pub agent: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolCallOutput {
    pub content: Value,
    pub is_error: bool,
}

pub(crate) struct AgentActivities {
    pub registry: Registry,
}

#[activities]
impl AgentActivities {
    #[activity(name = "agentic.model_step")]
    pub(crate) async fn model_step(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: StepInput,
    ) -> Result<ModelResponse, ActivityError> {
        let agent = self.registry.get(&input.agent)?;
        let request = ModelRequest {
            instructions: agent.instructions.clone(),
            messages: input.messages,
            tools: super::workflow::agent_spec(&agent).tools,
        };
        let mut response = agent.model.complete(request).await.map_err(|e| {
            let failure = if e.retryable {
                ApplicationFailure::new(e.message)
            } else {
                ApplicationFailure::non_retryable(e.message)
            };
            ActivityError::application(failure)
        })?;
        assign_tool_use_ids(&mut response);
        Ok(response)
    }

    #[activity(name = "agentic.call_tool")]
    pub(crate) async fn call_tool(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: ToolCallInput,
    ) -> Result<ToolCallOutput, ActivityError> {
        let agent = self.registry.get(&input.agent)?;
        let Some(tool) = agent.tools.get(&input.name) else {
            // The model asked for something that doesn't exist. Tell it, don't crash.
            return Ok(ToolCallOutput {
                content: Value::String(format!("unknown tool {:?}", input.name)),
                is_error: true,
            });
        };
        match tool.call(input.args).await {
            Ok(content) => Ok(ToolCallOutput {
                content,
                is_error: false,
            }),
            // Bad arguments are the model's fault; feed the error back so it can fix them.
            Err(ToolError::InvalidArguments(msg)) => Ok(ToolCallOutput {
                content: Value::String(msg),
                is_error: true,
            }),
            // Real failures propagate so the engine retries the call.
            Err(ToolError::Failed(msg)) => {
                Err(ActivityError::application(ApplicationFailure::new(msg)))
            }
        }
    }
}

/// Models (and scripts) may leave tool-use ids empty. Give every call a stable id.
fn assign_tool_use_ids(response: &mut ModelResponse) {
    for (i, block) in response.content.iter_mut().enumerate() {
        if let crate::Content::ToolUse { id, .. } = block
            && id.is_empty()
        {
            *id = format!("call_{i}_{}", uuid::Uuid::new_v4().simple());
        }
    }
}
