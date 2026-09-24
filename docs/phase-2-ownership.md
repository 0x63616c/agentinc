# Phase 2: daemon ownership

This is the phase-2 migration record. Current SDK execution, runtime configuration,
Tickets and crash recovery are documented in [phase-3-execution.md](phase-3-execution.md).

`aincd` owns product writes and Codex processes. The Mac app renders owned snapshots from the generated `ainc-client`; it has no SQLite dependency and never waits for SQL or a provider process on the foreground. The existing text-only Codex protocol, ephemeral threads, bounded replies, sign-in flow and retry context are preserved in `crates/ainc-daemon/src/codex.rs`.

## Acknowledgement and execution

`GET /v1/state` returns Conversations, ordered turns, Tasks and typed connection preferences. `POST /v1/commands` accepts a UUID operation ID and a closed command enum. A transaction commits the product change and receipt before returning its acknowledgement. Repeating an operation ID returns the same record; changing its payload returns a conflict. The app retains an uncertain operation ID for reconciliation instead of silently issuing another mutation. Pending UI changes are disabled and labelled; a failed refresh cannot be mistaken for an empty successful read.

Queued turns are Postgres records. The daemon claims and executes them independently of HTTP connections and native windows. One pending turn per Conversation is enforced in Postgres; different Conversations can progress concurrently. A session advisory lock prevents a second daemon from recovering a live runner's work. Completed results are retained and saved again on database failure without calling Codex again. Deleting a Conversation with accepted work is refused.

This phase preserves the current text runner. On daemon restart queued work resumes and an interrupted running turn becomes visibly retryable with an unknown-outcome message. It does not silently replay a provider call. SDK session orchestration, cancellation and stronger process-crash recovery remain phase 3; there are no new public SDK or Temporal concepts in this change.

## Legacy import

Before publishing readiness, the daemon runs the SQLx migrations and imports `assistant.sqlite3` and `session.json` from `AINC_LEGACY_DIR`. The installed default is `~/Library/Application Support/Agentinc OS/`. `cargo xtask dev` always overrides that with this worktree's `.local/dev/legacy`, and gives Codex `.local/dev/codex`. Development never imports the regular profile automatically.

SQLite is opened read-only and read inside a consistent transaction, including committed WAL content. The destination transaction imports schema 0/1 single-chat history or schema 2 Conversations, turn IDs/order, Tasks, assistant settings and the original session JSON. Interrupted turns become failed/retryable; import never dispatches a model call. A committed source receipt prevents reimport, including resurrection of later-deleted records. Future SQLite schemas, invalid session JSON and a populated destination without an import receipt are refused. Failure rolls back destination rows; source data is not migrated in place or deleted. Tests use disposable synthetic copies of the original schemas, never the regular Application Support directory.

Only shell layout, navigation and font remain app-local. Their versioned UI-preference DTO preserves the saved Tasks/Evee aliases and older tab histories. An unreadable or future-version file is displayed as unavailable and is not overwritten. Product preferences (selected model and Conversation) are daemon-owned.

## Local companion and remote configuration

The bundle includes and signs `Contents/MacOS/aincd`. The app checks the discovered daemon on a background executor. If it is unavailable, the app starts that companion using `AINC_DATABASE_URL` (a Postgres URL); the child survives app quit. The daemon locks its discovery file, binds an ephemeral loopback port and atomically publishes the URL. It creates an owner-only `owner-token` next to that file. Postgres remains an independently managed dependency; this phase does not bundle a database server.

- `AINC_DISCOVERY_FILE`: URL discovery file; installed default is `…/Agentinc OS/daemon/api-url`.
- `AINC_DAEMON_URL`: explicitly configured HTTP(S) URL; disables companion launch.
- `AINC_TOKEN_FILE`: bearer credential file; defaults beside discovery.
- `AINC_LEGACY_DIR`, `AGENTINC_CODEX_HOME`, `AGENTINC_CODEX_PATH`: daemon-side import/profile/provider overrides. The app does not access provider credentials.
- `AGENTINC_SESSION_PATH`: UI-only preferences; set it for every isolated app launch.

Product endpoints require the owner bearer credential. The generated CLI reads `AINC_TOKEN_FILE` or `.local/dev/owner-token`, with its existing `AINC_API_URL` override. The daemon still binds only loopback; remote exposure requires the private TLS/Tailscale deployment configuration from the architecture plan. Multi-user identity is phase 5.

For development:

```sh
cargo xtask dev
# In another terminal, using the same worktree:
AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" \
  crates/ainc-mac/scripts/pilot.sh "$PWD/.local/phase2-pilot"
```

The shared OverlayHost, Route/Page catalogue and design tokens were already imported. This phase preserves them and finishes the remaining persistence/unavailable-state work rather than rebuilding those components.

## Checks

`crates/ainc-daemon/tests/product.rs` tests real Postgres command receipts, constraints, read authorization and atomic import. `tests/process.rs` starts the real daemon with a gated Codex protocol fixture, closes its HTTP client while a reply is pending and proves that the completed reply survives. `ainc-client/tests/round_trip.rs` checks the generated enum command and nullable snapshot over real HTTP.

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
# DATABASE_URL must point to a disposable Postgres admin database (SQLx creates test databases).
cargo test --locked --workspace
cargo xtask generate --check
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" \
  cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --nocapture
```

Native acceptance is recorded in `crates/ainc-mac/docs/verification/OWNERSHIP.md`. Live subscription calls are opt-in; the default tests never use a real account or paid model.
