//! Side effects: model calls and tool calls. The only place user code runs.

use crate::{Agent, AgentSource, Message, ModelRequest, ModelResponse, ToolCtx, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, RwLock},
};
use temporalio_macros::activities;
use temporalio_sdk::{
    ApplicationFailure,
    activities::{ActivityContext, ActivityError},
};

/// Resolved definitions kept after a miss. A miss simply asks the source again, so the
/// bound only trades memory for repeated lookups; it is not a correctness limit.
const CACHE_CAPACITY: usize = 128;

/// Agents this worker can run, by name.
///
/// Pinned agents were installed when the runtime started and never leave. Everything else
/// (agents passed to `start`, agents resolved through the [`AgentSource`]) sits in a
/// bounded, least-recently-used cache, so a long-lived worker that runs many short-lived
/// agents does not grow without limit.
#[derive(Clone, Default)]
pub(crate) struct Registry {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    pinned: RwLock<HashMap<String, Agent>>,
    cache: Mutex<Cache>,
    source: Option<Arc<dyn AgentSource>>,
}

#[derive(Default)]
struct Cache {
    agents: HashMap<String, Agent>,
    /// Names from least to most recently used.
    order: VecDeque<String>,
}

impl Cache {
    fn get(&mut self, name: &str) -> Option<Agent> {
        let agent = self.agents.get(name).cloned()?;
        self.order.retain(|n| n != name);
        self.order.push_back(name.to_owned());
        Some(agent)
    }

    fn insert(&mut self, agent: Agent) {
        let name = agent.name.clone();
        self.order.retain(|n| n != &name);
        self.order.push_back(name.clone());
        self.agents.insert(name, agent);
        while self.agents.len() > CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.agents.remove(&oldest);
            }
        }
    }
}

impl Registry {
    pub(crate) fn new(pinned: &[Agent], source: Option<Arc<dyn AgentSource>>) -> Self {
        let registry = Self {
            inner: Arc::new(Inner {
                source,
                ..Default::default()
            }),
        };
        for agent in pinned {
            registry
                .inner
                .pinned
                .write()
                .unwrap()
                .insert(agent.name.clone(), agent.clone());
        }
        registry
    }

    /// Make an agent available for the runs this process starts with it.
    pub(crate) fn register(&self, agent: &Agent) {
        if self.inner.pinned.read().unwrap().contains_key(&agent.name) {
            return;
        }
        self.inner.cache.lock().unwrap().insert(agent.clone());
    }

    async fn get(&self, name: &str) -> Result<Agent, ActivityError> {
        if let Some(agent) = self.inner.pinned.read().unwrap().get(name) {
            return Ok(agent.clone());
        }
        if let Some(agent) = self.inner.cache.lock().unwrap().get(name) {
            return Ok(agent);
        }
        let Some(source) = &self.inner.source else {
            return Err(ActivityError::application(
                ApplicationFailure::non_retryable(format!(
                    "agent {name:?} is not registered on this worker"
                )),
            ));
        };
        match source.resolve(name).await {
            Ok(Some(agent)) if agent.name == name => {
                self.inner.cache.lock().unwrap().insert(agent.clone());
                Ok(agent)
            }
            Ok(Some(agent)) => Err(ActivityError::application(
                ApplicationFailure::non_retryable(format!(
                    "agent source resolved {name:?} to a definition named {:?}",
                    agent.name
                )),
            )),
            Ok(None) => Err(ActivityError::application(
                ApplicationFailure::non_retryable(format!(
                    "agent {name:?} is not registered on this worker and its agent source has no definition for it"
                )),
            )),
            // The source could not answer right now; the step retries.
            Err(error) => Err(ActivityError::application(ApplicationFailure::new(
                format!("agent source failed to resolve {name:?}: {error}"),
            ))),
        }
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
    /// Engine-generated. See [`ToolCtx::idempotency_key`].
    pub idempotency_key: String,
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
    /// Test mode: call every idempotent tool twice and require identical results.
    pub check_idempotency: bool,
}

/// Current activity type names. Workflows schedule by name (see `conversation::ModelStep`);
/// `build_worker` checks these against what is registered.
pub(crate) const MODEL_STEP: &str = "turnkeel.model_step";
pub(crate) const CALL_TOOL: &str = "turnkeel.call_tool";

/// Each activity is registered under its current name and its legacy alias; both run
/// the same code so retained histories keep making progress.
#[activities]
impl AgentActivities {
    #[activity(name = MODEL_STEP)]
    pub(crate) async fn model_step(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StepInput,
    ) -> Result<ModelResponse, ActivityError> {
        self.step(ctx, input).await
    }

