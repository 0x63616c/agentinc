use crate::Message;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Content;

/// A tool as described to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    /// The tool's name, as the model refers to it.
    pub name: String,
    /// What the tool does, to help the model decide when to call it.
    pub description: String,
    /// A JSON schema for the tool's arguments object.
    pub input_schema: Value,
    /// See [`crate::Tool::idempotent`].
    #[serde(default = "default_true")]
    pub idempotent: bool,
}

fn default_true() -> bool {
    true
}

/// One request to a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    /// The agent's instructions for the model.
    pub instructions: String,
    /// The conversation so far.
    pub messages: Vec<Message>,
    /// The tools the model may call.
    pub tools: Vec<ToolSpec>,
}

/// Why the model stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model finished its answer and is waiting for the next message.
    EndTurn,
    /// The model wants one or more tools called before it continues.
    ToolUse,
}

/// One response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// What the model said, including any tool calls.
    pub content: Vec<Content>,
    /// Why the model stopped producing output.
    pub stop_reason: StopReason,
}

impl ModelResponse {
    /// A reply of plain text that ends the turn.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::Text { text: text.into() }],
            stop_reason: StopReason::EndTurn,
        }
    }

    /// A reply that calls one tool with the given arguments and waits for its result.
    pub fn tool_call(name: impl Into<String>, input: Value) -> Self {
        Self {
            content: vec![Content::ToolUse {
                id: String::new(),
                name: name.into(),
                input,
            }],
            stop_reason: StopReason::ToolUse,
        }
    }
}

/// A model provider error.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ModelError {
    /// A human-readable description of what went wrong.
    pub message: String,
    /// Whether retrying the same request could succeed (rate limits, network).
    pub retryable: bool,
}

impl ModelError {
    /// An error that may go away if the same request is tried again.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    /// An error that retrying will not fix.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
}

/// An LLM provider.
pub trait Model: Send + Sync + 'static {
    /// Identifier shown in logs, e.g. `claude-sonnet-5`.
    fn id(&self) -> &str;
    /// Sends the request to the model and resolves to its reply.
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>>;
}

/// A shared model is a model. Lets one provider client back many agents.
impl<M: Model + ?Sized> Model for std::sync::Arc<M> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        (**self).complete(request)
    }
}
