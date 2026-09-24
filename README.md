# Turnkeel

**Write durable, observable and testable agents, powered by
[Temporal](https://temporal.io).**

A Rust SDK for building AI agents. You define an agent — model, instructions,
tools — start a run, and get a result. It's plain async Rust: no state machine,
no orchestration code. Runs survive crashes and restarts, failed steps are
retried, and every run keeps a full history you can read back.

```rust
use turnkeel::{Agent, Runtime, tool};

/// Get the current weather for a city.
#[tool]
async fn get_weather(city: String) -> anyhow::Result<String> {
    Ok(format!("{city}: 22°C, sunny"))
}

let turnkeel = Runtime::local().await?;
let agent = Agent::builder("weather-bot")
    .model(model)
    .tool(get_weather)
    .build();

let answer = turnkeel.start(&agent, "Weather in Lisbon?").await?.result().await?;
```

For a conversation that outlives a single turn, use a `Session`: send messages,
read the transcript, keep going.

## Testing

Tests never call a real provider. `turnkeel::testing` gives you `ScriptedModel`
for deterministic replies, `testing::run()` to execute an agent end to end, and
`assert_transcript()` for ordered assertions.

```rust
let model = ScriptedModel::new().on_user("hello", text("Hi there."));
let agent = Agent::builder("greeter").model(model).build();

let run = turnkeel::testing::run(&agent, "hello").await?;

run.assert_transcript().user("hello").assistant_contains("Hi there.").end();
```

## Crates

- `turnkeel` — the SDK
- `turnkeel-macros` — the `#[tool]` attribute, re-exported from `turnkeel`
- `ainc-mac` — the AgentInc app placeholder and the SDK's future running example

## Status

Early. Sessions and tools work; approvals, cancellation, streaming, structured
output and MCP tools are planned. See `AGENTS.md` for the design rules.

## License

MIT
