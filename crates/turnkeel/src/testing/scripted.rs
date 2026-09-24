use crate::{Content, Message, Model, ModelError, ModelRequest, ModelResponse, Role};
use futures::future::BoxFuture;
use serde_json::Value;
use std::sync::Arc;

/// A text reply.
pub fn text(s: impl Into<String>) -> ModelResponse {
    ModelResponse::text(s)
}

/// A tool call reply.
pub fn tool_call(name: impl Into<String>, args: Value) -> ModelResponse {
    ModelResponse::tool_call(name, args)
}

#[derive(Clone)]
enum Rule {
    /// Last message is from the user and its text contains this.
    User(String, ModelResponse),
    /// Last message carries a result for a call to this tool.
    ToolResult(String, ModelResponse),
    Otherwise(ModelResponse),
}

/// A deterministic model for tests. Responds based on the last message it sees.
///
/// Rules are checked in the order they were added. If nothing matches, the model fails the run
/// with a non-retryable error that includes the transcript, so the test fails fast and loud.
#[derive(Clone, Default)]
pub struct ScriptedModel {
    rules: Arc<Vec<Rule>>,
}

impl ScriptedModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reply when the latest user message contains `needle`.
    pub fn on_user(mut self, needle: impl Into<String>, reply: ModelResponse) -> Self {
        Arc::make_mut(&mut self.rules).push(Rule::User(needle.into(), reply));
        self
    }

    /// Reply when the latest message is a result from `tool`.
    pub fn on_tool_result(mut self, tool: impl Into<String>, reply: ModelResponse) -> Self {
        Arc::make_mut(&mut self.rules).push(Rule::ToolResult(tool.into(), reply));
        self
    }

    /// Reply when nothing else matched.
    pub fn otherwise(mut self, reply: ModelResponse) -> Self {
        Arc::make_mut(&mut self.rules).push(Rule::Otherwise(reply));
        self
    }

    fn respond(&self, req: &ModelRequest) -> Option<ModelResponse> {
        let last = req.messages.last()?;
        let user_text = (last.role == Role::User).then(|| last.text());
        let tool_names = tool_result_names(&req.messages);

        self.rules.iter().find_map(|rule| match rule {
            Rule::User(needle, reply) => user_text
                .as_deref()
                .filter(|t| t.contains(needle.as_str()))
                .map(|_| reply.clone()),
            Rule::ToolResult(name, reply) => {
                tool_names.iter().any(|n| n == name).then(|| reply.clone())
            }
            Rule::Otherwise(reply) => Some(reply.clone()),
        })
    }
}

/// Names of the tools whose results appear in the last message.
fn tool_result_names(messages: &[Message]) -> Vec<String> {
    let Some(last) = messages.last() else {
        return vec![];
    };
    let ids: Vec<&str> = last
        .content
        .iter()
        .filter_map(|c| match c {
            Content::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        return vec![];
    }
    messages
        .iter()
        .rev()
        .flat_map(|m| m.tool_uses())
        .filter(|(id, _, _)| ids.contains(id))
        .map(|(_, name, _)| name.to_owned())
        .collect()
}

impl Model for ScriptedModel {
    fn id(&self) -> &str {
        "scripted"
    }

    fn complete(
        &self,
        request: ModelRequest,
    ) -> BoxFuture<'static, Result<ModelResponse, ModelError>> {
        let result = self.respond(&request).ok_or_else(|| {
            ModelError::fatal(format!(
                "ScriptedModel has no rule for the latest message.\nTranscript:\n{}",
                format_transcript(&request.messages)
            ))
        });
        Box::pin(async move { result })
    }
}

pub(crate) fn format_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        for c in &m.content {
            let line = match c {
                Content::ModelContext { provider, .. } => format!("context from {provider}"),
                Content::Text { text } => format!("{:?}: {text}", m.role),
                Content::ToolUse { name, input, .. } => {
                    format!("{:?}: call {name}({input})", m.role)
                }
                Content::ToolResult {
                    content, is_error, ..
                } => {
                    format!(
                        "tool result{}: {content}",
                        if *is_error { " (error)" } else { "" }
                    )
                }
            };
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}
