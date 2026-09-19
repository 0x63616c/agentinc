use super::scripted::format_transcript;
use crate::{Content, Message, Role};

/// Fluent, ordered assertions over a transcript. Panics with the full transcript on failure.
pub struct TranscriptAssert<'a> {
    messages: &'a [Message],
    /// Flattened (message index, content index) cursor.
    pos: usize,
    flat: Vec<(Role, &'a Content)>,
}

impl<'a> TranscriptAssert<'a> {
    pub(crate) fn new(messages: &'a [Message]) -> Self {
        let flat = messages
            .iter()
            .flat_map(|m| m.content.iter().map(move |c| (m.role, c)))
            .collect();
        Self {
            messages,
            pos: 0,
            flat,
        }
    }

    fn next_matching(&mut self, what: &str, pred: impl Fn(Role, &Content) -> bool) -> &mut Self {
        match self.flat[self.pos..].iter().position(|(r, c)| pred(*r, c)) {
            Some(offset) => {
                self.pos += offset + 1;
                self
            }
            None => panic!(
                "expected {what} after position {} but did not find it.\nTranscript:\n{}",
                self.pos,
                format_transcript(self.messages)
            ),
        }
    }

    /// A user text block containing `needle`.
    pub fn user(&mut self, needle: &str) -> &mut Self {
        self.next_matching(&format!("user message containing {needle:?}"), |r, c| {
            r == Role::User && matches!(c, Content::Text { text } if text.contains(needle))
        })
    }

    /// An assistant text block containing `needle`.
    pub fn assistant_contains(&mut self, needle: &str) -> &mut Self {
        self.next_matching(
            &format!("assistant message containing {needle:?}"),
            |r, c| {
                r == Role::Assistant && matches!(c, Content::Text { text } if text.contains(needle))
            },
        )
    }

    /// A call to the named tool.
    pub fn tool_call(&mut self, name: &str) -> &mut Self {
        self.next_matching(
            &format!("call to tool {name:?}"),
            |_, c| matches!(c, Content::ToolUse { name: n, .. } if n == name),
        )
    }

    /// Any successful tool result.
    pub fn tool_result(&mut self) -> &mut Self {
        self.next_matching("tool result", |_, c| {
            matches!(
                c,
                Content::ToolResult {
                    is_error: false,
                    ..
                }
            )
        })
    }

    /// Any failed tool result.
    pub fn tool_error(&mut self) -> &mut Self {
        self.next_matching("tool error", |_, c| {
            matches!(c, Content::ToolResult { is_error: true, .. })
        })
    }

    /// Nothing else follows.
    pub fn end(&mut self) {
        if self.pos != self.flat.len() {
            panic!(
                "expected end of transcript at position {} but {} blocks remain.\nTranscript:\n{}",
                self.pos,
                self.flat.len() - self.pos,
                format_transcript(self.messages)
            );
        }
    }
}
