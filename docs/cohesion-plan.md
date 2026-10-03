# Cohesion plan

A complete, ordered plan to make the repo cohesive: one way to do each thing in the code, in the
UI, in the tooling. It was produced by a full review on 2026-10-02 (four parallel audits over
`crates/ainc-mac`, `crates/ainc-daemon` + `crates/turnkeel` + clients, build timings and tooling,
and cross-repo naming and copy). The decision summary with diagrams lives outside the repo; this
file is the authoritative, self-contained work list. An agent handed only this file has everything
it needs: the evidence (file:line as of commit `1c9740d`), the target shape, the order, and the
acceptance for each work package.

Vocabulary used: **module** (interface + implementation), **interface** (everything a caller must
know), **deep** (lots of behaviour behind a small interface), **shallow**, **seam** (where an
interface lives), **adapter** (a thing satisfying an interface at a seam), **leverage** (what
callers gain from depth), **locality** (what maintainers gain). Domain words come from
`CONTEXT.md`.

## Ground rules for executing this plan

- Work in Treehouse leases (`treehouse get --lease --json --lease-holder <holder>`), one work
  package per lease, commit per coherent step, `just test` clean before integration. The
  coordinator integrates and pushes.
- Every work package (WP) is additive where it touches a public interface: new methods and
  variants, never reshaped types, per AGENTS.md "Decisions already made".
- Do WP-B1 first. Everything else iterates ten times faster once the pre-commit gate is 4 s.
- Add a test for every behaviour change; tests never mention Temporal; no sleeps, no spins.
- Settled decisions that this plan does NOT reopen: Temporal as the engine, GPUI with our own
  components, daemon owns state, own agent loop, own updater, Tilt + Compose, Tower middleware
  parked. ADR-0004 holds: there is no second agent loop in the daemon (verified).

## Order of work

| Phase | Work packages | Why this order |
| --- | --- | --- |
| 0 Gate | B1, B2, B3, B4 | Measured 150 s → 4 s pre-commit; cold builds 2.3× faster; disk back |
| 1 Seams | M1 (Daemon module), D1 (receipts), D2 (errors + registry), D3 (Ticket writes + fence) | The seams the duplication hangs off; D3 is a live invariant break |
| 2 Shape | M2 (Sync), M3 (Page trait), M4 (presentation vocabulary), D4 (Connection), D5 (turnkeel registry + run history) | Depend on phase 1 seams |
| 3 Polish | M5 (component interface), M6 (lib + fixtures), D6 (runner skeleton + Conversations module), S1–S24 standards, U1 copy sweep, F1–F2 small fixes, L1 libraries, X1 docs | Mostly mechanical once the shapes exist |

---

## Phase 0 · Build and tooling (measured on an M2 Pro, 12 cores, Rust 1.98.1, Apple ld)

### Measurements

