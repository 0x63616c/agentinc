# How to add things

Recipes against the code as it is today. Each ends with a "Planned" line where
[the cohesion plan](cohesion-plan.md) changes the shape; follow the plan once that
work package has landed. Vocabulary is [CONTEXT.md](../CONTEXT.md).

## Add an endpoint

1. Put the handler in the daemon module that owns the resource (`crates/ainc-daemon/src/<resource>.rs`,
   for example `tickets.rs`, `automations.rs`, `workspaces.rs`). Annotate it with
   `#[utoipa::path(...)]` carrying the method, path, `operation_id = "{resource}_{action}"`,
   request body and every response, each error status as `ErrorBody`.
2. Return `Result<Json<T>, ApiError>`; errors are `ApiError::new(status, code, message)` from
   `product.rs`. Write mutations as a command on `POST /v1/<resource>/commands` with a UUID
   `operation_id` and a receipt table, like the existing resources, so replays return the
   original receipt.
3. Add `.route(...)` in the module's `router()` and the handler to the `paths(...)` list of the
   `#[openapi]` derive in `lib.rs` (`product_router()` merges the module routers).
4. Run `cargo xtask generate`: it rewrites `api/openapi-3.0.json`, `crates/ainc-client/src/generated.rs`
   and `crates/ainc-cli/src/generated.rs`. CI's `generated_api_and_clients_are_current` fails if
   you forget.
5. Add a round trip to `crates/ainc-client/tests/round_trip.rs` using the generated client against
   `ainc_daemon::product_router` under `#[sqlx::test(migrations = "../ainc-daemon/migrations")]`.
6. Tool parity: anything a person can do in the UI an agent must be able to do. A new `TicketCommand`
   variant reaches Evee automatically, because `conversation_tools.rs` derives the `ticket_command`
   schema from the OpenAPI `TicketCommand` schema; a new resource needs its own `turnkeel::Tool`
   impl there, and a CLI group (see "Add a CLI command").

Planned: one endpoint registry and one `app()` (D2), `CommandError` with a code enum (S4),
resource paths instead of `/v1/state` and `/v1/commands` (S11).

## Add a page

1. Add the variant to `Route` in `crates/ainc-mac/src/model.rs` and a `PageSpec` (route, title,
   icon, `in_sidebar`) to `PAGES` in the same file. Sidebar order, `⌘1…` numbering and the ⌘K
   palette all read `PAGES`; bump the `PAGES.len()` assertion in its tests.
2. Build the page as its own view in `crates/ainc-mac/src/<page>.rs` from `ui::` components
   (see [design-system.md](design-system.md)); render inside `Page::document` or `Page::canvas`.
3. Wire it in `shell.rs`: hold the view on `Shell`, construct it in `Shell::new`, and add the
   `Route::<Page> => ...` arm to the content `match` in `render`.
4. Add a `suite.capture("<page>", Route::<Page>, ...)` frame to `crates/ainc-mac/tests/rendered/runner.rs`
   and run `cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell`
   on macOS; `routes_and_history_use_shell_actions` in `shell.rs` covers navigation.

Planned: a `Page` trait with `Page::title()` from `PAGES` (M3, S20); page dialogs leave the shell.

## Add a component

Follow "Adding a component" in [design-system.md](design-system.md): `src/ui/`, tokens only,
a `debug_selector`, a row in the components table, a section on the Components page
(`crates/ainc-mac/src/components.rs`) and a rendered-shell frame. `cargo xtask check-ui`
rejects color literals outside `ui/tokens.rs`.

## Add a tool

1. In the SDK, a tool is an async fn with `#[tool]` (doc comment becomes the description) or a
   type implementing `turnkeel::Tool`. Mark tools that must not repeat
   `#[tool(idempotent = false)]`; the engine gives those one attempt. Use the `ToolCtx`
   idempotency key to make external effects safe to retry. `Runtime::test()` calls every
   idempotent tool twice and fails the run if results differ.
