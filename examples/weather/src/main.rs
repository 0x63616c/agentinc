//! The smallest possible agentic app: one agent, one tool, one question.
//!
//! Uses a scripted model so it runs without an API key. Swap `model()` for a real provider
//! once one exists. Run with `cargo run -p weather`.

use agentic::testing::{ScriptedModel, text, tool_call};
use agentic::{Agent, Agentic, tool};
use serde_json::json;

/// Get the current weather for a city.
#[tool]
async fn get_weather(city: String) -> anyhow::Result<String> {
    // Real I/O goes here. Failures are retried automatically.
    Ok(format!("{city}: 22°C and sunny"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model = ScriptedModel::new()
        .on_user(
            "Lisbon",
            tool_call("get_weather", json!({ "city": "Lisbon" })),
        )
        .on_tool_result("get_weather", text("It's 22°C and sunny in Lisbon."));

    let agentic = Agentic::local().await?;

    let agent = Agent::builder("weather-bot")
        .model(model)
        .instructions("Answer in one sentence.")
        .tool(get_weather)
        .build();

    let run = agentic
        .start(&agent, "What's the weather in Lisbon?")
        .await?;
    println!("run {}", run.id());
    println!("{}", run.result().await?);

    agentic.shutdown().await?;
    Ok(())
}