| What | Measured |
| --- | --- |
| Packages in Cargo.lock / built for Mac app / daemon / xtask | 932 / 435 / 363 / 394 |
| Unique crates whole workspace, with vs without Mac app and pilot crates | ~613 / ~408 (35% exist only for GPUI) |
| Mac app: one-line edit, `cargo build -p agentinc-os` / `cargo check` | 2.2 s / 1.0 s |
| Mac app: whole crate from scratch, non-incremental | 6.65 s |
| Mac app: final link | 0.30 s (linker is not a bottleneck) |
| gpui at opt-level=3 from scratch | 23.8 s |
| Daemon: edit, build / check | 3.6 s / 1.2 s |
| turnkeel edit then daemon build | 4.2–6.4 s |
| `just dev` app rebuild via `bundle.sh` | 7.4 s (cargo ~3 s, `stage-ghostty.sh` 3.7 s, `cargo xtask release-package-version` 0.9 s, codesign 0.4 s) |
| `bundle.sh` with nothing changed | 5.5 s |
| **`just check` / pre-commit, nothing changed, 3 runs** | **141 / 154 / 154 s** |
| Same steps from a plain shell: warm / UI touch / daemon touch | 3–4 s / 2 s / 11 s |
| Rebuild after a commit (build.rs watches `.git/HEAD`): bundle crates + xtask | 7.6 s + 6.1 s |
| First build of a new `-p` selection | 35–68 s |
| `cargo test --workspace --no-run` (26 test binaries; 13 integration files) | 68 s partly stale, 0.6 s warm |
| turnkeel tests: `cargo test` vs `cargo nextest` | 21.3 s with 1 flaky failure ("ephemeral server did not start within 5s") vs 12.8 s, 32/32 pass |
| Cold daemon build, `dev` profile | 200 s wall / 1345 CPU-s |
| Same, deps at opt-level=1 / `ci` profile (opt-level=0) | 137 s / 933 CPU-s; 87 s / 299 CPU-s |
| Of cold `dev` build: proc-macro crates / build scripts | 411 CPU-s / 160 CPU-s |
| `target/` | 41 GB, 24 GB incremental (42 copies of app incremental state, 33 of daemon's at ~0.5 GB each) |

Disk hit 121 MiB free during the review. Treat disk as a blocker before starting.

### WP-B1 · Scrub `CARGO_*` from every cargo command xtask launches — Strong, do first

Evidence. `cargo xtask check` runs `cargo clippy` from inside `cargo run`, so the child inherits
`CARGO_MANIFEST_DIR` and `CARGO_PKG_*`. ring's build script declares `rerun-if-env-changed` on
those. Cargo's fingerprint log shows it directly:
`EnvVarChanged { name: "CARGO_MANIFEST_DIR", old_value: Some(".../ainc-xtask"), new_value: None }`.
Each run rebuilds ring, rustls, reqwest, tonic, every temporalio crate, turnkeel, the daemon and
xtask, twice (xtask build and clippy build). The same leak hits the daemon that `just dev` runs
(`cargo xtask serve` → `cargo run -p ainc-daemon`): building the daemon by hand while it runs
costs 60–68 s in each direction.

Files. `crates/ainc-xtask/src/main.rs`: `step()` (~line 421), `cmd()` (~97), `daemon()` (~260);
`crates/ainc-xtask/src/release/prepare.rs`; `crates/ainc-xtask/src/runtime_smoke.rs`; anything
else that does `Command::new("cargo")`.

Change. One `fn cargo() -> Command` that calls `env_remove` for `CARGO_MANIFEST_DIR`,
`CARGO_MANIFEST_LINKS`, `CARGO_PKG_*` (iterate `std::env::vars()` and remove by prefix), used by
every call site. Add a unit test asserting the launched env has no `CARGO_PKG_` keys.

Acceptance. `just check` twice in a row with no edits: second run under 5 s. `cargo build -p
ainc-daemon` while `just dev` is running recompiles nothing.

### WP-B2 · Stop the release build script watching git refs; take the daemon out of xtask; split `ainc-release` — Strong

Evidence. `crates/ainc-release/build.rs` lines 20–40 run `git rev-parse` and print
`cargo:rerun-if-changed` on `.git/HEAD` and the branch ref, so every commit rebuilds every product
crate and xtask, once per variant. `crates/ainc-xtask/Cargo.toml:15` depends on `ainc-daemon` but
only `generate` needs it; xtask's closure is 580 crates with the daemon and 381 without, so `just
check`, `bundle.sh`, `just dev` and the release `prepare`/`distribute`/`publish` jobs all compile
Temporal to read a version string. The daemon and client link `ed25519`, `flate2`, `tar` and
streaming `reqwest` from `ainc-release` to get `client_header` and `check_client`.

Change.
1. `build.rs`: embed the commit only when `AINC_COMMIT` is set (the release pipeline already sets
   it); dev builds show no commit. Drop the `.git` watches.
2. Move `generate` into its own bin crate `crates/ainc-generate` (depends on `ainc-daemon`), keep
   the `cargo xtask generate` name as a thin exec of it, or a cargo alias.
3. Split `ainc-release` into `ainc-identity` (version, channel, headers, compatibility, discovery
   path, the `agent-inc-*` header names as consts) and `ainc-release` (updater, manifests,
   signing). Daemon, client, CLI and Mac app depend on identity only.

Acceptance. A commit with no code change recompiles nothing. `cargo tree -p ainc-xtask -e
normal` has no `temporalio-*`. `cargo tree -p ainc-daemon` has no `tar`/`flate2`/`ed25519`.

### WP-B3 · Profiles, test runner, bundle, CI — Strong

| Change | Evidence | Saving | Risk |
| --- | --- | --- | --- |
| `cargo nextest` in `just test` and CI (`cargo test --doc` separately; no doctests run today, all ignored) | 21.3 s → 12.8 s; 26 binaries serial | CI tests ~63 s → ~30 s; the flaky Temporal start test gets isolation | low |
| Replace blanket `[profile.dev.package."*"] opt-level=3` with explicit entries: keep 3 for gpui, its layout/text engines and the proc-macros our crates use (serde_derive, utoipa-gen, sqlx-macros, thiserror-impl, temporalio-macros); set 0 for aws-lc-sys, wit/wasm crates, bon-macros, mockall_derive, derive_more-impl, darling_core, zerofrom/yoke/zerovec-derive, synstructure, prost-derive, libsqlite3-sys. `[profile.dev.build-override]` does NOT work here (tried: 172 s) because `package."*"` wins | 411 of 1345 CPU-s are proc-macro crates | ~30% of cold builds | low |
| reqwest `default-features = false`, `rustls` with the ring provider installed at startup (`rustls::crypto::ring::default_provider().install_default()` in each binary's main) | aws-lc C build 100 s dev / 28 s CI, blocks every TLS crate until 119 s into a cold build; Temporal already uses ring; no openssl anywhere | ~100 s cold | medium: forgetting the provider install panics at first TLS use; add a smoke test |
| cargo-hakari workspace-hack crate | 17–34 crates change features between `-p` selections (serde_core, syn, tokio, hashbrown); gpui `test-support` comes in through dev-deps | 35–68 s per first variant build; much of the 41 GB | medium |
| `bundle.sh`: stamp-file skip of `stage-ghostty.sh` when bridge inputs unchanged; read the version from `Cargo.toml` directly instead of `cargo xtask release-package-version` | 3.7 s + 0.9 s of 7.4 s | UI loop → ~3 s | low |
| `just clean-incremental` recipe (cargo sweep or `rm target/*/incremental`); sccache shared across Treehouse worktrees | 24 GB incremental; each worktree starts cold (200 s) | disk, cold worktrees | low |
| CI: fold the `fmt` job into the clippy lane; drop `libssl-dev` from apt; replace the whole-`target/` cache keyed by SHA (2.1 GB per push, thrashes the 10 GB limit) with Swatinem/rust-cache or sccache; use temporal's `vendored-protox` feature to drop protoc installs; have CI call `cargo xtask check` instead of re-listing fmt/clippy/check-ui; move the `just test` Postgres bootstrap (bash + Docker + `sleep`) into `cargo xtask test` | AGENTS.md says tooling is xtask | unmeasured, minutes on cache miss | low |
| Gate turnkeel's `testing` module and Temporal's `testing` feature (ephemeral-server, zip, tar) behind a `testing` cargo feature | compiled into production | small | low |
| Merge turnkeel's four Temporal-heavy integration tests into one binary | ~6 CPU-s locally | small | low |

Not worth doing: lld/mold/`-ld_new` (link 0.30 s), splitting `agentinc-os` into crates (≤1 s per
edit), `-Zthreads` (needs nightly), `split-debuginfo` (`debug = 0` already).

Missing tooling to add to `just check`: cargo-deny with a `deny.toml` (installed, unused),
cargo-machete, `typos`, `taplo fmt --check`, a `just fix` recipe (`cargo clippy --fix` +
`cargo fmt`). bacon for a watch loop outside Tilt. cargo-insta only if snapshot tests appear.
Not worth it: cargo-semver-checks, release-plz, cargo-dist.

### WP-B4 · Workspace lints

Add `[workspace.lints]` in the root `Cargo.toml` and `lints.workspace = true` in every crate:
`rust.missing_docs = "warn"` for turnkeel and ainc-identity, `clippy.dbg_macro = "deny"`,
`rust.unused_must_use = "deny"`, and `clippy.toml` `disallowed-macros` for `println!`/`eprintln!`
in `ainc-mac` and `ainc-daemon` (allowed in `main.rs` and CLI/xtask). Today: no lints table, no
`missing_docs` anywhere, `#[allow(dead_code)]` ×25 (15 in `shell.rs`),
`clippy::too_many_arguments` ×2, `non_camel_case_types` ×2 (macros). Undocumented pub items: ainc-release
~17/30, daemon ~33/100; turnkeel's public modules are fully documented.

---

## Phase 1 and 2 · Mac app (`crates/ainc-mac`, package `agentinc-os`, 21k lines)

### WP-M1 · One Daemon module — Strong

Evidence. The daemon connection is three adapters:
- `src/storage.rs`: `background()` (lines 23–28, a process-global tokio runtime that `block_on`s),
  `client()` (35–110), `Store` (cache plus commands). `Store` re-exports ~20 generated
  `ainc_client::types` (3–8), so every page codes against wire types.
- `src/assistant.rs` (55 lines) calls `storage::client()` directly for Codex connection endpoints.
- `src/temporal.rs` ~189–198 calls `storage::background(storage::client()…temporal_executions())`
  and bypasses `Store`.

`client()` on every call: re-reads the discovery file; health-probes with a 1 s timeout (42–53);
may spawn `aincd` and poll up to 1200 × 50 ms (54–89); re-reads the owner token (95–99); builds a
new `reqwest::Client` (105–108). A ticket command calls `client()` twice (command, then
`refresh_inner`): two probes, the command, four GETs.

The idempotent command protocol (pending slot, operation_id, retry once on
`CommunicationError`, clear on 4xx, `refresh_inner`, clear pending) is pasted four times, ~50 lines
each: `storage.rs:263–322` (workspace), `357–416` (automation), `417–463` (ticket), `464–512`
(product). Representative (`443–458`):

```rust
let ack = background(async {
    let client = client().await?;
    let mut result = client.tickets_command().body(request.clone()).send().await;
    if matches!(result, Err(progenitor_client::Error::CommunicationError(_))) {
        result = client.tickets_command().body(request).send().await;
    }
    match result { Ok(ack) => Ok(ack.into_inner()), Err(error) => {
        if error.status().is_some_and(|s| s.is_client_error()) {
            *self.pending_ticket.lock().expect("pending Ticket command") = None; }
        Err(error.into()) } }
})?;
```

No seam for tests: the fake daemon is ~310 lines of `#[cfg(test)]` inside the production type
(`fixture: bool`, `fixture_ticket`, `fixture_ticket_create`, `fixture_workspace_command`;
`storage.rs:124–127, 174–185, 243–247, 323–356, 526–836`) and re-implements the daemon's Ticket
board rules (`545–549`). `Store::open(_path)` ignores its argument. `storage.rs` has zero tests,
so the retry policy is never tested. `update_required()` is a global atomic in
`ainc-client/src/lib.rs:14–18` polled from `Shell::render` (`shell.rs:906`). The app depends on
`reqwest` and `progenitor-client` only to build the client and match `CommunicationError`
(`ainc-mac/Cargo.toml:22`; `storage.rs:299,393,446,495`). The Mac app reads `AINC_DAEMON_URL`
(`storage.rs:37`), the CLI reads `AINC_API_URL` (`ainc-cli/src/main.rs:101`); discovery-path logic
is duplicated in `storage.rs:29`, `cli/main.rs:88`, `ainc-daemon/src/terminal_attach.rs:48–59`.

Target shape. A deep `daemon` module (`src/daemon.rs` + `src/daemon/`):
- `Daemon::connect() -> Daemon` once: discovery, spawn-if-needed, probe, token, one client.
  Reconnect on `CommunicationError`, not per call.
- `trait Command { type Ack; fn send(&self, client) -> ...; fn family() -> Family }` and one
  generic `Daemon::send<C: Command>(&self, op: OperationId, cmd: C) -> Result<C::Ack, DaemonError>`
  holding the pending slot per family, the retry-once rule and the clear-on-4xx rule.
- `Daemon::fetch(Slice) -> Snapshot` typed per slice (workspaces, product, tickets,
  automations, executions).
- `trait DaemonTransport` seam with `HttpTransport` and `MemoryTransport` adapters; the memory
  adapter replaces the `cfg(test)` fake and uses a shared pure board module instead of
  re-implementing rules.
- `DaemonError { Retryable, Rejected(ErrorBody), UpdateRequired, Unavailable }` classified inside
  `ainc-client` (`ainc_client::connect()`, `Error::classify()`), so `reqwest` and
  `progenitor-client` leave `ainc-mac/Cargo.toml`.
- Discovery and the URL override move to `ainc_identity::discovery()` (WP-B2) shared by app,
  CLI and attach client; the variable is `AINC_API_URL`.
- `update_required` becomes an observable on `Daemon`, not a polled atomic.

Tests. Retry-once, clear-on-4xx, pending-blocks-second-command, update-required propagation,
all through `MemoryTransport`.

Acceptance. One request path; `grep -r progenitor_client crates/ainc-mac/src` is empty; a
ticket command issues one command request and one fetch.

### WP-M2 · One Sync owner and one async action shape — Strong

Evidence. Polling loops (`loop { timer; this.update(refresh) }`): `tickets.rs:287–297` (2 s),
`automations.rs:138–148` (2 s), `evee.rs:942–983` (250 ms during a turn), `temporal.rs:115–129`
(1 s), `updates.rs:82–98` (60 s), `updates.rs:100–109` (100 ms). Every refresh fetches all four
snapshots (`storage.rs:212–229`) whichever page asked, from construction, even when hidden: ~8
GETs + 2 probes + 2 token reads every 2 s; during an Evee turn the full refresh runs every 250 ms.
`refresh()` with `refreshing`/`error`/`loaded`/`loading_started` flags duplicated identically in
`tickets.rs:318–349` and `automations.rs ~155–186`; the same fields re-declared in
`tickets.rs:120–123`, `automations.rs:16–19`, `evee.rs:60–63`, `temporal.rs:91`.
`mutate`/`command` (pending flag, background spawn, post back, reload, error) ×3:
`evee.rs:398–434`, `tickets.rs:566–612`, `automations.rs ~187–219`.
`background_executor().spawn` then `cx.spawn(this.update(...))` ~22 sites (23 and 22 grep hits);
`evee.rs:220–279` has three in a row (`refresh_connection`, `connect`, `disconnect`).
All polling is `#[cfg(not(test))]` (`tickets.rs:284`, `automations.rs:135`, `evee.rs:141,161`)
and `loaded: cfg!(test)` (`tickets.rs:273`, `automations.rs:119`), `credentials_busy: !cfg!(test)`
(`evee.rs:191`), so none of it is tested. Each page `cx.observe`d by Shell re-renders the whole
shell on any notify (`shell.rs:193,224,226,228,241`), including every hover-fade frame.
`architecture.md:17` claims "HTTP and SSE" but the daemon has no SSE endpoint.

Target shape.
- `Sync` entity owned by Shell: one cadence (2 s idle; 250 ms while a turn or run is active;
  paused when the window is hidden), fetches slices through `Daemon`, exposes
  `loaded / error / reconnecting` once, emits per-slice change events (`SliceChanged::Tickets`
  …). Pages subscribe to slices and keep no loop and no flags. Inject a clock/scheduler so tests
  drive ticks.
- `cx.run(op, apply)` helper (in `src/ui/async.rs` or `src/app.rs`): spawns on the background
  executor through `Daemon`, returns a `Pending` guard that blocks a second submission, posts the
  result back, routes errors to one sink with one wording. Replaces the three `mutate`/`command`
  copies and the ~22 spawn sites.
- Later (not now): the daemon grows an SSE or WebSocket event stream and `Sync` switches to it
  behind the same interface.

Acceptance. `grep -rn "background_executor().spawn" crates/ainc-mac/src` only inside the helper;
no `#[cfg(not(test))]` around sync; a test drives two ticks and asserts one fetch set per tick.

### WP-M3 · A `Page` interface; page dialogs leave the shell; Terminal, Settings, Agents, Connections become pages — Strong

Evidence. Adding a dialog touches four places: `model.rs:31–75` (the `Overlay` variant plus
`is_dialog`, `is_ticket_dialog`, `is_popover`), `shell.rs:626–629` (`cycle_focus`),
`shell.rs:1054–1063` (`dialog_content`), the page's own `overlay()` and `focus_handles()`.
`model::Overlay` knows ticket and conversation dialogs and `ConversationMenu(i64)` while Tickets
keeps its own 12-variant `Menu` enum (`tickets.rs:44–57`). Shell fans out by name:
`dismiss_menus` ×2 call sites × 3 pages (`shell.rs:1084–1088, 1149–1155`), font-size change
hand-notifies seven entities (`shell.rs:696–709`, global static `TYPE_SCALE` at
`ui/tokens.rs:305`), update-install draft preservation spans `shell.rs:529–564` and three
hand-written `update_drafts`/`restore_update_drafts` pairs (`evee.rs:86–110`,
`tickets.rs:156–188`, `automations.rs:57–95`); `updates.rs:413` reaches back into Shell through
the `UpdateHost(WindowHandle<Shell>)` global (`updates.rs:44–46`), a dependency cycle.
Settings is not a module: `Shell::static_page` in `shell/main_content.rs` assembled from the
shell's `appearance_section`, `Updates::settings()` (`updates.rs:458–548`) and
`AssistantPage::settings_view` (`evee.rs:280–397`), so the ChatGPT Connection UI lives in the
Assistant file. Terminal is three `#[cfg(target_os="macos")]` fields on Shell
(`shell.rs:111–116`), lazily created in `Shell::render` (`1003–1030`), hidden on navigate in
`dispatch` (`793–802`), empty state in `main_content.rs` (`terminal_page`); `src/ui/terminal.rs`
is the GPUI slot for `crate::terminal::TerminalHost`, so `ui` depends on an app module. Agents is
a method on `TicketsPage` (`tickets/agents.rs`), its dialog in `tickets/dialogs.rs`, rendered via
`tickets.update(|t| t.agents(window,cx))` (`shell.rs:1038–1040`).
Each page has its own "take me somewhere" event: `evee::Navigation {Settings, Chat, List}`
(`evee.rs:79–83`), `automations::OpenTicket(i64)` (`automations.rs:11`), `tickets::TicketsEvent
{OpenRuns, OpenConversation}` (`tickets.rs:60–65`), `Control::OpenTicket` (`shell.rs:63`).
`Control::Navigate` vs `Control::Open` (`shell.rs:673–686`) differ only in resetting the Assistant
to its list; sidebar uses Navigate, ⌘K uses Open, so "Assistant" behaves differently by entry.
`Route` takes no parameters, so Back does not leave a Ticket detail.
Shallow modules failing the deletion test: `src/shell/layout.rs` (47 lines, `layout_body`'s
`right` is always `None` at `shell.rs:1166`); `src/shell/pane.rs:4–17` `enum Side { Left }` with
`[T; 1]` arrays (`PANE_WIDTHS` `model.rs:5`, `Session.panes`, `grip_opacity`, `grip_animation`,
`pane_visible`, `pane_animation` `shell.rs:103–106`); `Option<Arc<Store>>` + `storage_error`
always `Some`/`None` (`shell.rs:146–147`) yet checked in `evee.rs:549,557,844,1123,1207`,
`tickets.rs:567,901`, `tickets/agents.rs:15,43`, `automations.rs`, with unreachable copy
"Conversation data is unavailable…" (`evee.rs:1207–1211`); `Shell::workspace_id`
(`shell.rs:135–141`) constructs a throwaway `Store::new()`.
`model.rs` mixes navigation (`Route`, `Router`, `PAGES`), the overlay vocabulary, UI preferences
and legacy session migration (`from_json`, ~110 lines) and palette `recent_commands`.

Target shape.
- `trait Page { fn render; fn overlay(&self) -> Option<AnyElement>; fn focus_handles(&self);
  fn dismiss_menus(&mut self); fn drafts(&self) -> Drafts; fn restore(&mut self, Drafts);
  fn title(&self) -> SharedString /* from PAGES */ }`, Shell holds `Vec<AnyPage>` keyed by
  `Route` and iterates; no per-page names in Shell.
- `OverlayHost` holds `Shell(Overlay) | Page(Route)`; page dialogs live in their page.
- One `Destination` event type replaces the four navigation events; `Open` and `Navigate`
  collapse.
- New entities: `TerminalPage` (owns the three cfg fields and lifecycle; `ui/terminal.rs` moves
  beside it), `SettingsPage` (appearance, updates, shortcuts), `ConnectionsPage` (the ChatGPT
  Connection UI out of `evee.rs`; module name matches CONTEXT.md), `AgentsPage` (out of
  `tickets/`). `Updates` exposes `on_before_install` callback instead of `UpdateHost`.
- `model.rs` splits into `routes.rs` (Route, Router, PAGES), `overlay.rs`, `ui_state.rs`
  (the persisted Session, renamed `UiState` to free the word Session).
- Delete `shell/layout.rs`, `pane::Side`, the `[T;1]` arrays, `Option<Arc<Store>>`,
  `storage_error`.
- Speculative, later: `Route::Ticket(id)`, `Route::Conversation(id)` so Back/Forward cover
  details (touches session persistence and migration).

Acceptance. Adding a dialog touches one file. `shell.rs` under 1,000 lines. No `cfg!(test)`
value forks in page constructors.

### WP-M4 · Presentation vocabulary: time, work state, shortcuts, copy — Strong

Evidence.
- Relative time, two incompatible: `tickets/model.rs:134–146` ("now", "5m", seconds) and
  `temporal.rs:42–54` ("just now", "5m ago", milliseconds, no upper bound).
- Absolute date, four: `ui/display.rs:106–114` (`%b %-d, %H:%M`), `temporal.rs:56–64`
  (`%b %-d, %Y · %H:%M:%S`), `evee.rs:19–37` (`Today, HH:MM` / `Yesterday` / `MM/DD/YY` by
  string slicing; "Today" means "last 24 hours", so yesterday 23:00 shows as "Today, 23:00"),
  and the raw daemon string `"Updated {conversation.updated}"` at `evee.rs:642–645` (daemon
  `to_char(...,'YYYY-MM-DD HH24:MI')` in server timezone, `ainc-daemon/src/product.rs:203`; the
  API carries both `updated` and `updated_at`). Gallery literal "2m ago" (`components.rs:357`).
- `now()` helpers ×4: `tickets/list.rs:18–22`, `evee.rs:558–560`, `temporal.rs:306`,
  `storage.rs:553–555`. Units: seconds in DB, milliseconds in `ExecutionView`.
- `"queued" | "running"` string checks ×5: `tickets.rs:648`, `tickets/agents.rs:65`,
  `evee.rs:457`, `evee.rs:954`, `tickets/detail.rs:480,869`. State→`Tone` tables ×3:
  `tickets/detail.rs:861–875`, `automations.rs:492–499`, `temporal.rs:32–39`. The app duplicates
  wire strings it already gets typed (`tickets/model.rs:37–45 status_key` mirrors daemon
  `tickets.rs:55`).
- Shortcut display, four notations: `"⌘ + K"`, `"⌘ + [ / ⌘ + ]"` (`shell/main_content.rs:4–11,
  202`); `"⌘K"`, `"⌘,"`, `format!("⌘{}")` (`shell/sidebar.rs:36,81`, `shell/palette.rs:32–54`);
  `"Search · ⌘ K"`, `"Toggle sidebar · ⌘ B"` as accessible labels (`sidebar.rs:61`,
  `header.rs:87`); "Return to send · Shift+Return…" (`evee.rs:1272`); `"ESC"` vs `"↵"`. The
  `kbd()` doc comment shows `⌘ + K` (`ui/display.rs:157`). Bindings (`shell.rs:1273–1281`) and
  display strings are maintained separately; `crates/ainc-mac/README.md:67–72` says Back/Forward
  is Cmd+Option+Left/Right but the code binds `cmd-[`/`cmd-]`.
- No pluralization helper; counts are bare numbers.

Target shape, all in `src/ui/`, pure, unit-tested:
- `ui/time.rs`: `relative(secs) -> String` ("now", "5m", "3h", "2d", then "%b %-d"),
  `absolute(secs)` ("%b %-d, %H:%M", adds year when not this year), `duration(ms)`, one
  `now()`, local time, seconds in. Daemon drops the `to_char` `updated` field (see S5).
- `ui/work_state.rs`: `enum WorkState { Queued, Running, Done, Failed, Cancelled, … }` with
  `parse(&str)`, `label()`, `tone()`, `is_active()`; used by Ticket runs, Automations
  occurrences and Temporal executions.
- `ui/shortcuts.rs`: one table `(action, keystroke, glyph)` that builds the `KeyBindings`,
  feeds `kbd()`, the Settings list, accessible labels and a generated README table
  (`cargo xtask readme` already exists; extend it).
- `ui/copy.rs`: `unavailable(thing, recovery)`, `pluralize(n, one, many)`, the confirm-dialog
  builder `confirm_delete(name, what)`.
- xtask `check-ui` grows: ban `.format("%` outside `ui/time.rs`; ban string literals
  `"queued"`/`"running"` outside `work_state.rs`; ban `"⌘ +"`.

### WP-M5 · One component interface; hover plumbing hidden; tokens enforced — Worth exploring

Evidence. Interface styles: builder + `.build(&hover, action, cx)` (`Button`, `ListRow`,
`MenuEntry`, `MenuButton` `build(hover, items, on_toggle, cx)`, `Select` `build(hover, on_toggle,
on_select, cx)`); builder with different `.build` (`Field` `build(window, cx)`, no hover;
`EmptyState`/`PageHeader`/`Page` `build()`; `LoadingFrame::new(start, window).inline(..)`);
free functions with 6–8 positional args (`segmented`, `tabs`, `chip`, `toggle`, `checkbox`,
`table_row` with 8 args and `#[allow(clippy::too_many_arguments)]` `ui/table.rs:83–84`);
struct-plus-closures (`render_palette(PaletteView{..}, hover, 3 closures, window, cx)`,
`Toasts::render(hover, right, bottom, on_dismiss, cx)`). `toggle` and `checkbox` take no
`HoverFade` though design-system.md says every interactive component does. Selectors built four
ways: `format!("{id:?}")` Debug (`ui/toggle.rs:17,69`), `author_id` (`button.rs:70–77`), `chip`
only for `ElementId::Name`, explicit `.selector()` on `MenuEntry`/`EmptyState`/`Field`.
Hover: each host owns a `HoverFade`, writes a 5-line `impl HoverHost` (7: `shell.rs:127`,
`evee.rs:68`, `tickets.rs:132`, `automations.rs:34`, `temporal.rs:96`, `components.rs:27`,
`updates.rs:64`), calls `self.hover.animate(window)` in every render path (9, incl. non-Render
`settings_view`, `agents`, `UpdateView::settings`); forgetting it silently stalls fades.
Menu state: `OverlayHost` for dialogs/Search/Notifications/UserMenu/ConversationMenu, but
per-page booleans/enums for selects and menus (`TicketsPage.menu` 12 variants,
`AssistantPage.model_menu_open`, `ComponentsPage.select_open`) which is why Shell calls
`dismiss_menus` on three pages.
Naming overlap: badge/pill/tag/chip/count_badge/hint/kbd (Automations list uses `badge`, its
detail `status_pill` `automations.rs:466` vs `572`; Agents list uses `status_pill`; `tag`
re-implements `status_dot` inline `ui/badge.rs:82–88`; `chip` is a form control not a label);
dialog/sheet/popover/menu/overlay/floating/Deferred; `panel()` is the content card, `card()` a
raised group, `settings_section` hand-rolls a third surface (`ui/settings.rs:12–19`).
Raw `px(n)` spacing literals bypass tokens (colours are gated, spacing only warns):
`shell/header.rs` 19 (tab contour geometry), `ui/badge.rs` 9 (`h(22.)`, `h(18.)`, `px(5.)`,
`gap(6.)`), `ui/segmented.rs` 5, `ui/display.rs` 5 (`kbd` `h(20.)`/`px(5.)`, `nav_icon`
`size(17.)`), `ui/loading.rs` 5, `ui/empty.rs` (`size(40.)`, `max_w(480.)`), `menus.rs`
(`popover_shell(350.)`, `h(44.)`), `evee.rs` (`max_w(620.)`, `680.`, `900.`, `w(160.)`), select
widths `240.` (`evee.rs`, `automations.rs`), Temporal columns `150/170/90` (`temporal.rs:77–83`),
`font_family("SF Mono")` (`temporal.rs:272`). `ButtonKind::Destructive` doc says "A solid red
action" (`ui/button.rs:15`), code and docs say outlined. The "+ after the label" rule is a hidden
special case on `icon_name == Some("plus")` in `Button::build`. Icons are stringly typed; a
missing name silently returns `Ok(None)`.
Duplicated Ticket presentation: `DraggedTicket::card` (`board.rs:36–91`) re-implements
`TicketsPage::card`; status group header in board lane and `tickets/list.rs:60–104`; "Working"
indicator in `board.rs` card and `list.rs` `list_row`; "Clear filters" closure
`tickets.rs:830–845` and `tickets/list.rs:30–41`; assignee avatar choice ×4
(`tickets.rs:627–632`, `board.rs:42–48`, `ui/select.rs:212–218`, and `tickets/agents.rs` which
wrongly uses the round person `avatar` for agents). Open-overlay boilerplate ×8
(`evee.rs:671,694`, `tickets/detail.rs:131,158,694`, `tickets.rs:466,484`,
`shell.rs:648,721,732`). Per-page loading/error frame hand-assembled ×4 (`tickets.rs:910–944`,
`tickets/agents.rs:28–49`, `automations.rs ~629–668`, `temporal.rs:336–372`); Assistant has none
and shows "Start a conversation" before data loads.

Target shape.
- Every control is a builder ending in `.build(ui, action)` where `Ui<'_, V>` carries `cx`,
  `window` and the hover state (or hover state keyed by `ElementId` via element state) so hosts
  drop `HoverFade`, `HoverHost` and `animate`. `HoverFade::track` remains the one place hover is
  computed.
- Selectors derive from the authored id in one function (`author_id`, already extracted in the
  current staged change to `button.rs`).
- `enum Icon` replaces `&'static str` names.
- Selects and menus register with `OverlayHost` as popovers (one open at a time); the 12-variant
  `Menu` enum and `dismiss_menus` fan-out go.
- Shared pieces: `ticket_card_body`, `status_group_header`, `working_indicator`,
  `assignee_avatar(Assignee)`, `page_frame(loaded, error, reconnecting, skeleton_rows, body)`.
- `badge` for lists/headers, `status_pill` for tables/property rows, `tag` for labels, `chip`
  for form picks; document and apply. `settings_section` uses `card()`.
- Promote `checks/ui_spacing.rs` from warn to fail once the literals above are tokenized
  (`TAB_CONTOUR_*`, `BADGE_HEIGHT`, `KBD_HEIGHT`, `READING_WIDTH`, `SELECT_WIDTH`, table column
  tokens, `FONT_MONO` from the font setting).

### WP-M6 · Library target and a `fixtures` feature — Worth exploring

Evidence. No `lib` target; `tests/rendered_shell.rs:5–36` re-includes every module via
`#[path]`, duplicating `main.rs`'s module list. 41 `fn fixture_*` methods on production structs,
27 `#[allow(dead_code)]`, ~190 fixture lines in `shell.rs:341–527`, `evee.rs:755–784`,
`temporal.rs:143–165`. `titlebar_zoom_requests` (`shell.rs:117–118`) is test-only state.
Notifications are fixture-only: `notification_items` filled only by `fixture_notifications`
(`shell.rs:420–438`), so production shows an empty panel with a "Mark all read" control and a
palette "Show notifications" entry that never apply.
Unit tests by file: `shell.rs` 15 (window-level), `model.rs` 9, `fuzzy.rs` 6, `input.rs` 5,
`tickets/model.rs` 4, `evee.rs` 2, else 0–1. Untested or rendered-only: `storage.rs` (zero
tests); polling/refresh/error states; `updates.rs` state machine with magic native action codes
`1..7` (`updates.rs:200–245`, one test for `apply_choice`); `TicketsPage::describe` activity
sentences (`tickets/detail.rs:390–488`, pure apart from `assignee_name`); Temporal and
Automations formatters (`relative_time`, `duration`, `status_tone`, `workflow_label`,
`rule_state`, `every`); palette ranking (`palette_candidates` needs `&Shell`); board drag/drop
(`DragState::after`, `drop_ticket`; `apply_move` is tested).

Target shape. `src/lib.rs` with a thin `src/main.rs`; `src/testing/` behind
`feature = "fixtures"` (rendered and pilot tests enable it); the in-memory `Daemon` adapter from
M1 replaces the fake store; `UpdateState::on(event) -> Vec<Effect>` reducer with a typed
`NativeAction` enum; `describe` moves to `tickets/model.rs` with a name-lookup closure;
`fn candidates(&UiState, &[Ticket]) -> Vec<PaletteCandidate>` pure. Either wire real
notifications (daemon activity feed) or remove the bell, panel and palette entry.

### Mac app dependencies (fold into M1/M6)

`reqwest` and `progenitor-client` leave with M1. `tokio` (`rt-multi-thread`) exists to
`block_on` inside gpui's executor (`storage.rs:23–28`); after M1 a current-thread runtime or a
gpui/tokio bridge suffices; `tokio::sync::watch` in `updates.rs` is the other use.
`objc2-app-kit` features `NSImage`, `NSResponder` and `objc2-foundation` `NSData` look unused
from Rust (only `about.rs` uses `NSApplication`, `NSDictionary`, `NSString`); trim.
`accesskit = "0.24.0"` is pinned independently and must match gpui's. Dev-deps
`tokio-tungstenite`, `futures`, `libc` serve only `tests/pilot_terminal_sessions.rs`
(requires `automation`) but compile for every `cargo test` including Linux CI; move them behind
the feature. `chrono`, `pulldown-cmark`, `libloading`, `raw-window-handle`,
`unicode-segmentation` are justified.

---

## Phase 1 and 2 · Daemon, SDK and clients

### WP-D1 · One command-receipt module and an `OperationId` — Strong

Evidence. Command receipt (UUID check, advisory lock, look up receipt, replay or conflict,
insert) ×4 in four tables with four scopings and four lock-key formats; `result_id` is bigint in
some and text in others: `product.rs:245–273,402–408` (`command_receipts`);
`tickets.rs:496–545` (`ticket_receipts`); `automations.rs:137–155,233` (`automation_receipts`);
`workspaces.rs:109–132,181–186` (`workspace_receipts`). SHA-256 of an idempotency key turned into
a UUID operation ID ×4: `conversation_tools.rs:79–82,160–163`, `automations.rs:287–289`,
`coding.rs:367–368`. The same bad-UUID check returns `"invalid"` from tickets and automations but
`"invalid_operation"` from product and workspaces (`tickets.rs:496`, `automations.rs:137`,
`product.rs:245`, `workspaces.rs:109`). Operation IDs are `String`s validated by hand four times,
terminal IDs a fifth. In the daemon the mutation `TicketsTool` is marked idempotent but
`Server` + `Runtime::configured` never turns on `check_idempotency`, so receipts are checked by
hand instead (`conversations.rs:367–376`).

Target shape. `crates/ainc-daemon/src/receipts.rs`:
`OperationId` newtype (`parse`, `from_idempotency_key`), `Scope`, and
`receipts::execute(tx, scope, op, &cmd, |tx| apply) -> Result<Receipt, CommandError>` with the
replay/conflict rule and one lock-key rule. Expand/contract migration to one `receipts` table
(`scope`, `operation_id`, `request_hash`, `result jsonb`). DTO fields typed as UUID in the
schema. One error code (`invalid_operation`) for a bad id.

### WP-D2 · A domain `CommandError`, auth extractors, one endpoint registry, one `app()` — Strong

Evidence. Shared HTTP plumbing (`Product`, `authorize`, `ApiError`, `ErrorBody`) lives in the
legacy module `product.rs:14–175`; every module imports `crate::product::{ApiError, ErrorBody,
Product}`. `authorize(&headers)` + `workspaces::current` repeated in ~10 handlers
(`product.rs:189,231`; `automations.rs:101,111`; `terminal_sessions.rs:102,129,161,184`;
`workspaces.rs:67`; `tickets.rs:338`). Per-module `invalid()` helpers (`tickets.rs:300`,
`automations.rs:87`, `workspaces.rs:191`). `tickets::execute_in` and `automations::execute`
return `ApiError` with status codes even from tools and activities; callers discard the reason:
`conversation_tools.rs:83,164` and `coding.rs:381` `map_err(|_| ...)` so the model always gets
"Command refused. Read current Ticket revisions…"; `automations.rs:305` formats with `{e:?}`.
`From<sqlx::Error>` (`product.rs:149–170`) turns non-constraint errors, including SQL bugs, into
503 "Try again". Error shape exceptions: `/health/ready` bare 503 (`lib.rs:51–55`); 426 body
hand-built (`lib.rs:155–158`) and declared on no operation; `code` is a free string (`conflict`,
`invalid`, `invalid_operation`, `unauthorized`, `forbidden`, `unavailable`,
`connection_unavailable`, `busy`, `terminal_unavailable`, `session_ended`). Same failure,
different status: missing record is 409 in product and workspaces, 403 in tickets
(`tickets.rs:460`), 404 in terminal; wrong owner token on `/v1/tickets` falls through to the
agent-credential lookup and returns 403 "outside the current assignment" instead of 401
(`tickets.rs:338–351`). The conflict message "has a reply in progress" is returned for all
resources (`product.rs:145`). `temporal.rs:86–87` string-matches turnkeel error text
(`"invalid page token"`) because turnkeel reports validation as `Error::Connection`
(`engine/visibility.rs:21,47`, `engine/recurring.rs:67`).
Registration: each path written three times (`#[utoipa::path(path=…)]`, `.route(…)`, the
`paths(...)` list `lib.rs:76–99`); `connection.rs:165–170` keeps its own `openapi()` merged at
`lib.rs:104`; `temporal::router` merged in `main.rs:160` not `product_router`; router assembly
split across `lib.rs:110–143`, `temporal.rs:44–54`, `main.rs:138–162`; middleware attached in
three places so base routes get the compatibility check and version header twice
(`lib.rs:117–118` and `141–142`). Dead `/v1/tickets/contract` phase-1 echo (`lib.rs:26–29,
70–74`) still in client and CLI (`ainc-cli/src/main.rs:124`). ~9 env vars read ad hoc in
`main.rs` and `codex.rs`; `AGENTINC_CODEX_HOME`/`AGENTINC_CODEX_PATH` (`codex.rs:17,23`) break
the `AINC_*` scheme.

Target shape. `crates/ainc-daemon/src/api.rs`:
- Extractors `Owner` and `Actor` (`FromRequestParts`) that do auth + current workspace.
- `enum CommandError { Invalid(String), Conflict(String), Forbidden, NotFound, Unavailable }`
  with `IntoResponse` (one `ErrorBody { code: ErrorCode, message }` where `ErrorCode` is a
  schema enum), `Into<turnkeel::ToolError>` keeping the message, and `From<sqlx::Error>` that
  maps constraint violations only and lets everything else surface as a 500 with tracing.
- 426 declared as a shared response in the OpenAPI; `/health/ready` returns `ErrorBody`.
- `utoipa-axum` `OpenApiRouter` + `routes!` so each endpoint is registered once; one `app()`
  that merges every module's router and applies `tower-http` `SetResponseHeader`, `TraceLayer`
  and the compatibility layer exactly once. Delete the contract stub.
- `Config` struct read once in `main.rs` (figment or plain `std::env`) with the `AINC_` prefix
  only; rename the two `AGENTINC_CODEX_*` variables.
- turnkeel adds `Error::InvalidInput` (additive) so `temporal.rs` stops string-matching.

### WP-D3 · Every Ticket write through `tickets::execute_in`; one assignment fence — Strong, correctness

Evidence. `product.rs:368–396` `CreateTodo`, `CompleteTodo`, `DeleteTodo` write `tickets`
directly: no board lock (breaks the AGENTS.md invariant), no activity, skip `enter_column` and
positions; `DeleteTodo` skips `links::detach_all`, so the other Ticket's history never shows the
lost relationship (contradicts `phase-3-tickets.md`). `coding.rs:162–184` inserts Comments
directly instead of `tickets::execute`. `execution.rs:240–336` `advance`/`project` write
correctly. "Is this agent's assignment still live?" is five SQL variants: `tickets.rs:349`,
`tickets.rs:408`, `tickets.rs:461–473`, `coding.rs:133`, `execution.rs:271–283`;
`automations.rs:117` and `:321` hard-code `generation=1`. `conversations.rs:141` and `:157`
lock the same Conversation row twice. `automations::snapshot` (`automations.rs:115–124`) reads
three tables without a transaction while `product.rs:200`, `tickets.rs:415`, `workspaces.rs:79`
use REPEATABLE READ. Only `tests` call the Todo commands (`docs/phase-3-tickets.md` calls them a
phase-2 compatibility adapter); OpenAPI marks nothing deprecated.

Target shape. Translate the Todo commands into `Create`/`SetStatus`/`Delete` inside
`tickets::execute_in`, mark them deprecated in the OpenAPI, delete in the following release.
`coding.rs` posts Comments through the Ticket module. `tickets/fence.rs`:
`LiveAssignment::lock(tx, actor) -> Result<LiveAssignment, CommandError>` defines the predicate
once (assignee, generation, actionable status) and every writer uses it. Extract
`change_status` (`tickets.rs:906–935`) into a pure transition plan (cancel, bump generation,
start, refuse) unit-tested without Postgres; same for Occurrence admission
(`automations.rs:269–279`). A `pg::snapshot_tx()` helper used by all four snapshots.

### WP-D4 · One ChatGPT Connection module — Strong

Evidence. The Connection is spread over `codex.rs` (process + JSON-RPC over stdio with a reader
thread), `connection.rs` (HTTP + in-memory status), `inference.rs:76–140` (credentials),
`main.rs:124`, plus Mac `assistant.rs` and `evee.rs:280–397`. `account/read` appears in
`codex.rs:165`, `inference.rs:107` and `codex::status`. `CodexModel::complete` →
`credentials()` (`inference.rs:97–140`) spawns `codex app-server`, does the initialize
handshake, calls `account/read` with `refreshToken:true`, re-reads `auth.json`, and for
`connection-default` also `model/list`, on every model step of every turn. `GET /v1/connection`
spawns a process per request (`connection.rs:70`). `codex.rs:52–57` kills only the child, not
its group. `CodexModel::new` duplicates `CodexModels::resolve` and is test-only
(`inference.rs:87–95`). The SSE client (`inference.rs:142–278`) does not stream: buffers up to
4 MiB (`171–183`) then splits on `"\n\n"`; `request_body` and `parse_stream` are pure and well
tested. `ConnectionStatus.account` (`connection.rs:21`) uses an avoid-word.

Target shape. `crates/ainc-daemon/src/connection.rs` + `connection/`: one long-lived Codex
client (or a token cache with refresh on 401) exposing `status()`, `login()`, `logout()`,
`credentials()`; `inference.rs` keeps only the pure request and parse code and uses
`eventsource-stream` for framing. Rename the DTO field `account` → `identity` or `signed_in_as`.
Tests for status error stickiness, login/cancel/logout, and "one spawn per process lifetime"
using a Rust fake app-server (see S18).

### WP-D5 · turnkeel: lazy agent resolution, run history in agent vocabulary, SDK names — Strong

Evidence. Registry (`engine/activities.rs:16–42`) keyed by agent name, filled at `configured`,
`start_with_id`, `open_session`, `session_handle` (`engine/mod.rs:126,201,248,274`), never
shrinks. Tools cannot receive per-run context (AGENTS.md rules out `run_id` on `ToolCtx`), so
the daemon mints an agent name per run: `ticket-{run_id}-v1` (`execution.rs:55`),
`conversation-{id}-v1` (`conversations.rs:34`); startup rebuilds every in-flight agent before
`Runtime::configured` (`execution.rs:106–111`, `conversations.rs:64–70`); the versioned
definition model (`evee-v2`) is bypassed. `SharedModel(Arc<dyn Model>)` wrapper ×2
(`execution.rs:18–30`, `conversations.rs:19–31`) because turnkeel lacks `impl Model for Arc<dyn
Model>`. `RunHandle`/`SessionHandle` duplicate cancel and error mapping
(`engine/mod.rs:368–454`); `Run::events` (`run.rs:44–62`) duplicates `Session::events_from`
(`session.rs:73–88`).
Temporal leaks: `Runtime::run_history` returns `RunRecord { id, run_id, kind, status }`
(`runtime.rs:15–46`) where `status` is Temporal's (`ContinuedAsNew`, `Terminated`, `TimedOut`;
`engine/visibility.rs:112–121`), `kind` is the workflow type name, `run_id` is Temporal's
per-attempt id contradicting `run.rs:6` ("never exposed"). The daemon serves
`/v1/temporal/executions` with `workflow_id`/`workflow_type` and Temporal UI URLs
(`temporal.rs`), the Mac app has a Temporal page hard-coding `"agentinc.run"`,
`"agentinc.session"`, `"turnkeel.occurrence"` (`ainc-mac/src/temporal.rs:25–27`) and mapping raw
`ContinuedAsNew`/`TimedOut` (`:10`). `visibility.rs:68–75` opens a new Temporal client per call
and scans the full retained history per page (`82–141`). `RecurringAction::execute` must return
`turnkeel::Error` (`recurring.rs:26`); `Occurrence.input` is untyped `Value`. The daemon mutates
the worker group with `push_str` (`conversations.rs:69`, `automations.rs:346`).
`agent_spec` calls `TicketsTool::schema()` inside every `model_step` activity
(`engine/activities.rs:83`), and `schema()` rebuilds the whole OpenAPI document
(`conversation_tools.rs:35`): twice per model call.
Naming: workflow/activity names `agentinc.run`, `agentinc.session`, `agentinc.model_step`,
`agentinc.call_tool`, id prefixes `agentinc-run-`, `agentinc-session-`, queues
`agentinc-{uuid}`, thread `agentinc-worker`, log "agentinc worker stopped"
(`engine/workflow.rs:12,62`, `engine/session.rs:12,45`, `engine/activities.rs:73,99`,
`engine/mod.rs:62,74,135,165`) while recurring uses `turnkeel.occurrence`
(`engine/recurring.rs:22,48`). These are durable Temporal identifiers.
Testing gaps: no scripted way to return `ModelError::retryable`; recurring and `run_history`
untested inside turnkeel; `Runtime::test`/`local` download the Temporal CLI; sessions never end.