    #[activity(name = crate::engine::legacy::MODEL_STEP)]
    pub(crate) async fn legacy_model_step(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StepInput,
    ) -> Result<ModelResponse, ActivityError> {
        self.step(ctx, input).await
    }

    #[activity(name = CALL_TOOL)]
    pub(crate) async fn call_tool(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ToolCallInput,
    ) -> Result<ToolCallOutput, ActivityError> {
        self.call(ctx, input).await
    }

    #[activity(name = crate::engine::legacy::CALL_TOOL)]
    pub(crate) async fn legacy_call_tool(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ToolCallInput,
    ) -> Result<ToolCallOutput, ActivityError> {
        self.call(ctx, input).await
    }
}

impl AgentActivities {
    async fn step(
        &self,
        ctx: ActivityContext,
        input: StepInput,
    ) -> Result<ModelResponse, ActivityError> {
        let agent = self.registry.get(&input.agent).await?;
        let request = ModelRequest {
            instructions: agent.instructions.clone(),
            messages: input.messages,
            tools: super::conversation::agent_spec(&agent).tools,
        };
        let mut response = cancellable(&ctx, agent.model.complete(request))
            .await?
            .map_err(|e| {
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

    async fn call(
        &self,
        ctx: ActivityContext,
        input: ToolCallInput,
    ) -> Result<ToolCallOutput, ActivityError> {
        let agent = self.registry.get(&input.agent).await?;
        let Some(tool) = agent.tools.get(&input.name) else {
            // The model asked for something that doesn't exist. Tell it, don't crash.
            return Ok(ToolCallOutput {
                content: Value::String(format!("unknown tool {:?}", input.name)),
                is_error: true,
            });
        };
        let tool_ctx = ToolCtx::new(&input.idempotency_key);
        let first = cancellable(&ctx, tool.call(tool_ctx.clone(), input.args.clone())).await?;

        if self.check_idempotency && tool.idempotent() && first.is_ok() {
            let second = cancellable(&ctx, tool.call(tool_ctx, input.args)).await?;
            if !matches!(&second, Ok(v) if Some(v) == first.as_ref().ok()) {
                return Err(ActivityError::application(
                    ApplicationFailure::non_retryable(format!(
                        "tool {:?} is not idempotent: calling it twice with the same arguments gave\n  first:  {:?}\n  second: {:?}\nEither make it idempotent (use ctx.idempotency_key()) or mark it #[tool(idempotent = false)].",
                        input.name, first, second
                    )),
                ));
            }
        }

        match first {
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

/// Providers may leave tool-use ids empty. Every call needs one so its result can be
/// correlated back to it in the conversation. (Idempotency keys are separate; see the workflow.)
fn assign_tool_use_ids(response: &mut ModelResponse) {
    for (i, block) in response.content.iter_mut().enumerate() {
        if let crate::Content::ToolUse { id, .. } = block
            && id.is_empty()
        {
            *id = format!("call_{i}_{}", uuid::Uuid::new_v4().simple());
        }
    }
}

/// Heartbeats detect a dead worker and deliver cancellation to live I/O futures.
/// Dropping a future must stop its owned processes; tools still fence/receipt effects.
pub(super) async fn cancellable<T>(
    ctx: &ActivityContext,
    future: impl std::future::Future<Output = T>,
) -> Result<T, ActivityError> {
    tokio::pin!(future);
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            _ = ctx.cancelled() => return Err(ActivityError::cancelled()),
            output = &mut future => return Ok(output),
            _ = heartbeat.tick() => { ctx.record_heartbeat(()).await?; }
        }
    }
}
