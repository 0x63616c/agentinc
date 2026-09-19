use agentinc::testing::{Gate, ScriptedModel, text, tool_call};
use agentinc::{Agent, Agentinc, Content, Message, Role, SessionId, ToolCtx, tool};
use serde_json::json;

/// Echoes its idempotency key.
#[tool]
async fn echo_key(ctx: &ToolCtx) -> anyhow::Result<String> {
    Ok(ctx.idempotency_key().to_owned())
}

fn texts(transcript: &[Message]) -> Vec<String> {
    transcript.iter().map(Message::text).collect()
}

#[tokio::test]
async fn turns_share_history() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("hello", text("hi"))
        .on_user("again", text("hi again"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    session.send("hello").await?;
    session.wait_idle().await?;
    session.send("again").await?;
    session.wait_idle().await?;

    assert_eq!(
        texts(&session.transcript().await?),
        ["hello", "hi", "again", "hi again"]
    );
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
    session.send("start").await?;
    gate.entered().await;
    session.send("actually").await?;
    gate.release();
    session.wait_idle().await?;

    assert_eq!(
        texts(&session.transcript().await?),
        ["start", "", "", "actually", "changed course"]
    );
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
    session.send("start").await?;
    gate.entered().await;
    session.send("one").await?;
    session.send("two").await?;
    let cleared = session.clear_pending().await?;
    gate.release();
    session.wait_idle().await?;

    assert_eq!(texts(&cleared), ["one", "two"]);
    let transcript = session.transcript().await?;
    assert!(!transcript.iter().any(|m| m.text() == "one"));
    assert_eq!(transcript.last().unwrap().text(), "finished");
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
    session.send("one").await?;
    session.send("two").await?;
    session.wait_idle().await?;

    let transcript = session.transcript().await?;
    let users: Vec<String> = transcript
        .iter()
        .filter(|m| m.role == Role::User)
        .map(Message::text)
        .collect();
    assert_eq!(users, ["one", "two"]);
    assert_eq!(transcript.last().unwrap().text(), "r2");
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn wait_idle_on_a_fresh_session_returns_immediately() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(text("hi"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    session.wait_idle().await?;
    assert!(session.transcript().await?.is_empty());

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
    session.send("hello").await?;
    session.wait_idle().await?;

    assert_eq!(texts(&session.transcript().await?), ["hello", "hi"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn send_to_an_unknown_session_fails() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(text("hi"));
    let agent = Agent::builder("bot").model(model).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session_by_id(&agent, SessionId::new("agentinc-session-nope"));
    let err = session.send("hello").await.unwrap_err();
    assert!(matches!(err, agentinc::Error::Other(_)), "{err}");

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
    session.send("go").await?;
    session.wait_idle().await?;
    session.send("go").await?;
    session.wait_idle().await?;

    let keys: Vec<String> = session
        .transcript()
        .await?
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