Target shape, all additive.
- `Runtime::configured_with(config, impl AgentSource)` where `AgentSource::resolve(name) ->
  Option<Agent>` runs on the worker; the registry becomes a cache with eviction when a run ends.
  The daemon's source reads the Ticket/Conversation definition from Postgres. Startup
  reconstruction and per-run names go away; versioned definitions (`evee-v2`) work.
- `impl<M: Model + ?Sized> Model for Arc<M>`; delete both `SharedModel`s.
- `pub enum RunStatus { Running, Completed, Failed, Cancelled }`, `RunRecord` without the
  attempt id and with `kind: RunKind { Run, Session, Occurrence }`; `Error::InvalidInput`;
  `visibility` reuses the engine's client and reads only the needed page. The daemon's
  `/v1/temporal/executions` becomes `/v1/work` (or folds into Ticket runs) and the Mac page
  stops knowing Temporal names.
- Rename SDK-internal names to `turnkeel.*`/`turnkeel-` and register the old names as aliases
  for retained histories; xtask check: no `agentinc|ainc|ticket|evee` under
  `crates/turnkeel/src`.
- `testing::Script` gains `reply_retryable()`; `Server` turns on `check_idempotency`; recurring
  gets its own tests. Gate `testing` behind a feature (WP-B3).
