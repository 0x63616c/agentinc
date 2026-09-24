use crate::Message;
use serde::{Deserialize, Serialize};

/// Something that happened in a conversation, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Event {
    /// A message was added to the conversation: user input, a model reply, or tool results.
    Message(Message),
    /// The model finished replying and the agent is idle until the next message.
    TurnEnded,
}
