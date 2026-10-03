use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Who produced a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// A message written by the user.
    User,
    /// A message written by the model.
    Assistant,
}

/// One block inside a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Plain text.
    Text {
        /// The text itself.
        text: String,
    },
    /// Opaque provider context (for example encrypted reasoning) needed on the
    /// next model call. Consumers must preserve it without displaying it as text.
    ModelContext {
        /// Which provider produced this context.
        provider: String,
        /// The provider's own payload, to be sent back unchanged.
        value: Value,
    },
    /// The model asking for a tool to be called.
    ToolUse {
        /// Identifies this call, so its result can refer back to it.
        id: String,
        /// The name of the tool to call.
        name: String,
        /// The arguments to call the tool with, as a JSON object.
        input: Value,
    },
    /// The outcome of a tool call, sent back to the model.
    ToolResult {
        /// The ID of the tool call this answers.
        tool_use_id: String,
        /// What the tool returned, or the error it reported.
        content: Value,
        /// Whether the tool failed, in which case `content` describes the failure.
        is_error: bool,
    },
}

/// A message in the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Who produced the message.
    pub role: Role,
    /// The blocks that make up the message, in order.
    pub content: Vec<Content>,
}

impl Message {
    /// Creates a user message holding one text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Content::Text { text: text.into() }],
        }
    }

    /// Creates an assistant message from the given content blocks.
    pub fn assistant(content: Vec<Content>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    /// All text blocks joined together.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Tool calls requested in this message.
    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &Value)> {
        self.content.iter().filter_map(|c| match c {
            Content::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        })
    }

    /// The first tool call with this name, if any.
    pub fn tool_call(&self, name: &str) -> Option<&Value> {
        self.tool_uses()
            .find(|(_, n, _)| *n == name)
            .map(|(_, _, input)| input)
    }
}

impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Message::user(s)
    }
}

impl From<String> for Message {
    fn from(s: String) -> Self {
        Message::user(s)
    }
}
