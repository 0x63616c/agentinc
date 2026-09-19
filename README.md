# agentinc

**Write the agent. Not the state machine.**

Durable AI agents in Rust — ordinary async code that survives crashes, restarts
and flaky tools. Define an agent, start a run, get a result.

Powered by [Temporal](https://temporal.io), so you never write a workflow.

```rust
use agentinc::{Agent, Agentinc, tool};

/// Get the current weather for a city.
#[tool]
async fn get_weather(city: String) -> anyhow::Result<String> {
    Ok(format!("{city}: 22°C, sunny"))
}

let agentinc = Agentinc::local().await?;
let agent = Agent::builder("weather-bot")
    .model(model)
    .tool(get_weather)
    .build();

let answer = agentinc.start(&agent, "Weather in Lisbon?").await?.result().await?;
```

For a conversation that outlives a single turn, use a `Session`: send messages,
read the transcript, keep going.

## Testing

Tests never call a real provider. `agentinc::testing` gives you `ScriptedModel`
for deterministic replies, `testing::run()` to execute an agent end to end, and
`assert_transcript()` for ordered assertions.

```rust
let model = ScriptedModel::new().on_user("hello", text("Hi there."));
let agent = Agent::builder("greeter").model(model).build();

let run = agentinc::testing::run(&agent, "hello").await?;

run.assert_transcript().user("hello").assistant_contains("Hi there.").end();
```

## Crates

- `agentinc` — the SDK
- `agentinc-macros` — the `#[tool]` attribute, re-exported from `agentinc`
- `agentinc-os` — a personal life-OS app, and the SDK's running example

## Status

Early. Sessions and tools work; approvals, cancellation, streaming, structured
output and MCP tools are planned. See `AGENTS.md` for the design rules.

## License

MIT
