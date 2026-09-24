# Phase 3: Ticket command boundary

`GET /v1/tickets` and `POST /v1/tickets/commands` are generated-client/CLI operations.
The domain implementation is `crates/ainc-daemon/src/tickets.rs`; the checked-in
OpenAPI contract is generated from it. Tickets have Backlog, To do, In progress
and Done status, one human/agent assignee, a revision, and an assignment generation.
Comments are their durable work log. Agents are created at runtime with an immutable
model/instruction snapshot captured for each assignment.

The Ticket migration renames the phase-2 Postgres table in place, keeps IDs/titles,
and maps incomplete/completed rows to To do/Done. SQLite import writes the new
schema. Phase-2 DTOs remain a compatibility adapter until the native Ticket UI lands;
they do not create a second authoritative store. Legacy completion only operates on
human-owned Tickets, so the old UI cannot complete an agent's live assignment.

Commands commit their receipt, domain mutation and outbox intent together. UUID
operation IDs are scoped to workspace and actor; a replay returns its original
receipt and a changed payload conflicts. Updates check revisions under a row lock.
Assignment and actionable status changes snapshot a new run and queue its stable
ID; cancellation/reassignment queue cancellation of the previous generation.
A cancelled Ticket remains in the four-status model, and run state explains why
work stopped. Assigning again is an explicit new attempt. Backlog/Done never dispatch.

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

The dispatch consumer and coding-tool execution are the next slice. Queued intent
is durable but this domain slice alone does not execute an assigned Ticket.
