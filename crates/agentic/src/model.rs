use crate::Message;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Content;

/// A tool as described to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
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
    pub instructions: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}

/// Why the model stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
}

/// One response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub content: Vec<Content>,
    pub stop_reason: StopReason,
}

impl ModelResponse {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::Text { text: text.into() }],
            stop_reason: StopReason::EndTurn,
        }
    }

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
    pub message: String,
    /// Whether retrying the same request could succeed (rate limits, network).
    pub retryable: bool,
}

impl ModelError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

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
    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>>;
}
