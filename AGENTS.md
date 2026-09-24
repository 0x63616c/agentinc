# Turnkeel

A Rust SDK for building AI agents. You define an agent (model, instructions, tools),
start a run, and get a result. Every run is durable: it survives crashes and restarts
and retries failed steps.

Durability comes from Temporal. That is an implementation detail and stays one.

## The one rule

**Nothing Temporal leaks into the public API.** No workflow, activity, signal, task queue,
or `temporalio_*` type is visible to a user. A user's code and tests read as if turnkeel
were an ordinary async library. If you find yourself exposing a Temporal concept, wrap it
in agent vocabulary first: `Run`, `Event`, `Session`, `Tool`, `Model`.

The test for this rule: `crates/turnkeel/src/engine/` is the only place `temporalio_*`
is imported. Keep it that way.

## How to build

Tracer bullets. Build the thinnest real end-to-end path that meets the ask, then let the
next concrete need (usually the next test) pull in the next piece. Do not add fields,
options, parameters, limits, or defaults nobody asked for; mention them in a line instead.
Past examples of getting this wrong: retry caps, `max_turns`, `run_id` on `ToolCtx`.

No sleeps or timers in tests. Coordinate on state or explicit gates.

## Layout

Flat: every crate lives directly under `crates/`. No nesting; related crates share a
prefix instead (`crates/ainc-tickets`, never `crates/ainc-mac/tickets`).
For the accepted product plan, see [docs/architecture.md](docs/architecture.md) and
[docs/adr/](docs/adr/). Crate names below describe the current source.

- `crates/turnkeel` — the SDK. Public modules at `src/*.rs`. The private engine at
  `src/engine/`. Test helpers at `src/testing/`.
- `crates/turnkeel-macros` — the `#[tool]` attribute. Re-exported from `turnkeel`; users
  never depend on it directly.
- `crates/ainc-mac` — the imported native GPUI AgentInc app. Build its signed bundle
  with `crates/ainc-mac/scripts/bundle.sh`; see its README for isolated validation.
- `crates/gpui-pilot`, `crates/gpui-pilot-cli` — opt-in app automation. The GPUI
  patch and regeneration instructions are in `vendor/` and
  `crates/ainc-mac/scripts/vendor-pilot-gpui.py`. Pilot launches are hidden by
  default; see `crates/ainc-mac/docs/GPUI_PILOT.md` for the visible OS-test opt-in.

Expected to grow: provider crates, and possibly a core crate if the engine ever
needs to become its own crate.

## Two names, two layers

- **SDK** (`turnkeel`) runs *one agent* durably. Boundary test: does this make
  sense with a single agent and no UI? If yes, it belongs in the SDK.
- **AgentInc** is a personal life-OS app; its native client is at `crates/ainc-mac`.
  The daemon owns product state; migration, isolated profile overrides and local
  companion setup are documented in `docs/phase-2-ownership.md`; current runtime and tool policy
  are in `docs/phase-3-execution.md`; scheduled Ticket grants, overlap/catch-up policy and
  phase-4 evidence are in `docs/phase-4-automations.md`. Rule: anything a human can do in
  its UI, an agent can do through the same tools.

There is no framework layer yet. Extract one from the OS later, once the generic parts
are obvious. Do not start it early.

OS constructs (settled, seven): Agents (data, created at runtime), Conversations (map to
SDK sessions), Tickets (shared human/agent, assignable to an agent, comments are the work
log), Knowledge (later), Inbox (human view over an event stream), Automations (trigger +
agent + prompt), Connections (external accounts contributing tools).

OS MVP: tickets, a minimal agents registry, and one automation: "ticket assigned to an
agent → run it".

## How the engine works

The agent loop is `turn()` in `engine/conversation.rs`, over a `Conversation` (agent
spec, history, pending messages). Two workflows call it: a run (`engine/workflow.rs`)
takes one turn and ends; a session (`engine/session.rs`) waits for a message, takes a
turn, and repeats forever. Messages sent mid-turn are seen at the next model call.
Model calls and tool calls are activities (`engine/activities.rs`), which is the only
place user code (models, tools) runs.

Workflow code must be deterministic: no I/O, no clocks, no randomness, no iteration over
`HashMap`. Anything that touches the outside world goes in an activity. Users never write
workflow code, so this rule only binds us.

Models and tools are Rust objects that cannot cross the wire. The workflow carries a
serializable `AgentSpec` (name, instructions, tool schemas); the worker resolves the real
objects from a registry keyed by agent name.

## Testing

Tests never call a real model provider. Use `turnkeel::testing`: `ScriptedModel` for
rule-based replies, `Script` for call-by-call control, `testing::run()` to execute an
agent end to end, `assert_transcript()` for ordered assertions. Tests run against a real local Temporal
dev server, so they exercise the real workflow and real activities.

`Runtime::test()` also calls every idempotent tool twice and fails the run if the results
differ. Tools that must not repeat are marked `#[tool(idempotent = false)]`; the engine
gives those exactly one attempt. Tools receive an idempotency key through `ToolCtx` so
they can make external side effects safe to retry.

For control over ordering and mid-turn behaviour use `testing::Script`: the test answers
every model call and tool call itself, so a turn stays open exactly as long as the test
wants. To wait for a session to finish a turn, read `session.events()` until
`Event::TurnEnded`.

Add a test for every behaviour change in the loop. The test should not mention Temporal;
if it has to, the public API has leaked.

Personal packaging, update signing and bundled runtime ownership: [docs/phase-5-distribution.md](docs/phase-5-distribution.md).
The production/development channel and isolation rule is also documented there;
`ainc-release::identity` is its single code source.

Stable runtime deployment and process-recovery checks: [docs/phase-3-runtime.md](docs/phase-3-runtime.md).

Verify with `cargo test` and `cargo clippy --all-targets`. Both should be clean.

## Committing

Commit proactively. Every coherent step that builds and passes tests gets its own
commit, without waiting to be asked. Small commits with a clear message beat one large
one at the end.

## Decisions already made

These were argued out and settled. Revisit with a reason, not by accident.

- **Start minimal, grow additively.** Sessions, approvals, cancellation, structured
  output, MCP tools, middleware are all planned but not built. Add them as new methods
  and variants, not by reshaping existing types.
- **Streaming will use Temporal's Workflow Streams protocol** (signals to publish,
  updates to long-poll, a query for the offset) so existing Python/TS subscribers can
  read turnkeel runs. The public surface will be `run.events()` returning a `Stream`.
- **Events should serialize to AG-UI** so existing agent frontends work unchanged.
- **Conversation size is handled with a claim-check payload codec**, added later and
  invisibly. Not by passing conversation references through the workflow.
- **Continue-as-new is deferred** and internal. `Agent` and workflow state must stay
  serializable so it can land without an API change.
- **Model providers are thin `reqwest` clients we own**, not community SDKs.
- **Tower for middleware was considered and parked.** Do not add hook callbacks in the
  meantime; that space is reserved.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
