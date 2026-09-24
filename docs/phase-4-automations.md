# Scheduled Automations

An Automation saves a bounded Ticket proposal (prompt and registered agent), an
interval in whole minutes, and its desired pause state. Saving grants recurring
creation and assignment within the owner's workspace. Assigned Ticket agents cannot
grant recurring work; Evee uses the same scoped command and receipt path as the UI.
The initial UI interval is 30 minutes.

Postgres owns rule revisions, command receipts and occurrence history. Turnkeel's
private engine owns real Temporal Schedules. A reconciler applies each definition
and the app shows Pending until its revision is acknowledged. Errors remain visible.
Rules are created paused before their catch-up policy is applied, preventing an
interrupted creation from firing under an unintended default.

Each occurrence calls the feature-owned `TicketProposal` command inside the same
transaction as its occurrence receipt. That command validates the registered agent,
creates the Ticket, and commits its assignment/run/outbox together. The occurrence
ID binds retries to one Ticket. The usual generation and effect receipts then protect
execution. No Home or Calendar effect framework is introduced.

## Overlap, missed work and availability

- Skip overlap. The scheduled occurrence stays active until its Ticket run is
  terminal. A row lock and active-run check also enforce this for manual Run now.
- Catch up only within ten seconds. Temporal's durable missed-window and overlap
  counters are projected as count-delta history entries, rather than inventing
  timestamps or Tickets for firings the server did not execute.
- Run now is deliberate replacement work, including while paused. It does not replay
  the backlog. It has a caller-owned occurrence ID and a retained command receipt,
  so a lost HTTP acknowledgement cannot create a second occurrence.
- Editing or pausing increments the grant revision. An occurrence not yet admitted
  rechecks it; old or paused firings are retained without creating work. A previously
  admitted Ticket continues and can be cancelled through its normal Ticket control.
- History includes queued occurrences and linked Tickets. A missing Ticket worker
  heartbeat is shown as worker unavailable; a disconnected app reports unavailable
  state and does not claim writes were accepted offline.

Runtime action groups are separate from ordinary agent groups. Definitions and
receipts survive daemon replacement. Scheduled occurrence activities heartbeat and
retry under the same ID; a one-day activity attempt renews through retry if the
Ticket remains open. Recurring callbacks are internal operational actions, not model
calls, and Temporal types remain confined to `crates/turnkeel/src/engine/`.

## Verification

Phase 4 acceptance is in progress. The PR tracks real-server recovery and effect
checks, complete workspace validation, Linux CI, the existing rendered suite and
native gpui-pilot acceptance. Do not treat an open PR as completion of these gates.
