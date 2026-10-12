# Daemon ownership

`aincd` owns product writes. The Mac app renders owned snapshots from the generated `ainc-client`; it never waits for SQL on the foreground.

## Commands and receipts

`GET /v1/todos`, `GET /v1/calendar` and `GET /v1/workspaces` return whole snapshots for the selected Workspace. `POST /v1/<resource>/commands` accepts a UUID operation ID and a closed command enum. A transaction commits the product change and its receipt before returning. Repeating an operation ID returns the same record; changing its payload returns a conflict. Receipts are scoped per Workspace for Todos and Events. The app keeps an uncertain operation ID for reconciliation instead of silently issuing another mutation; a failed refresh cannot be mistaken for an empty successful read.

Only shell layout, navigation, theme and font remain app-local, in a versioned UI-state file. An unreadable or future-version file is displayed as unavailable and is not overwritten.

## Local companion and configuration

The bundle includes and signs `Contents/MacOS/aincd`. The app checks the discovered daemon on a background executor. If it is unavailable, the app starts that companion; the child survives app quit. The daemon locks its discovery file, binds an ephemeral loopback port and atomically publishes the URL. It creates an owner-only `owner-token` next to that file. Without `DATABASE_URL` it starts the bundled Postgres ([distribution.md](distribution.md)).

- `AINC_DISCOVERY_FILE`: URL discovery file; installed default is `…/Agentinc OS/daemon/api-url`.
- `AINC_API_URL`: explicitly configured HTTP(S) URL; disables companion launch.
- `AINC_TOKEN_FILE`: bearer credential file; defaults beside discovery.
- `DATABASE_URL`: an external Postgres for the daemon; `AINC_DATABASE_URL` on the app is passed to the companion as `DATABASE_URL`.
- `AINC_SESSION_PATH`: UI-only state; set it for every isolated app launch.

Product endpoints require the owner bearer credential. The generated CLI reads `AINC_TOKEN_FILE` or `.local/dev/owner-token`, with its `AINC_API_URL` override. The daemon binds only loopback.

For development:

```sh
cargo xtask dev
# In another terminal, using the same worktree:
AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" \
  crates/ainc-mac/scripts/pilot.sh "$PWD/.local/pilot"
```

## Checks

`crates/ainc-daemon/tests/api.rs` and the module tests cover real Postgres command receipts, Workspace scoping and authorization. `ainc-client/tests/round_trip.rs` and `ainc-cli/tests/round_trip.rs` check the generated clients over real HTTP. `just test` runs all of them.
