use agentinc::testing::{ScriptedModel, text, tool_call};
use agentinc::{Agent, Agentinc, Message, Role, Session, tool};
use serde_json::json;
use std::time::Duration;

/// Takes a while.
#[tool]
async fn slow() -> anyhow::Result<String> {
    tokio::time::sleep(Duration::from_millis(400)).await;
    Ok("done".into())
}

/// Poll the transcript until the last message is an assistant reply containing `needle`.
async fn wait_for_reply(session: &Session, needle: &str) -> anyhow::Result<Vec<Message>> {
    for _ in 0..200 {
        let transcript = session.transcript().await?;
        if let Some(last) = transcript.last()
            && last.role == Role::Assistant
            && last.text().contains(needle)
        {
            return Ok(transcript);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("no reply containing {needle:?} within 10s")
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
    wait_for_reply(&session, "hi").await?;
    session.send("again").await?;
    let transcript = wait_for_reply(&session, "hi again").await?;

    let texts: Vec<String> = transcript.iter().map(Message::text).collect();
    assert_eq!(texts, ["hello", "hi", "again", "hi again"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn message_sent_mid_turn_is_seen_at_the_next_step() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("start", tool_call("slow", json!({})))
        .on_user("actually", text("changed course"))
        .on_tool_result("slow", text("finished"));
    let agent = Agent::builder("bot").model(model).tool(slow).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    session.send("start").await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    session.send("actually").await?;
    let transcript = wait_for_reply(&session, "changed course").await?;

    let texts: Vec<String> = transcript.iter().map(Message::text).collect();
    assert_eq!(texts, ["start", "", "", "actually", "changed course"]);
    agentinc.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn clear_pending_returns_unseen_messages() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("start", tool_call("slow", json!({})))
        .on_tool_result("slow", text("finished"));
    let agent = Agent::builder("bot").model(model).tool(slow).build();
    let agentinc = Agentinc::test().await?;

    let session = agentinc.session(&agent).await?;
    session.send("start").await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    session.send("one").await?;
    session.send("two").await?;
    let cleared = session.clear_pending().await?;
    let transcript = wait_for_reply(&session, "finished").await?;

    let cleared: Vec<String> = cleared.iter().map(Message::text).collect();
    assert_eq!(cleared, ["one", "two"]);
    assert!(!transcript.iter().any(|m| m.text() == "one"));
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
    wait_for_reply(&session, "hi").await?;

    agentinc.shutdown().await?;
    Ok(())
}
