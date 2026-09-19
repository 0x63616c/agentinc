use agentic::testing::{ScriptedModel, text, tool_call};
use agentic::{Agent, Agentic, tool};
use serde_json::json;

/// Get the current weather for a city.
#[tool]
async fn get_weather(city: String) -> anyhow::Result<String> {
    Ok(format!("{city}: 22°C, sunny"))
}

/// Always fails.
#[tool]
async fn broken() -> anyhow::Result<String> {
    anyhow::bail!("boom")
}

#[tokio::test]
async fn answers_without_tools() -> anyhow::Result<()> {
    let model = ScriptedModel::new().on_user("hello", text("Hi there."));
    let agent = Agent::builder("greeter").model(model).build();

    let run = agentic::testing::run(&agent, "hello").await?;

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

    let run = agentic::testing::run(&agent, "Weather in Lisbon?").await?;

    assert_eq!(run.output, "It's sunny in Lisbon.");
    run.assert_transcript()
        .user("Lisbon")
        .tool_call("get_weather")
        .tool_result()
        .assistant_contains("sunny")
        .end();

    let result = run.transcript[2].content.first().unwrap();
    assert!(
        matches!(result, agentic::Content::ToolResult { content, .. } if content == "Lisbon: 22°C, sunny")
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

    let run = agentic::testing::run(&agent, "weather?").await?;

    run.assert_transcript()
        .tool_call("get_weather")
        .tool_error()
        .assistant_contains("couldn't");
    Ok(())
}

#[tokio::test]
async fn failing_tool_fails_the_run() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(tool_call("broken", json!({})));
    let agent = Agent::builder("bot").model(model).tool(broken).build();

    let err = agentic::testing::run(&agent, "go").await.unwrap_err();

    assert!(
        matches!(err, agentic::Error::RunFailed(ref m) if m.contains("boom")),
        "{err}"
    );
    Ok(())
}

#[tokio::test]
async fn unscripted_message_fails_loudly() -> anyhow::Result<()> {
    let model = ScriptedModel::new().on_user("something else", text("nope"));
    let agent = Agent::builder("bot").model(model).build();

    let err = agentic::testing::run(&agent, "hello").await.unwrap_err();

    assert!(
        matches!(err, agentic::Error::RunFailed(ref m) if m.contains("no rule")),
        "{err}"
    );
    Ok(())
}

#[tokio::test]
async fn turn_limit_stops_runaway_loops() -> anyhow::Result<()> {
    let model = ScriptedModel::new().otherwise(tool_call("get_weather", json!({ "city": "x" })));
    let agent = Agent::builder("bot")
        .model(model)
        .tool(get_weather)
        .max_turns(3)
        .build();

    let err = agentic::testing::run(&agent, "go").await.unwrap_err();

    assert!(
        matches!(err, agentic::Error::RunFailed(ref m) if m.contains("3 turns")),
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

    let agentic = Agentic::test().await?;
    let a = agentic.start(&agent, "one").await?;
    let b = agentic.start(&agent, "two").await?;
    assert_ne!(a.id(), b.id());
    assert_eq!(a.result().await?, "1");
    assert_eq!(b.result().await?, "2");
    agentic.shutdown().await?;
    Ok(())
}
