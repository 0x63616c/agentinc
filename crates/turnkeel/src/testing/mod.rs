//! Test helpers. No real model provider is ever called.
//!
//! ```ignore
//! let model = ScriptedModel::new()
//!     .on_user("Weather in Lisbon?", tool_call("get_weather", json!({"city": "Lisbon"})))
//!     .on_tool_result("get_weather", text("It's sunny in Lisbon."));
//!
//! let agent = Agent::builder("bot").model(model).tool(get_weather).build();
//! let run = turnkeel::testing::run(&agent, "Weather in Lisbon?").await?;
//! run.assert_transcript().user("Weather in Lisbon?").tool_call("get_weather").tool_result().assistant_contains("sunny");
//! ```

mod script;
mod scripted;
mod transcript;

pub use script::{ModelCall, Script, ScriptModel, ScriptTool, ToolCall};
pub use scripted::{ScriptedModel, text, tool_call};
pub use transcript::TranscriptAssert;

use crate::{Agent, Runtime, Error, Message, Run};

/// A finished run plus its transcript, for asserting on.
#[derive(Debug)]
pub struct TestRun {
    pub run: Run,
    pub output: String,
    pub transcript: Vec<Message>,
}

impl TestRun {
    pub fn assert_transcript(&self) -> TranscriptAssert<'_> {
        TranscriptAssert::new(&self.transcript)
    }
}

/// Spin up a test runtime, run the agent to completion, tear everything down.
pub async fn run(agent: &Agent, input: impl Into<Message>) -> Result<TestRun, Error> {
    let turnkeel = Runtime::test().await?;
    let run = turnkeel.start(agent, input).await?;
    let output = run.result().await?;
    let transcript = run.transcript().await?;
    turnkeel.shutdown().await?;
    Ok(TestRun {
        run,
        output,
        transcript,
    })
}
