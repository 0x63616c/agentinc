# Phase 3: durable runtime foundation

`Runtime::configured(RuntimeConfig { endpoint, scope, worker_group }, &agents)` installs
agent definitions before polling and reuses the same deployment identity across processes.
Keep all three values stable across worker replacement. Every worker in a group must
register the same definitions. Use versioned agent names and keep old definitions installed
until their sessions and runs have finished. `local`, `test` and the original `connect`
remain isolated convenience runtimes; `connect` alone does not promise restart recovery.

Persist a `RunId` with the immutable command payload before calling `start_with_id`.
Repeating that ID attaches to the existing run, including after completion, while its
history is retained. The daemon must validate command receipts and retain effect receipts
beyond execution-history expiry. An existing ID does not validate a newly supplied prompt:
that binding belongs to the caller's command transaction. `run_by_id` retrieves its result
without redispatch. Session IDs already support attachment across processes.

`Run::cancel` and `Session::cancel` request cancellation. They do not undo external effects
or establish an authorization fence; Ticket assignment generations and tool permission
checks must enforce that product boundary. A pending external effect still needs an
idempotency receipt or explicit unknown-outcome reconciliation.

The real-server checks live in `crates/turnkeel/tests/recovery.rs`:

- A worker completes a session turn, is killed, and a second message is accepted while
  its group is offline. A newly spawned worker replays and completes the second turn.
- A gated model call accepts duplicate starts before and after completion without a
  second call. Its recorded history passes the replay engine with no provider/tools.
- A gated run is cancelled and returns `Error::Cancelled`.

Run `cargo test --locked -p turnkeel -p turnkeel-macros` and
`cargo clippy --locked -p turnkeel -p turnkeel-macros --all-targets -- -D warnings`.
Tests use only scripted models, no subscription credentials. The activity-crash and live-event follow-up is documented in [phase-3-provider.md](phase-3-provider.md).
This foundation does not claim durable product outbox dispatch, Ticket fencing,
provider compatibility, bounded session history, or native UI acceptance; those remain
phase 3 delivery gates.