- `TicketsTool::schema()` cached in a `OnceLock`; `RunHandle`/`SessionHandle` share one
  `handle.rs`.

### WP-D6 · Shared background runner; Conversations as a module — Worth exploring

Evidence. Background runner (detached advisory-lock owner, PgListener, JoinSet, 1–2 s sleep
tick, drain) ×3 with magic lock IDs `7358710303`, `7358710202`, `7358710404`:
`execution.rs:91–168`, `conversations.rs:51–133`, `automations.rs:337–370`. NOTIFY channels as
literals across 5 files: `agentinc_dispatch`, `agentinc_turns`, `agentinc_results`,
`agentinc_automations`. `Definition` SELECT ×4 in `execution.rs:106,133,188,396`. Run state
tracked twice (`ticket_runs.state` and workflow status; the runner awaits `run.result()` per run,
`execution.rs:133–152`). Conversation lives in five places (`product.rs:47–63,274–367` commands,
DTOs, snapshot; `conversations.rs` runner; `conversation_tools.rs`; `connection.rs:145–156`
logout reads `turns`; `product.rs:313` delete cancels sessions); its only live write API is the
"legacy compatibility adapter" `/v1/state` + `/v1/commands` (`product_state`/`product_command`).
`conversations.rs:252–278` `save_result` is called only from `tests/product.rs:91` and has a
sleep-retry loop. `product.rs:194–196,236–238` `snapshot`/`execute` wrappers hard-code
`"local"` and are test-only. `conversation_tools.rs` has two copy-pasted structs switched by
`mutation: bool`; the `inline($ref)` schema walker is duplicated
(`conversation_tools.rs:36–55,121–140`; `ainc-cli/src/main.rs:143–182`). Tests spin on
`yield_now` without a gate: `automations.rs:515–525, 611–624`, `temporal.rs:175–192` (up to 200
iterations), `tests/process.rs:435–446`.
Process spawning: four owners with their own unsafe libc: `coding.rs:243–338` (sandbox-exec,
process group, SIGKILL group on drop), `terminal_sessions.rs:247–394` (openpty, setsid,
SIGHUP), `local_runtime.rs:13–236` (Postgres/Temporal children, 100 ms readiness sleep),
`codex.rs:46–128`. `aincd` is three programs dispatched on argv (`main.rs:31–46`); the terminal
attach WebSocket client (`terminal_attach.rs`) belongs in `ainc-cli`. Legacy SQLite import runs
on every start forever (`main.rs:81–86`); `rusqlite` (bundled C) exists only for it.