2. In the daemon, Ticket agents get `CodingTool` per `coding::Permission` (`ReadFile`, `WriteFile`,
   `Shell`, `Git`) plus `CommentTool`, assembled in `execution.rs`; Evee gets `list_tickets` and
   `ticket_command` from `conversation_tools.rs`. A new coding capability is a `Permission`
   variant and a `CodingTool` arm; a new Evee capability is a `Tool` impl calling the same
   command handler the UI uses.
3. Policy: `WorkspacePolicy` confines effects to `AINC_WORKSPACE_DIR`; `AINC_TOOL_ALLOW` lists the
   permitted tool names and absent configuration allows none. Every effect first commits an
   intent receipt and a Comment. Read [execution.md](execution.md) before adding an effectful
   tool, and cover it with a scripted-model test (`turnkeel::testing`).

Planned: `#[tool]` names move to `turnkeel.*` (S8); one shared daemon test fixture (S18).

## Add a migration

1. Create `crates/ainc-daemon/migrations/YYYYMMDDHHMMSS_name.sql`. `sqlx::migrate!()` embeds the
   directory, so the daemon and every `#[sqlx::test(migrations = ...)]` pick it up; there is no
   registry to edit.
2. Expand, then contract: add columns, tables and constraints a running older daemon tolerates;
   drop or rename in a later release, inside the compatibility window ([distribution.md](distribution.md)).
3. Naming: a lifecycle column is `status`, never `state`; times are epoch-second `bigint` columns
   named `*_at` (`created_at`, `updated_at`) with `extract(epoch FROM clock_timestamp())::bigint`
   for event times; never format times in SQL. User-visible records use `bigint` IDs; execution
   records use text UUIDs. Add a Postgres test that exercises the new shape through the command
   handler, not raw SQL.

Planned: `query_as!` with offline data and `cargo sqlx prepare --check` (S12); `state` columns
migrate to `status` on contact (S15).

## Add a CLI command

The CLI is generated; you add an operation, not a command.

1. Name the endpoint's `operation_id` `{resource}_{action}` (`tickets_command`, `automations_state`).
   `ainc` splits it at the first `_` into `ainc {resource} {action}`, mapping `state` to `list`;
   the few exceptions are the `operation_group` match in `crates/ainc-cli/src/main.rs`.
2. For a `.../commands` endpoint whose body is a `kind`-tagged enum, add the resource to the
   `variants()` map in `main.rs` so each variant becomes a subcommand with schema-derived flags
   (`ainc tickets create --title ...`); `--json-body FILE` always works.
3. Run `cargo xtask generate`, then extend `crates/ainc-cli/tests/round_trip.rs`, which runs the
   built `ainc` binary against a real daemon.

Planned: operation IDs become an `OperationId` type shared with receipts (D1).

## Cut a release

1. On a clean `main`, run `just release patch|minor|major`. It runs `cargo xtask bump`: bumps
   `[workspace.package].version` in the root `Cargo.toml`, runs `cargo update --workspace` and
   `cargo xtask generate`, and commits `chore(release): release X.Y.Z`. A dirty tree refuses.
2. Push that commit to `main`. The `Distribution` workflow (`.github/workflows/release.yml`) starts
   only on a product version change: the Mac runner builds and tests, Ubuntu signs, notarizes
   and staples, the [native upgrade gate](upgrade-gate.md) runs, then the release and
    legacy `feed.json` and Sparkle `appcast.xml` publish. Sparkle deltas are generated
    on the Mac from published signed bundles, round-trip verified, and signed on
    Linux. Nothing else needs doing; watch the run.
3. Release notes come from commit subjects since the last published version; a nonempty
   `docs/releases/X.Y.Z.md` replaces them. Runner and fallback details:
   [release-runner.md](release-runner.md); signing, channels and the updater:
   [distribution.md](distribution.md). The SDK crates version independently.

Planned: the release build stops watching git refs and `ainc-release` splits (B2); CI calls
`cargo xtask check` / `cargo xtask test` (S22).
