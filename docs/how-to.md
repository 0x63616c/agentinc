# How to add things

Recipes against the code as it is today. Vocabulary is [CONTEXT.md](../CONTEXT.md).

## Add an endpoint

1. Put the handler in the daemon module that owns the resource (`crates/ainc-daemon/src/<resource>.rs`,
   for example `todos.rs`, `calendar.rs`, `workspaces.rs`). Annotate it with
   `#[utoipa::path(...)]` carrying the method, path, `operation_id = "{resource}_{action}"`,
   request body and every response, each error status as `ErrorBody`.
2. Return `Result<Json<T>, ApiError>`; errors are `ApiError::new(status, code, message)` from
   `api.rs`. Write mutations as a command on `POST /v1/<resource>/commands` with a UUID
   `operation_id` and a receipt scope (`receipts.rs`), like the existing resources, so replays
   return the original receipt.
3. Add `.route(...)` in the module's `router()`, merge it in `api::routes()`, and add the handler
   to the `paths(...)` list of the `#[openapi]` derive in `lib.rs`.
4. Run `cargo xtask generate`: it rewrites `api/openapi-3.0.json`, `crates/ainc-client/src/generated.rs`
   and `crates/ainc-cli/src/generated.rs`. CI's `generated_api_and_clients_are_current` fails if
   you forget.
5. Add a round trip to `crates/ainc-client/tests/round_trip.rs` using the generated client against
   `ainc_daemon::app` under `#[sqlx::test(migrations = "../ainc-daemon/migrations")]`.
6. Parity: anything a person can do in the UI must be possible through the API, so the CLI (see
   "Add a CLI command") and any later agent tool reach it too.

## Add a page

1. Add the variant to `Route` in `crates/ainc-mac/src/routes.rs` and a `PageSpec` (route, title,
   icon, `in_sidebar`) to `PAGES` in the same file. Sidebar order, `⌘1…` numbering and the ⌘K
   palette all read `PAGES`; bump the `PAGES.len()` assertion in its tests.
2. Build the page as its own view in `crates/ainc-mac/src/<page>.rs` from `ui::` components
   (see [design-system.md](design-system.md)) implementing `page::Page`; render inside
   `PageFrame::document`, `fill` or `canvas`.
3. Wire it in `shell.rs`: add a `PageHandle` for it to the `pages` list in `Shell::new`.
4. Add a `suite.capture("<page>", Route::<Page>, ...)` frame to `crates/ainc-mac/tests/rendered/runner.rs`
   and run `cargo test --locked -p ainc-mac --features rendered-tests --test rendered_shell`
   on macOS; `routes_and_history_use_shell_actions` in `shell.rs` covers navigation.

## Add a component

Follow "Adding a component" in [design-system.md](design-system.md): `src/ui/`, tokens only,
a `debug_selector`, a row in the components table, a section on the Components page
(`crates/ainc-mac/src/components.rs`) and a rendered-shell frame. `cargo xtask check-ui`
rejects color literals outside `ui/tokens.rs`.

## Add a tool

In the SDK, a tool is an async fn with `#[tool]` (doc comment becomes the description) or a
type implementing `turnkeel::Tool`. Mark tools that must not repeat
`#[tool(idempotent = false)]`; the engine gives those one attempt. Use the `ToolCtx`
idempotency key to make external effects safe to retry. `Runtime::test()` calls every
idempotent tool twice and fails the run if results differ. The product has no agent tools yet;
when they return, a tool calls the same command handler the UI uses.

## Add a migration

1. Create `crates/ainc-daemon/migrations/YYYYMMDDHHMMSS_name.sql`. `sqlx::migrate!()` embeds the
   directory, so the daemon and every `#[sqlx::test(migrations = ...)]` pick it up; there is no
   registry to edit.
2. Expand, then contract: add columns, tables and constraints a running older daemon tolerates;
   drop or rename in a later release, inside the compatibility window ([distribution.md](distribution.md)).
3. Naming: a lifecycle column is `status`, never `state`; times are epoch-second `bigint` columns
   named `*_at` (`created_at`, `updated_at`) with `extract(epoch FROM clock_timestamp())::bigint`
   for event times; never format times in SQL. User-visible records use `bigint` IDs. Add a
   Postgres test that exercises the new shape through the command handler, not raw SQL.
4. Daemon SQL is `sqlx::query!`/`query_as!`/`query_scalar!`, checked against the migrations at
   compile time from `.sqlx/` (builds set `SQLX_OFFLINE`; no database needed). After changing a
   query or a migration, run `cargo xtask prepare-sqlx` (needs `DATABASE_URL` and
   `cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres`)
   and commit `.sqlx/`. `cargo xtask check` and `test` fail on a stale `.sqlx/`, and `check`
   rejects runtime `sqlx::query(` in production code.

## Add a CLI command

The CLI is generated; you add an operation, not a command.

1. Name the endpoint's `operation_id` `{resource}_{action}` (`todos_command`, `calendar_state`).
   `ainc` splits it at the first `_` into `ainc {resource} {action}`, mapping `state` to `list`;
   the few exceptions are the `operation_group` match in `crates/ainc-cli/src/main.rs`.
2. For a `.../commands` endpoint whose body is a `kind`-tagged enum, add the resource to the
   `variants()` map in `main.rs` so each variant becomes a subcommand with schema-derived flags
   (`ainc todos add --title ...`); `--json-body FILE` always works.
3. Run `cargo xtask generate`, then extend `crates/ainc-cli/tests/round_trip.rs`, which runs the
   built `ainc` binary against a real daemon.

## Cut a release

1. On a clean `main`, run `just release patch|minor|major`. It runs `cargo xtask bump`: bumps
   `[workspace.package].version` in the root `Cargo.toml`, runs `cargo update --workspace` and
   `cargo xtask generate`, and commits `chore(release): release X.Y.Z`. A dirty tree refuses.
2. Push that commit to `main`. The `Distribution` workflow (`.github/workflows/release.yml`) runs
   on every push to `main` until that version is published, so a failed run is retried by the next
   push (an unpublished draft left by a failed run is replaced): the Mac runner builds and tests, Ubuntu signs, notarizes
   and staples, the [native upgrade gate](upgrade-gate.md) runs, then the release and
   legacy `feed.json` and Sparkle `appcast.xml` publish. Sparkle deltas are generated
   on the Mac from published signed bundles, round-trip verified, and signed on
   Linux. Nothing else needs doing; watch the run.
3. Release notes come from commit subjects since the last published version; a nonempty
   `docs/releases/X.Y.Z.md` replaces them. Runner and fallback details:
   [release-runner.md](release-runner.md); signing, channels and the updater:
   [distribution.md](distribution.md). The SDK crates version independently.
