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
- `ainc-daemon`, `ainc-client`, `ainc-cli` — product API, generated Rust client and CLI
- `ainc-mac` — the imported native AgentInc GPUI app (see [its README](crates/ainc-mac/README.md))
- `gpui-pilot`, `gpui-pilot-cli` — opt-in native UI automation

## Development environment

Install Docker, Tilt, the Temporal CLI, PostgreSQL's `psql`, and Rust. Run `cargo xtask dev` in a worktree to start its isolated Postgres, Temporal, UI and native `aincd`. `cargo xtask doctor` prints its identity and discovered API URL; `cargo xtask down` stops only that worktree's stack and keeps its database volume. `cargo xtask generate` refreshes the checked-in OpenAPI 3.0.3 export and Progenitor client/CLI; `cargo xtask generate --check` checks drift.

After `cargo xtask dev`, use `cargo run -p ainc-cli -- health_ready`, `get_version`, or `ticket_contract --title example`. The Ticket operation is a phase 1 wire contract; Ticket persistence and commands come later. For a disposable two-worktree isolation/restart check, run `dev/check-isolation.sh WORKTREE_A WORKTREE_B`; it stops those two stacks when finished and preserves their volumes.

Product storage, the one-way legacy import and the bundled companion are documented in [phase 2 ownership](docs/phase-2-ownership.md). Product API operations require the daemon's owner credential; the development CLI reads `.local/dev/owner-token` automatically. For `cargo test --workspace`, set `DATABASE_URL` to a disposable Postgres admin database; SQLx creates and removes separate test databases. CI provides that service. SDK-only tests remain `cargo test -p turnkeel`.

## Status

Early. Sessions and tools work; approvals, cancellation, streaming, structured
output and MCP tools are planned. See `AGENTS.md` for the design rules.

## License

MIT
