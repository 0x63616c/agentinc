# Phase 3 native acceptance

Tickets replace Tasks in navigation, search, saved routes and visible copy. The old
`tasks` preference value remains a read alias. The page uses generated Ticket
commands with operation receipts and revisions, and refreshes its owned snapshot in
the background. Existing phase-2 clients retain their wire compatibility.

The UI supports Backlog / To do / In progress / Done, human or agent assignment,
Comments, cancellation of live work, and a minimal agent registry. Today shows actual
open Tickets, prioritizes In progress, and opens their detail. Read failures display
Tickets unavailable; loading and a successful empty read are distinct states.

The detail page retains the shell's spacing, solid surfaces and hover transitions.
Saving restores a stable page focus so keyboard navigation/search continue to work
after controls become disabled. Comments show their author and retained body; SDK
output and tool evidence arrive through the same snapshot as human Comments.

## Checks on this Mac

- Full workspace tests and strict Clippy pass, with disposable SQLx databases and
  scripted/loopback inference. Backend evidence is in [phase-3-execution.md](../../../../docs/phase-3-execution.md).
- The 38-frame real Metal suite covers navigation, two window sizes, dialogs,
  search and the Evee panel, including negative capture-integrity controls.
- `pilot_acceptance` drives the actual app through gpui-pilot: search, Unicode Ticket
  creation, all four statuses, Comments, agent registration, Backlog assignment,
  reassignment to the human, and opening the Ticket from Today. Backlog assignment
  deliberately avoids dispatching a live model. A second isolated launch verifies
  the unavailable state using a refused loopback connection.
- Automation-feature strict Clippy checks the driver acceptance code too.
- CLI smoke verifies native WindowServer identity at 1360 × 828 and normal driver
  endpoint cleanup on quit. The signed debug bundle includes the companion daemon;
  `codesign --verify --deep --strict` passes, its normal executable rejects automation
  flags, and LaunchServices opened its own 1360 × 828 native window with isolated state.

Native UI acceptance uses the worktree's own Postgres/Temporal stack and a fresh
synthetic database, separate discovery, runtime worker identity, Codex profile and UI
session. No regular Application Support data is imported. No subscription call is
made. Process recovery and sandboxed effects are independently proven by daemon/SDK
tests; these screenshots alone do not claim that evidence.

## Captures

Saved at 1360 × 828 logical pixels from gpui-pilot's real Metal window capture:

- [Ticket and Comments](phase3/ticket-comments.png)
- [Assignee and status](phase3/ticket-assignee.png)
- [Agent registry](phase3/agents.png)
- [Today with real Tickets](phase3/today-tickets.png)
- [Today unavailable](phase3/today-unavailable.png)

Commands from the repository root:

```sh
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --test-threads=1 --nocapture
cargo clippy --locked -p agentinc-os --features automation --all-targets -- -D warnings
```
