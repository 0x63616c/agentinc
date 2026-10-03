# Durable agent execution

`aincd` now hosts both Conversation sessions and assigned Ticket runs on Turnkeel.
The official Codex client is used only for isolated ChatGPT sign-in, credential
refresh and model discovery. Model inference goes directly through our Responses
adapter in SDK activities. No Codex thread/turn execution remains.

## Configuration

`cargo xtask dev` / `cargo xtask serve` writes `.local/dev/runtime.json` with the
worktree's endpoint, scope and stable worker group, and gives the daemon an isolated
workspace at `.local/dev/workspace`. The development command explicitly permits
`read_file`, `write_file`, `shell` and `git` there.

Installed companions also need a `runtime.json` beside their discovery file, or
`AINC_RUNTIME_CONFIG` containing the same JSON. `AINC_WORKSPACE_DIR` selects the
coding directory. `AINC_TOOL_ALLOW` is a JSON list of the four tool names; absent
configuration allows none. Keep the runtime identity stable across process restarts.
The SDK's configuration remains free of Temporal vocabulary; deployment operators
still provision the underlying server and namespace.

## Execution and recovery

A Ticket assignment commits an immutable run definition and a start outbox entry
in the same transaction as the command receipt. Only To do / In progress work starts.
A stable SDK run ID reconciles a crash between dispatch and acknowledgement.
The run's result projects once as a Comment and successful completion sets Done.
Failures appear in the run record and Comments, without inventing a fifth status.
Each daemon owns separate Postgres session advisory locks for Ticket and Conversation
execution. Those connections are detached from the pool: closing the owner releases
the lock, rather than returning a still-locked connection to another request.

Turns are a durable message outbox. Sessions persist their model, initial history and
consumed event offset. `send_once` deduplicates a turn/attempt delivery; each event and
its projection commit together. Model changes and retries after terminal failure
create a fresh session seeded with completed dialogue. Replaced/deleted Conversations
leave durable cancellation intent. A lost worker retries interrupted model activities;
a database transport error retains pending work for recovery instead of marking it as
an agent failure.

Evee has `list_tickets` and `ticket_command`, using the authoritative command schema
and handler with the owner's local workspace capability. Assignment is the only path
from a Conversation to autonomous coding. Ticket agents can post Comments and complete
their own assignment; they cannot create work or change another Ticket. HTTP agent
credentials are tied to a run/generation, and every mutation checks that authority
again inside its transaction, including receipt replays.

## Coding effects and cancellation

The Mac worker uses the OS sandbox for every child, clears the environment, denies
network access and confines writable paths to the configured directory. System
executables/libraries can be read; profile credentials and adjacent repositories
cannot. Relative file paths reject traversal; the kernel also blocks escaping
symlinks. Git permits status, diff, log, add and commit. Shell permission deliberately
allows arbitrary commands inside the same sandbox. Linux workers fail closed for
coding tools until a supported OS sandbox is implemented; API, inference and SDK
recovery tests remain portable.

Every effect first commits its assignment check, intent receipt and a Started Comment.
Arbitrary process tools receive one SDK attempt. A completed receipt returns the saved
result for the same key; an interrupted receipt remains unknown and refuses blind
re-execution. This is at-most-once admission, not a claim of exactly-once filesystem
transactions. A model may inspect an unknown effect with a fresh read before proposing
new work. Comments contain tool output and final evidence; opaque provider reasoning
context is never projected there.

Cancel/reassign increments the Ticket generation and queues SDK cancellation. New
commands/effects from the old generation are refused, and old results cannot complete
the new assignment. Dropping a live tool kills its owned process group. An effect
already admitted before cancellation can have changed files; cancellation cannot
roll those changes back. Its retained receipt and work log make that boundary explicit.

## Evidence

Run against a disposable Postgres admin URL (SQLx creates separate test databases):

```sh
cargo test --locked -p turnkeel -p ainc-daemon -- --test-threads=1
cargo clippy --locked -p turnkeel -p ainc-daemon -p ainc-xtask --all-targets -- -D warnings
```

- SDK recovery tests kill real child workers, restore session history, retry an
  interrupted durable fixture effect under the same key and replay retained history.
- Daemon process tests kill the actual binary during a gated Responses request,
  restore Conversations and assigned Tickets, reconstruct a lost dispatch acknowledgement,
  and verify a single result Comment and successful replay.
- Ticket tests cover concurrent receipt deduplication, payload conflicts, transactional
  migration/import, workspace scope, generation fencing and cancel/reassign.
- Execution tests produce a real file in a disposable sandbox, inspect its Comments,
  refuse escaped symlinks and unknown effect outcomes, and cancel an open model call.
- Conversation tests exercise Evee's own Ticket tool, command receipts, model-switch
  history and deletion without resurrecting old authority.

All model tests are scripted or loopback HTTP fixtures. The debug fixture endpoint
accepts only exact fake credentials and a loopback address; release builds ignore it.
No isolated sign-in exists here, so a live subscription smoke is not run.

Unused Tickets retain the existing explicit Delete action through the scoped command
API. Once a Ticket has execution history, deletion is refused so run/effect evidence
remains inspectable; mark it Done instead. Legacy phase-2 wire names remain compatible
for older clients while the current native UI and domain use Tickets.
