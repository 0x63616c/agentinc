//! Tests driven by `testing::Script`: the test decides every model reply and tool result
//! at the moment it happens.

use agentinc::testing::{Script, text};
use agentinc::{Agent, Agentinc, Content, Event, Message, ModelResponse, StopReason};
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::json;

type Events = BoxStream<'static, Result<Event, agentinc::Error>>;

async fn next_turn(events: &mut Events) -> anyhow::Result<Vec<Message>> {
    let mut messages = Vec::new();
    while let Some(event) = events.next().await {
        match event? {
            Event::Message(m) => messages.push(m),
            Event::TurnEnded => return Ok(messages),
            other => anyhow::bail!("unexpected event {other:?}"),
        }
    }
    anyhow::bail!("event stream ended without TurnEnded")
}

fn calls(names: &[&str]) -> ModelResponse {
    ModelResponse {
        content: names
            .iter()
            .map(|n| Content::ToolUse {
                id: String::new(),
                name: (*n).to_owned(),
                input: json!({}),
            })
            .collect(),
        stop_reason: StopReason::ToolUse,
    }
}

#[tokio::test]
async fn three_tool_calls_run_in_the_order_the_model_asked() -> anyhow::Result<()> {
    let script = Script::new();
    let agent = Agent::builder("bot")
        .model(script.model())
        .tool(script.tool("a"))
        .tool(script.tool("b"))
        .tool(script.tool("c"))
        .build();
    let agentinc = Agentinc::test().await?;
    let session = agentinc.session(&agent).await?;
    let mut events = session.events();

    session.send("go").await?;
    script
        .next_model_call()
        .await
        .reply(calls(&["a", "b", "c"]));
    for name in ["a", "b", "c"] {
        let call = script.next_tool_call().await;
        assert_eq!(call.name(), name);
        call.succeed(json!(format!("result of {name}")));
    }
    script.next_model_call().await.reply(text("done"));

    let turn = next_turn(&mut events).await?;
    assert_eq!(turn.len(), 4);
    let results: Vec<&Content> = turn[2].content.iter().collect();
    assert_eq!(results.len(), 3);
    assert!(matches!(results[0], Content::ToolResult { content, .. } if content == "result of a"));
    assert!(matches!(results[2], Content::ToolResult { content, .. } if content == "result of c"));
    assert_eq!(turn[3].text(), "done");
    agentinc.shutdown().await?;
    Ok(())
}