Target shape.
- `crates/ainc-daemon/src/worker.rs`: `Leased { lock, listener, tick, JoinSet, drain }` with a
  `reconcile()` hook; `pg/coordination.rs` with channel and lock-id constants; tests call
  `reconcile()` or LISTEN instead of spinning.
- `conversations.rs` + `conversations/{commands,runner,tools}.rs`; `/v1/conversations` added
  beside `/v1/state`; `/v1/state`/`/v1/commands` deprecated then removed; one
  `CommandFamily` descriptor (schema computed once) behind `CommandTool<C>`, `ReadTool<S>` and
  the CLI's variant builder.
- `command-group` / `pty-process` for process groups; `terminal_attach` moves to `ainc-cli`;
  `rusqlite` behind a `legacy-import` feature with a done-marker row and a removal date.
- Delete `save_result`, the product wrappers, `CodexModel::new`.

### API seam, client and CLI (fold into D2/D6)

Pipeline: utoipa → hand-kept `paths()` → xtask 3.1→3.0.3 conversion with custom nullable passes
(`ainc-xtask/src/main.rs:297–420`) → progenitor client + CLI checked in → version hooks in
`ainc-client/src/lib.rs`. Coherent; check mode keeps it honest. Manual parts: the CLI hand-maps
operation IDs to groups (`ainc-cli/src/main.rs:119–136`) by `split_once('_')` (yields `ainc
product …` for conversations, `ainc terminal sessions_list`), hard-codes group → command schema
(`144–149`, workspaces get no schema-derived flags), re-parses the OpenAPI JSON at runtime with
`Box::leak`, and passes the body through a temp file (`404–406`). The CLI identifies itself as
`mac/…` (`ainc-release/src/lib.rs:110`). Adding one read endpoint today touches 2 daemon files
+ 3 generated + 1–4 consumers. Generated types are the Mac app's domain model
(`storage.rs:3–12`, `temporal.rs:2`, `tickets/fixture.rs:6`, `shell.rs:381`). Two major
`schemars` (0.8 for progenitor, 1.x for turnkeel) until progenitor upgrades. OpenAPI lint to add
in xtask: operationId `{resource}_{action}` with resource ∈ CONTEXT.md nouns; schema names
`XSnapshot`/`XReceipt`/`XCommandRequest` (rename `WorkspaceState`, `Acknowledgement`,
`AutomationSnapshot.rules` → `automations`); no avoid-words; every operation declares the
shared error responses; client header string from identity consts. SQL: adopt
`sqlx::query_as!` with offline `.sqlx` data (`cargo sqlx prepare --check` in CI) or one column
list per row struct (extend `TICKET_COLUMNS`).

