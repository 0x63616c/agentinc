use agentinc::testing::{Gate, ScriptedModel, text, tool_call};
use agentinc::{Agent, Agentinc, Content, Event, Message, Role, SessionId, ToolCtx, tool};
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::json;

type Events = BoxStream<'static, Result<Event, agentinc::Error>>;

/// Echoes its idempotency key.
#[tool]
async fn echo_key(ctx: &ToolCtx) -> anyhow::Result<String> {
    Ok(ctx.idempotency_key().to_owned())
}

/// Read events up to and including the next `TurnEnded`; return the messages seen.
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

fn texts(messages: &[Message]) -> Vec<String> {
    messages.iter().map(Message::text).collect()
}

#[tokio::test]
async fn turns_share_history() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("hello", text("hi"))
        .on_user("again", text("hi again"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("hello").await?;
    let first = next_turn(&mut events).await?;
    session.send("again").await?;
    let second = next_turn(&mut events).await?;

    assert_eq!(texts(&first), ["hello", "hi"]);
    assert_eq!(texts(&second), ["again", "hi again"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn events_include_tool_calls_and_results_in_order() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("go", tool_call("echo_key", json!({})))
        .on_tool_result("echo_key", text("ok"));
    let agent = Agent::builder("bot").model(model).tool(echo_key).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("go").await?;
    let turn = next_turn(&mut events).await?;

    assert_eq!(turn.len(), 4);
    assert_eq!(turn[0].text(), "go");
    assert!(turn[1].tool_call("echo_key").is_some());
    assert!(matches!(turn[2].content[0], Content::ToolResult { .. }));
    assert_eq!(turn[3].text(), "ok");
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn late_subscriber_replays_history_then_continues() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("hello", text("hi"))
        .on_user("again", text("hi again"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut live = session.events();
    session.send("hello").await?;
    next_turn(&mut live).await?;

    // Subscribe after the first turn: history first, then live.
    let mut late = session.events();
    assert_eq!(texts(&next_turn(&mut late).await?), ["hello", "hi"]);
    session.send("again").await?;
    assert_eq!(texts(&next_turn(&mut late).await?), ["again", "hi again"]);
    assert_eq!(texts(&next_turn(&mut live).await?), ["again", "hi again"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn message_sent_mid_turn_is_seen_at_the_next_step() -> anyhow::Result<()> {
    let gate = Gate::new("wait");
    let model = ScriptedModel::new()
        .on_user("start", tool_call("wait", json!({})))
        .on_user("actually", text("changed course"))
        .on_tool_result("wait", text("finished"));
    let agent = Agent::builder("bot")
        .model(model)
        .tool(gate.clone())
        .build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("start").await?;
    gate.entered().await;
    session.send("actually").await?;
    gate.release();
    let turn = next_turn(&mut events).await?;

    assert_eq!(
        texts(&turn),
        ["start", "", "", "actually", "changed course"]
    );
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn events_arrive_while_a_turn_is_still_running() -> anyhow::Result<()> {
    let gate = Gate::new("wait");
    let model = ScriptedModel::new()
        .on_user("start", tool_call("wait", json!({})))
        .on_tool_result("wait", text("finished"));
    let agent = Agent::builder("bot")
        .model(model)
        .tool(gate.clone())
        .build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("start").await?;
    gate.entered().await;

    // The turn is blocked in the tool, yet the user message and the tool call are already out.
    let Event::Message(user) = events.next().await.unwrap()? else {
        panic!("expected a message");
    };
    let Event::Message(call) = events.next().await.unwrap()? else {
        panic!("expected a message");
    };
    assert_eq!(user.text(), "start");
    assert!(call.tool_call("wait").is_some());

    gate.release();
    assert_eq!(texts(&next_turn(&mut events).await?), ["", "finished"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn clear_pending_returns_unseen_messages() -> anyhow::Result<()> {
    let gate = Gate::new("wait");
    let model = ScriptedModel::new()
        .on_user("start", tool_call("wait", json!({})))
        .on_tool_result("wait", text("finished"));
    let agent = Agent::builder("bot")
        .model(model)
        .tool(gate.clone())
        .build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("start").await?;
    gate.entered().await;
    session.send("one").await?;
    session.send("two").await?;
    let cleared = session.clear_pending().await?;
    gate.release();
    let turn = next_turn(&mut events).await?;

    assert_eq!(texts(&cleared), ["one", "two"]);
    assert_eq!(texts(&turn), ["start", "", "", "finished"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn clear_pending_with_nothing_pending_is_empty() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(text("hi"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    assert!(session.clear_pending().await?.is_empty());

    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn back_to_back_messages_are_all_seen_in_order() -> anyhow::Result<()> {
    // Whether they land in one turn or two depends on timing; either way nothing is lost.
    let model = ScriptedModel::new()
        .on_user("one", text("r1"))
        .on_user("two", text("r2"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("one").await?;
    session.send("two").await?;

    let mut seen = next_turn(&mut events).await?;
    if seen.last().map(Message::text) != Some("r2".into()) {
        seen.extend(next_turn(&mut events).await?);
    }
    let users: Vec<String> = seen
        .iter()
        .filter(|m| m.role == Role::User)
        .map(Message::text)
        .collect();
    assert_eq!(users, ["one", "two"]);
    assert_eq!(seen.last().unwrap().text(), "r2");
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn session_by_id_reattaches() -> anyhow::Result<()> {
    let model = ScriptedModel::new().on_user("hello", text("hi"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let id = agentinc.session(&agent).await?.id().clone();
    let session = agentinc.session_by_id(&agent, id);
    let mut events = session.events();
    session.send("hello").await?;

    assert_eq!(texts(&next_turn(&mut events).await?), ["hello", "hi"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn unknown_session_fails_on_send_and_on_events() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(text("hi"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session_by_id(&agent, SessionId::new("agentinc-session-nope"));
    let err = session.send("hello").await.unwrap_err();
    assert!(matches!(err, agentinc::Error::Other(_)), "{err}");

    let mut events = session.events();
    assert!(events.next().await.unwrap().is_err());
    assert!(events.next().await.is_none(), "stream ends after an error");

    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn tool_idempotency_keys_are_unique_across_turns() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("go", tool_call("echo_key", json!({})))
        .on_tool_result("echo_key", text("ok"));
    let agent = Agent::builder("bot").model(model).tool(echo_key).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    let mut events = session.events();
    session.send("go").await?;
    let mut all = next_turn(&mut events).await?;
    session.send("go").await?;
    all.extend(next_turn(&mut events).await?);

    let keys: Vec<String> = all
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|c| match c {
            Content::ToolResult { content, .. } => content.as_str().map(str::to_owned),
            _ => None,
        })
        .collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
    assert!(keys[0].starts_with(session.id().as_str()), "{}", keys[0]);
    agentinc.shutdown().await?;
    Ok(())
}
