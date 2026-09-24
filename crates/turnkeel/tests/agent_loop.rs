use turnkeel::testing::{ScriptedModel, text, tool_call};
use turnkeel::{Agent, Runtime, ToolCtx, tool};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Get the current weather for a city.
#[tool]
async fn get_weather(city: String) -> anyhow::Result<String> {
    Ok(format!("{city}: 22°C, sunny"))
}

/// Always fails, and must not be retried.
#[tool(idempotent = false)]
async fn broken() -> anyhow::Result<String> {
    anyhow::bail!("boom")
}

#[tokio::test]
async fn answers_without_tools() -> anyhow::Result<()> {
    let model = ScriptedModel::new().on_user("hello", text("Hi there."));
    let agent = Agent::builder("greeter").model(model).build();

    let run = turnkeel::testing::run(&agent, "hello").await?;

    assert_eq!(run.output, "Hi there.");
    run.assert_transcript()
        .user("hello")
        .assistant_contains("Hi there.")
        .end();
    Ok(())
}

#[tokio::test]
async fn calls_a_tool_then_answers() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user(
            "Lisbon",
            tool_call("get_weather", json!({ "city": "Lisbon" })),
        )
        .on_tool_result("get_weather", text("It's sunny in Lisbon."));
    let agent = Agent::builder("weather-bot")
        .model(model)
        .tool(get_weather)
        .build();

    let run = turnkeel::testing::run(&agent, "Weather in Lisbon?").await?;

    assert_eq!(run.output, "It's sunny in Lisbon.");
    run.assert_transcript()
        .user("Lisbon")
        .tool_call("get_weather")
        .tool_result()
        .assistant_contains("sunny")
        .end();

    let result = run.transcript[2].content.first().unwrap();
    assert!(
        matches!(result, turnkeel::Content::ToolResult { content, .. } if content == "Lisbon: 22°C, sunny")
    );
    Ok(())
}

#[tokio::test]
async fn bad_arguments_are_fed_back_to_the_model() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user(
            "weather",
            tool_call("get_weather", json!({ "town": "Lisbon" })),
        )
        .on_tool_result("get_weather", text("I couldn't look that up."));
    let agent = Agent::builder("weather-bot")
        .model(model)
        .tool(get_weather)
        .build();

    let run = turnkeel::testing::run(&agent, "weather?").await?;

    run.assert_transcript()
        .tool_call("get_weather")
        .tool_error()
        .assistant_contains("couldn't");
    Ok(())
}

#[tokio::test]
async fn failing_non_idempotent_tool_fails_the_run() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(tool_call("broken", json!({})));
    let agent = Agent::builder("bot").model(model).tool(broken).build();

    let err = turnkeel::testing::run(&agent, "go").await.unwrap_err();

    assert!(
        matches!(err, turnkeel::Error::RunFailed(ref m) if m.contains("boom")),
        "{err}"
    );
    Ok(())
}

#[tokio::test]
async fn unscripted_message_fails_loudly() -> anyhow::Result<()> {
    let model = ScriptedModel::new().on_user("something else", text("nope"));
    let agent = Agent::builder("bot").model(model).build();

    let err = turnkeel::testing::run(&agent, "hello").await.unwrap_err();

    assert!(
        matches!(err, turnkeel::Error::RunFailed(ref m) if m.contains("no rule")),
        "{err}"
    );
    Ok(())
}

#[tokio::test]
async fn runs_are_independent_on_one_runtime() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("one", text("1"))
        .on_user("two", text("2"));
    let agent = Agent::builder("counter").model(model).build();

    let turnkeel = Runtime::test().await?;
    let a = turnkeel.start(&agent, "one").await?;
    let b = turnkeel.start(&agent, "two").await?;
    assert_ne!(a.id(), b.id());
    assert_eq!(a.result().await?, "1");
    assert_eq!(b.result().await?, "2");
    turnkeel.shutdown().await?;
    Ok(())
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Returns how many times it has been called. Not idempotent, and not marked as such.
#[tool]
async fn count() -> anyhow::Result<usize> {
    Ok(COUNTER.fetch_add(1, Ordering::SeqCst) + 1)
}

static SENT: AtomicUsize = AtomicUsize::new(0);

/// Same behaviour, but honest about it.
#[tool(idempotent = false)]
async fn send() -> anyhow::Result<usize> {
    Ok(SENT.fetch_add(1, Ordering::SeqCst) + 1)
}

/// Idempotent the right way: keyed on the call.
#[tool]
async fn charge(ctx: &ToolCtx, amount: u32) -> anyhow::Result<String> {
    Ok(format!("charged {amount} (key {})", ctx.idempotency_key()))
}

#[tokio::test]
async fn test_runtime_catches_non_idempotent_tools() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(tool_call("count", json!({})));
    let agent = Agent::builder("bot").model(model).tool(count).build();

    let err = turnkeel::testing::run(&agent, "go").await.unwrap_err();

    assert!(
        matches!(err, turnkeel::Error::RunFailed(ref m) if m.contains("\"count\" is not idempotent")),
        "{err}"
    );
    Ok(())
}

#[tokio::test]
async fn tools_marked_non_idempotent_run_exactly_once() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("go", tool_call("send", json!({})))
        .on_tool_result("send", text("done"));
    let agent = Agent::builder("bot").model(model).tool(send).build();

    let run = turnkeel::testing::run(&agent, "go").await?;

    assert_eq!(run.output, "done");
    assert_eq!(SENT.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn idempotency_key_is_stable_across_the_double_call() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user("pay", tool_call("charge", json!({ "amount": 5 })))
        .on_tool_result("charge", text("paid"));
    let agent = Agent::builder("bot").model(model).tool(charge).build();

    let run = turnkeel::testing::run(&agent, "pay").await?;

    assert_eq!(run.output, "paid");
    run.assert_transcript().tool_call("charge").tool_result();
    let result = &run.transcript[2].content[0];
    let turnkeel::Content::ToolResult { content, .. } = result else {
        panic!("{result:?}")
    };
    let expected = format!("charged 5 (key {}/t0/c0)", run.run.id());
    assert_eq!(content, &serde_json::Value::String(expected));
    Ok(())
}