---

## Phase 3 · Standardize on (S1–S24)

| # | Today | Standard | Enforced by |
| --- | --- | --- | --- |
| S1 | xtask spawns cargo with inherited `CARGO_*` | one `cargo()` launcher | test on the env (B1) |
| S2 | serial `cargo test`, one flaky Temporal start | `cargo nextest` | justfile, ci.yml (B3) |
| S3 | no workspace lints, 25 `allow(dead_code)` | `[workspace.lints]` + `lints.workspace = true` | Cargo, `just check` (B4) |
| S4 | error codes as free strings, 426 undeclared, bare 503 on ready | `CommandError` → one `ErrorBody` with a code enum | types; OpenAPI lint (D2) |
| S5 | epoch seconds, milliseconds and SQL `to_char` strings on the wire; `now()` vs `clock_timestamp()` mixed (`product.rs:349` vs `links.rs:86`, `execution.rs:170`); `timestamptz` in `product_bootstrap`/`legacy_imports`, bigint elsewhere; `last_seen` vs `*_at` | epoch seconds on every DTO, `*_at` columns, no formatting in SQL, `clock_timestamp()` for event times; drop `Conversation.updated` | OpenAPI lint; migration checklist |
| S6 | seven time formatters, four `now()`s in the app | `ui::time` only | xtask check bans `.format("%` elsewhere (M4) |
| S7 | four shortcut notations, README wrong | one shortcuts table, glyph-only `⌘K`, generated README | shared table (M4) |
| S8 | `agentinc.*` and `turnkeel.*` names inside the SDK | `turnkeel.*`, old names aliased | xtask check (D5) |
| S9 | `AINC_*`, `AGENTINC_*` (`AGENTINC_SESSION_PATH`, `AGENTINC_CODEX_HOME`, `AGENTINC_WINDOW_TITLE`, `AGENTINC_CAPTURE_*`, `AGENTINC_PILOT_NARROW`), `TURNKEEL_*`; `AINC_DAEMON_URL` vs `AINC_API_URL`; `upgrade_gate.rs:220` writes `sessions.json` vs `session.json` | `AINC_*` product, `TURNKEEL_*` SDK; one `Config`; one `discovery()`; `AINC_API_URL` | xtask check on env names (D2, M1) |
| S10 | package `agentinc-os` in `ainc-mac`; `gpui-pilot-cli` binary named `gpui-pilot`; `agent-inc-*` header literal ×15; spellings AgentInc / `Agentinc OS` / `AgentInc Development` / `co.worldwidewebb.agentinc`; default workspace `'World Wide Webb'` hard-coded in a migration and `storage.rs` | package `ainc-mac`, binary `AgentInc`; headers as consts in identity; `Agentinc OS` only as the documented legacy data dir | AGENTS.md naming rule; grep check |
| S11 | `/v1/state`, `/v1/commands`, `Snapshot.todos`, `AutomationSnapshot.rules`, `WorkspaceState`, `Acknowledgement`, `/v1/tickets/contract` | resource paths; `XSnapshot`/`XReceipt`/`XCommandRequest`; Todo deprecated then deleted; stub deleted | OpenAPI lint (D2, D6) |
| S12 | ~100 runtime SQL strings, `Definition` SELECT ×4 | `query_as!` + offline data, or one column list per row struct | `cargo sqlx prepare --check` |
| S13 | NOTIFY channels and lock ids as literals in five files; `automations::snapshot` without a transaction | `pg::coordination` constants; `snapshot_tx()` | shared module (D6) |
| S14 | bigint, text-uuid and free-text ids mixed (`GENERATED BY DEFAULT` vs `ALWAYS`); `T-42` only in the app, "Ticket 4821" in a fixture | bigint for user-visible records, uuid text for execution ids; `ticket_key()` shared so CLI, tools, notifications say `T-42` | docs/how-to.md; shared helper |
| S15 | `status` (tickets, ExecutionView) vs `state` (turns, ticket_runs, conversation_sessions, occurrences, TerminalSession) | `status` for lifecycle; migrate on contact | migration checklist |
| S16 | `log` + `eprintln` in app (custom `log::Log` `main.rs:21–26`, 8 direct `eprintln!`), `tracing` ×5 in daemon with mixed casing, `println` in CLI/xtask; no unified logging | `tracing` everywhere; app subscriber → `os_log`; lower-case messages; structured fields | clippy disallowed-macros (B4) |
| S17 | `foo.rs + foo/` (mac tickets, shell, daemon tickets) and `mod.rs` (mac `ui/`, turnkeel `engine/`, `testing/`, xtask `checks/`, `release/`, `readme/`); 13 of 54 app files lack `//!` (shell.rs, evee.rs, temporal.rs, automations.rs, input.rs, main.rs, about.rs, shell/{main_content,layout,header,sidebar,pane}.rs, tickets/fixture.rs); only Tickets splits model/view/dialogs | `foo.rs + foo/`; every page dir has `model.rs`; `//!` everywhere; file named after its type (`evee.rs` holds `AssistantPage`; `assistant.rs` is the Codex adapter) | xtask `check-layout` |
| S18 | fake Codex app-server ×3 (`ainc-daemon/src/codex.rs:260`, `tests/process.rs:40` in Python, `ainc-xtask/src/readme/capture.rs`); test `ModelCatalog` ×3 (`execution.rs:345`, `conversations.rs:290`, `automations.rs:723`); `apply()` helpers ×4; two `round_trip.rs` | `ainc-daemon::testing` with one Rust Codex fixture and shared helpers | no-Python rule; shared module |
| S19 | tests spin on `yield_now` up to 200 iterations | LISTEN on NOTIFY channels or `reconcile()` step | AGENTS.md no-sleep rule extended to spins (D6) |
| S20 | page titles hard-coded (`tickets.rs:895`, `evee.rs:562`, `automations.rs:608`, `temporal.rs:311`, `tickets/agents.rs:21`); only Settings uses `route.label()` | `Page::title()` from `PAGES` | shared helper (M3) |
| S21 | Automations form is an inline card with a hand-rolled "Assign to" label (`automations.rs:320–325`) and chips where Tickets use a `Select`; Tickets forms are dialogs; pending label always "Saving…" (`tickets/dialogs.rs:89`) | forms via `OverlayHost` + `dialog_footer(verb)` with verb-following pending label ("Creating…", "Deleting…") | design-system.md; `dialog_footer` takes a verb enum (M3/M5) |
| S22 | ci.yml re-lists fmt/clippy/check-ui; `just test` is bash + Docker + `sleep` | CI calls `cargo xtask check` / `cargo xtask test`; Postgres bootstrap in xtask | ci.yml, justfile (B3) |
| S23 | no recipe for endpoint, page, tool, migration, CLI command; release split over README, AGENTS.md, phase-5-distribution, release-runner; `docs/GPUI_PILOT.md` and `docs/verification/GPUI_PILOT.md` duplicate; phase-* docs named by delivery order; `docs/releases/` stops at 0.3.4 (version is 0.5.0) | `docs/how-to.md` with seven recipes; topic-named docs (tickets.md, automations.md, distribution.md, runtime.md); one pilot doc; releases dir either maintained or deleted | X1 |
| S24 | CONTEXT.md lacks Agent, Workspace, Label, Turn, Activity, Owner/Principal, Work (ticket run); AGENTS.md:86 lists "seven constructs" (Agents, Knowledge, Inbox…) that do not match CONTEXT.md; "Session" means five things (SDK session, `conversation_sessions`, terminal sessions, Mac `model::Session`, gpui-pilot `transport::Session`, xtask `Session`); CONTEXT.md:3 itself uses "agentic" | add the terms; rename Mac `Session` → `UiState`; decide on "agentic"; align AGENTS.md's construct list | CONTEXT.md; xtask avoid-word check over UI strings and OpenAPI |

