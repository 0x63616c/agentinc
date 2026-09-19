# agentic

A Rust SDK for building AI agents. You define an agent (model, instructions, tools),
start a run, and get a result. Every run is durable: it survives crashes and restarts
and retries failed steps.

Durability comes from Temporal. That is an implementation detail and stays one.

## The one rule

**Nothing Temporal leaks into the public API.** No workflow, activity, signal, task queue,
or `temporalio_*` type is visible to a user. A user's code and tests read as if agentic
were an ordinary async library. If you find yourself exposing a Temporal concept, wrap it
in agent vocabulary first: `Run`, `Event`, `Session`, `Tool`, `Model`.

The test for this rule: `crates/agentic/src/engine/` is the only place `temporalio_*`
is imported. Keep it that way.

## Layout

- `crates/agentic` — the SDK. Public modules at `src/*.rs`. The private engine at
  `src/engine/`. Test helpers at `src/testing/`.
- `crates/agentic-macros` — the `#[tool]` attribute. Re-exported from `agentic`; users
  never depend on it directly.
- `examples/` — runnable apps, one crate each. Every public feature should appear in at
  least one example.

Expected to grow: `crates/agentic-<provider>` for model providers (Anthropic first),
more examples, and possibly `crates/agentic-core` if the engine ever needs to become its
own crate.

## How the engine works

One run is one workflow (`engine/workflow.rs`). The agent loop lives there and is the
only workflow code in the project. Model calls and tool calls are activities
(`engine/activities.rs`), which is the only place user code (models, tools) runs.

Workflow code must be deterministic: no I/O, no clocks, no randomness, no iteration over
`HashMap`. Anything that touches the outside world goes in an activity. Users never write
workflow code, so this rule only binds us.

Models and tools are Rust objects that cannot cross the wire. The workflow carries a
serializable `AgentSpec` (name, instructions, tool schemas); the worker resolves the real
objects from a registry keyed by agent name.

## Testing

Tests never call a real model provider. Use `agentic::testing`: `ScriptedModel` for
deterministic replies, `testing::run()` to execute an agent end to end,
`assert_transcript()` for ordered assertions. Tests run against a real local Temporal
dev server, so they exercise the real workflow and real activities.

Add a test for every behaviour change in the loop. The test should not mention Temporal;
if it has to, the public API has leaked.

Verify with `cargo test` and `cargo clippy --all-targets`. Both should be clean.

## Decisions already made

These were argued out and settled. Revisit with a reason, not by accident.

- **Start minimal, grow additively.** Sessions, approvals, cancellation, structured
  output, MCP tools, middleware are all planned but not built. Add them as new methods
  and variants, not by reshaping existing types.
- **Streaming will use Temporal's Workflow Streams protocol** (signals to publish,
  updates to long-poll, a query for the offset) so existing Python/TS subscribers can
  read agentic runs. The public surface will be `run.events()` returning a `Stream`.
- **Events should serialize to AG-UI** so existing agent frontends work unchanged.
- **Conversation size is handled with a claim-check payload codec**, added later and
  invisibly. Not by passing conversation references through the workflow.
- **Continue-as-new is deferred** and internal. `Agent` and workflow state must stay
  serializable so it can land without an API change.
- **Model providers are thin `reqwest` clients we own**, not community SDKs.
- **Tower for middleware was considered and parked.** Do not add hook callbacks in the
  meantime; that space is reserved.
