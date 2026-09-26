# Phase 3: Ticket command boundary

`GET /v1/tickets` and `POST /v1/tickets/commands` are generated-client/CLI operations.
The domain implementation is `crates/ainc-daemon/src/tickets.rs`; the checked-in
OpenAPI contract is generated from it. Tickets have one of six statuses (see the board section below), one human/agent
assignee, a revision, and an assignment generation.
Comments are their durable work log. Agents are created at runtime with an immutable
model/instruction snapshot captured for each assignment.

The Ticket migration renames the phase-2 Postgres table in place, keeps IDs/titles,
and maps incomplete/completed rows to To do/Done. SQLite import writes the new
schema. Phase-2 DTOs remain a compatibility adapter for older clients;
they do not create a second authoritative store. Legacy completion only operates on
human-owned Tickets, so the old UI cannot complete an agent's live assignment.

Commands commit their receipt, domain mutation and outbox intent together. UUID
operation IDs are scoped to workspace and actor; a replay returns its original
receipt and a changed payload conflicts. Updates check revisions under a row lock.
Assignment and actionable status changes snapshot a new run and queue its stable
ID; cancellation/reassignment queue cancellation of the previous generation.
Cancelling work (the `cancel` command) leaves the Ticket's status alone, and run
state explains why work stopped. Assigning again is an explicit new attempt. Backlog/Done never dispatch.

The owner token is confined to the local workspace. Agent bearer credentials are
stored as SHA-256 hashes and restricted to a run's Ticket/generation. Agent reads
show only their assigned Ticket; Comments and permitted completion commands pass
the same domain boundary as owner commands. Authorization is checked again inside
the transaction, including receipt reads, so stale credentials cannot write after
cancellation/reassignment. Agents cannot register another agent or reassign Tickets.

Real Postgres tests cover in-place migration, repeat/concurrent commands, payload
and revision conflicts, rollback of invalid assignment, outbox atomicity, cross-workspace
read/write denial, agent scope, stale credentials, cancellation and redispatch. The
existing legacy import and native-client compatibility tests remain green. A generated
client submits a Comment and reads the migrated Ticket over HTTP.

The outbox consumer, coding policy and durable Conversation integration are documented
in [phase-3-execution.md](phase-3-execution.md).

## The 1.0 board

The board migration (`20260926000000_ticket_board.sql`) adds Blocked and Cancelled
statuses, a priority (urgent, high, medium, low, none), a description, up to ten
labels, a position inside each status column, created and updated times, and the
Conversation a Ticket came from. Only To do and In progress dispatch an agent; any
other status stops live work, exactly as Backlog and Done did.

`move` puts a Ticket into a column directly below a neighbour (or first) and
renumbers that column, under one board lock per workspace. Reordering is not an
edit: it keeps the revision. A neighbour that left the column is a stale view and
conflicts. New Tickets and status changes land at the top of their new column,
including the executor's To do → In progress → Done moves.

Relationships (`ticket_links`, in `src/tickets/links.rs`) are stored once from their
source: blocks, relates to (stored lower ID first), duplicates and parent of. The
reverse readings are blocked by, duplicated by and sub-issue. Loops and a second
parent are refused, and both Tickets must be in the actor's workspace.

History (`ticket_activity`, in `src/tickets/activity.rs`) commits with each change:
created, renamed, described, status, priority, assigned, labels, linked/unlinked on
both sides, and run lifecycle (queued, completed, failed, cancelled) with its run ID.
Changes made through Evee carry the Conversation's ID. History is served per Ticket
from `GET /v1/tickets/{id}/activity`, so the polled snapshot and Evee's
`list_tickets` result stay small; relationships are in the snapshot.