## Phase 3 · UI copy (U1)

### The style sheet (add to `docs/design-system.md`)

1. Title Case for buttons, menu items, dialog, page and section titles.
2. Sentence case for field labels, descriptions, hints, placeholders, empty-state bodies,
   toasts and errors.
3. Glossary constructs (Ticket, Comment, Conversation, Automation, Agent, Occurrence,
   Connection) are capitalized when they mean the construct.
4. Full sentences end with a period; titles, labels and buttons do not.
5. Always `…` (U+2026) and `’`; an action gets `…` only when it opens further input.
6. Verbs: New creates a top-level record, Add attaches to a record, Delete removes permanently,
   Remove detaches, Save commits an edit, Cancel dismisses a dialog, Close dismisses a panel.
7. Destructive confirm: title `Delete “{name}”?`, body `{What} will be permanently deleted.`,
   button `Delete`.
8. Shortcuts glyph-only, no separators: `⌘K` `⌘[` `⌘,` `⎋`, from the one table.
9. Errors: `{Thing} is unavailable. {Recovery}.`; raw detail behind a disclosure, never
   interpolated. Validation errors go to `Field::error`, load errors to `banner`.
10. Empty states: `No {things} yet.` plus one action, always through `EmptyState`.
11. British spelling: Cancelled.
12. People are round avatars, agents are rounded squares, everywhere.

Enforcement: `cargo xtask check-copy` scans string literals passed to `Button::new`,
`MenuEntry::new`, `PageHeader::new`, `EmptyState`, `dialog_shell` for `...`, straight
apostrophes, "Canceled", avoid-words, and case violations.

### Violations to fix (file:line at `1c9740d`)

- Capitalization: "New Conversation" (`evee.rs:550,578,1177`) vs "New conversation"
  (`evee.rs:1136`); "No Agents yet" (`tickets/agents.rs:37`) vs "No agents yet"
  (`automations.rs:330`); "Toggle Sidebar" (`main.rs:137`) vs "Toggle sidebar"
  (`shell/main_content.rs:6`, `shell/header.rs:87`); sentence-case buttons "Add label"
  (`tickets/labels.rs:158`), "Add relationship" (`tickets/detail.rs:679`, `dialogs.rs:67`),
  "Mark all read" (`shell/menus.rs:165`), "Run now" (`automations.rs:431`), "Stop work",
  "Clear filters", "Load more", "Sign out", "Rename conversation"/"Delete conversation"
  (`evee.rs:822,867`) beside "Check for Updates", "Check Now", "View Changelog", "Send
  Feedback", "Help Center", "Save Automation", "Edit Automation", "Rename Ticket"; lowercase
  constructs "Open ticket" (`ui/fuzzy.rs:108`), "agents, tickets and runs"
  (`shell/menus.rs:196`); "conversation" always lowercase while "Comment", "Workspace" are
  capitalized ("Add a Comment…", "Retry the unacknowledged Workspace change" in `storage.rs`).
- Ellipsis/apostrophes: "Downloading update..." (`native_update.rs:130`) vs "Downloading
  update…" (`updates.rs:360`); placeholders "Search Tickets" (`tickets.rs:200`) vs "Search…"
  (`components.rs:65`), "Ask Evee…", "Add a Comment…" (`tickets.rs:202`); "You're all caught
  up" (`shell/menus.rs:193`) vs "You’re up to date" (`updates.rs:326`).
- Spelling: "Canceled" (`temporal.rs:10`), "Download canceled" (`updates.rs:385`) vs
  "Cancelled" (`tickets/model.rs:32`).
- Verbs: "Connect ChatGPT" (`evee.rs:1200`) vs "Sign in with ChatGPT" (`evee.rs:320`); "Save"
  vs "Save Automation" vs submit "Create"; "Changelog" (`main.rs:121`) vs "View Changelog" vs
  "Release notes" (`updates.rs:540–542`); "Saving…" for Create and Delete
  (`tickets/dialogs.rs:89`).
- Confirms: Ticket `Delete “{title}”?` / "This Ticket, its Comments and its relationships will
  be removed." / Delete (`tickets/dialogs.rs:73–80`); Conversation `Delete “{title}”?` / "This
  permanently removes its messages from this Mac." / "Delete conversation"
  (`evee.rs:824,837,867`): "from this Mac" is false (conversations live in Postgres); gallery
  says "This Ticket and its Comments will be removed." (`components.rs`).
- Placeholders mix questions ("What needs doing?", "What should the agent do?"), imperatives
  ("Describe the work"), nouns ("Ticket title", "Name", "Rule name", "Connection default"), a
  value ("30").
- Empty states: "No X yet" ×6, "No matching X" ×2, "No matches", "No Tickets" (lane), "No other
  Tickets match." (period), "Start a conversation", "You're all caught up"; Assistant hand-rolls
  two (`evee.rs:1187–1206`, `1237–1250`), notifications hand-rolls one
  (`shell/menus.rs:181–199`); skeleton row counts 4/3/3/5 per page.
- Headers: Tickets and Automations detail put a small ghost "← Tickets"/"← Automations" in
  `PageHeader::leading`; the Assistant conversation uses a custom centred-title row with a
  regular-size ghost "Conversations" and no `PageHeader` (`evee.rs:1153–1186`). Refresh: text
  button (Temporal), icon-only (Automations), error-only (Assistant), none (Tickets, Agents).
- Vocabulary in UI: "Accounts & connections" (`shell/main_content.rs:172`, README:13), "Account
  menu" aria label (`sidebar.rs`), `AssistantPage.account` (`evee.rs:51`); "Rule name"
  (`automations.rs:97`, `components.rs:60`), "Edit rule" (`components.rs:456`), "Recurring
  rules…" (`automations.rs:609,611`), `rule_state`; "Run now"/"Run now deliberately replaces a
  missed firing" (`automations.rs:431,484`) for an Occurrence; "Runs" tab, "Attempt", "Agent
  run" (`tickets/detail.rs:839,887`, `temporal.rs:25`); `Navigation::Chat`, `show_chat`,
  `fixture_chat` (`evee.rs:46,81`, `shell.rs:199,510`), ids `new-chat`, `chat-menu`,
  `rename-chat`, `delete-chat`, `chat-history` (`evee.rs:564–1230`), "Connect ChatGPT to chat
  with Evee" (`evee.rs:1195`), design-system.md:81 "(chat, terminal)"; icon `"tasks"` for
  Tickets (`model.rs:90`, `tickets.rs:926`, `shell.rs:423`), README "Evee and Tasks"
  (`ainc-mac/README.md:53`), "task dialogs" (`:72`), `phase-2-ownership.md:20` "Tasks";
  `Relation::SubIssue` (`tickets/model.rs:233`), "sub-issue" (`daemon tickets/links.rs:2,19`,
  README, `board.rs` doc) vs UI "Sub-Ticket"; "Connection issue" (`evee.rs:329`); "current
  space tab contour" (`shell/header.rs:4`, `shell.rs:1170`, design-system.md:77), README:63
  "single tab"; "Search" (`sidebar.rs:61`, `Overlay::Search`) vs "Go to…" (README, Settings,
  palette placeholder) for ⌘K; "firstmate" in `docs/phase-5-distribution.md:68,163`;
  `crates/ainc-mac/AGENTS.md` points at `src/gallery.rs` (it is `src/components.rs`) and has
  two duplicate bullets (`:5–6`); `ButtonKind::Destructive` doc comment.
- Visual: Agents page round avatars for agents (`tickets/agents.rs`); Automations list icon
  "refresh" vs route/empty-state "repeat"; Blocked is a red glyph + keys on board cards
  (`board.rs` `card_footer`) but a red `status_pill("Blocked by T-1")` in the list; Delete
  Ticket hidden rather than disabled once runs exist (`tickets/detail.rs:121`); toasts used only
  for "Session could not be saved" (`shell.rs:579–594`), nothing schedules dismissal; error
  prefixes "Tickets unavailable:", "Automations unavailable:", "Data unavailable:"
  (`evee.rs:154`), "Workflow history unavailable:", "Change was not acknowledged:"
  (`tickets.rs:604`), raw `e.to_string()` (`evee.rs:216,425`, `automations.rs:213`), "Temporal
  is unavailable. Try again." (`temporal.rs:159`), "Companion daemon missing…"
  (`storage.rs:58`); Tickets overview banner lacks "Reconnecting…" while Agents (same entity)
  has it; Automations validation ("Enter an interval in whole minutes.") goes to the page banner
  (`automations.rs:244,249`).

## Phase 3 · Small fixes (F1 app, F2 daemon)

F1 (app): delete `Option<Arc<Store>>`/`storage_error`; collapse `pane::Side` and `[T;1]`;
delete `shell/layout.rs`; fix Destructive doc, README shortcut, AGENTS.md pointers; wire or
remove notifications; observe `update_required`; collapse `Navigate`/`Open`; trim objc2
features; feature-gate pilot dev-deps; stop cloning the ticket snapshot per keystroke
(`shell/palette.rs:96`, `palette_results` called from `keys`, `cycle_focus`, `command_palette`,
`choose_palette`) and the product snapshot per render (`tickets/detail.rs:815–829`,
`automations.rs:271`).

F2 (daemon/SDK/repo): delete `/v1/tickets/contract`, `save_result`, product wrappers,
`CodexModel::new`; middleware applied once; `conversations.rs:141,157` double lock;
`automations::snapshot` transaction; 401 vs 403 on `/v1/tickets`; `schema()` in `OnceLock`;
visibility client reuse; Rust Codex fixture replacing `tests/process.rs:40`; CLI self-identifies
as `cli/…`; legacy import done-marker; remove `crates/ainc-mac/scripts/__pycache__/`.

## Phase 3 · Libraries (L1)

Adopt: cargo-nextest, cargo-hakari, cargo-deny (`deny.toml`), cargo-machete, typos, taplo,
cargo sweep / sccache, utoipa-axum, tower-http, eventsource-stream, sqlx `query_as!` offline,
secrecy (owner token, access tokens), figment or envy (one config), command-group and
pty-process, reqwest rustls + ring provider.
Do not adopt: lld/mold/`-ld_new`, crate-splitting the app, release-plz, cargo-dist,
cargo-semver-checks, Tower hooks in turnkeel (parked by decision), community model SDKs
(decided: thin reqwest clients we own).

## Phase 3 · Docs (X1)

Stale claims to fix: `crates/ainc-mac/docs/verification/STATUS.md:9–23` and
`GPUI_UPGRADE.md:45,90` say `dist/Agentinc OS.app` (bundle.sh:15 builds `dist/AgentInc
Dev.app`); README shortcut table; `architecture.md:68` ("first Ticket endpoint only echoes"),
`:79` "four-status Tickets" vs `:44` six, `:17,50` SSE / typed errors / cursor pagination (none
exist); README.md:17 "three commands" vs AGENTS.md four; README "UI lint scripts"; AGENTS.md is
titled "# Turnkeel" but covers the repo; `docs/releases/` ends at 0.3.4.
Add `docs/how-to.md` with: add an endpoint (module + `routes!` + `cargo xtask generate` +
round-trip test + tool parity), add a page (Page impl + `PAGES` + rendered test), add a
component (existing design-system recipe), add a tool (`#[tool]` + daemon policy), add a
migration (`YYYYMMDDHHMMSS_name.sql`, expand/contract, `status` naming), add a CLI command
(operationId convention), cut a release. Rename phase-* docs by topic; merge the two
GPUI_PILOT.md; point AGENTS.md at how-to and at this plan.

## Done means

- `just check` under 5 s warm; `just test` green on nextest; `target/` under 15 GB.
- One Daemon module, one Sync, one Page trait, one time/state/shortcut/copy vocabulary in the
  app; `grep` checks in xtask for each standard pass.
- One receipt executor, one `CommandError`, one endpoint registry, one assignment fence, one
  Connection, one runner skeleton in the daemon; turnkeel's public interface names nothing from
  Temporal and nothing from the product.
- `CONTEXT.md` complete; no avoid-words in UI strings, OpenAPI or SDK names; `docs/how-to.md`
  answers every "how do I add X".
