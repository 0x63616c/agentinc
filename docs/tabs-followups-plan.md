# Tabs, cohesion follow-ups, and the next minor release

Requested by Calum on 2026-10-03. This is the durable execution checklist for
the tabs/sidebar request and the entire supplied cohesion follow-up handoff.
The goal is to implement, verify, integrate on `main`, push, and **publish the
next minor release**. Source main already contains the independent 0.7.0 release;
this work's next minor is **0.8.0**. Published Latest was still 0.6.0 at the audit.
Re-check the version before bumping; publication and the upgrade gate are part
of completion, not a later optional step.

## Execution order and ownership

1. Audit current code, `AGENTS.md`, `CONTEXT.md`, `docs/how-to.md`,
   `docs/cohesion-plan.md`, checks and their follow-up allow-lists.
2. Run daemon/API and build/SDK lanes independently. One Mac editing lane only;
   coordinate mechanical Mac API call-site edits from the daemon lane.
3. Integrate coherent commits, validate, and finish Mac follow-ups after API
   migrations; detail-aware navigation comes last.
4. Run complete integration, native visual and packaged-app acceptance.
5. Write release notes, bump minor on clean main, push and watch Distribution
   through successful publication and the native upgrade gate.

Workers use Sonnet and owned Treehouse leases; commit and report SHAs, without
pushing or releasing. The coordinator integrates, validates and pushes. Preserve
unrelated active branches, especially `feat/sparkle-updater`; reconcile any
updater architecture changes before modifying or releasing them. No PR unless
Calum requests one. Do not run a PR-creating validation pipeline by default.

## UI acceptance checklist

- [x] Multiple tabs, using the reference's curved top corners and inverse
  shoulders joining the rounded content card.
- [x] Right-hand X appears on tab hover, closes with the mouse without selecting
  a different tab accidentally, and remains keyboard accessible.
- [x] Cmd+T opens a tab; Cmd+Shift+W closes; Cmd+Shift+[ / ] moves left/right.
- [x] Shortcut handling also works while a native Ghostty pane is focused.
- [x] Tabs and their histories persist and restore safely from single-tab and
  older multi-tab files. Unknown/removed routes, invalid selection, empty files,
  closing first/last/active/inactive tabs and rapid repeated actions are covered.
- [x] Many tabs stay inside a bounded horizontal viewport. Active tabs reveal
  on selection, opening, closing and resize. Scrolling works with trackpad and
  mouse wheel; right/left boundaries and rounded shoulders do not bleed, clip
  incorrectly, overlap page corners or push controls outside the window.
- [x] Sidebar Search field and palette say **Search**, replacing **Go to…**.
- [x] Collapsed sidebar remains a thin icon strip: Search, Tickets, Assistant,
  remaining navigation icons and bottom profile avatar. Match expanded vertical
  positions and icon rail; retain usable hit targets and selected styling.
- [x] Expanded sidebar minimum width increases by at least 10% (150 → 176 px
  planned); resizing, persistence and animation remain correct.
- [x] White update-available control in the sidebar, including collapsed mode;
  adapt to AgentInc rather than copying the blue ChatGPT reference.
- [x] Profile menu is appropriately sized, fits labels at every font size, shows
  available updates, and Support opens alongside it without parent overlap.
- [x] Cmd+Shift+U checks for updates.

## Daemon/API lane — all handoff items

- [x] **D-2:** Remove deprecated CreateTodo/CompleteTodo/DeleteTodo adapters,
  Snapshot.todos if present, and test allow(deprecated); regenerate.
- [x] **D-3:** AutomationSnapshot.rules → automations; TerminalSession → Terminal,
  CreateTerminalSession → CreateTerminal, terminal_sessions_* → terminals_*.
  Settle Terminal as a domain noun; empty legacy schema/operation allow-lists.
- [x] **D-1:** Mac uses conversations_state/conversations_command. Delete
  /v1/state and /v1/commands, product CLI group and legacy operation exceptions;
  regenerate API/client/CLI. No product_state/product_command uses remain.
  Old endpoint operation IDs returning 409 on the new endpoint is accepted.
  D-1…D-3 worker commit `d9a57be` integrated as `6ae6ebc`; generated API and
  callers migrated together. Integrated SQL, API/client generation and tests pass.
- [x] **D-4:** Compile-check SQL with sqlx query!/query_as! and committed .sqlx
  offline metadata. One commit per module: tickets, automations,
  product/conversations, execution, workspaces, receipts, coding, terminal.
  No literal runtime queries except named/reasoned exceptions. Add prepare
  --check --workspace to check when DATABASE_URL exists (explicit skip otherwise),
  run in test, install sqlx-cli in CI and verify offline builds there.
- [x] **D-5:** Move terminal WebSocket attach to `ainc terminal attach <id>`;
  bundle/sign ainc, change Mac host, bundle script, Ghostty docs, pilot terminal
  test and xtask smoke; delete aincd argv dispatch. Shared identity discovery.
  Terminal smoke and pilot terminal acceptance pass.
- [x] **D-6:** One process-group/PTY helper owns spawn-in-group,
  kill-group-on-drop, setpgid/setsid/killpg/openpty. Migrate coding, terminal,
  local runtime and Codex safely; test cleanup and process lifecycles.
- [x] **D-7:** Shared ainc-client ticket_key(id) produces T-42 and is used by CLI,
  Conversation tools and Mac. Use clock_timestamp for event times. Rename
  lifecycle state to status only on tables otherwise touched by a migration,
  with documented expand/contract compatibility.
  Integrated checked-SQL modules in `cbd07e5`…`d12a236`, SQL gate `e91dcfd`,
  attach `691d8ee`, process ownership `6f011e1`, event times `7faf659`, shared
  keys/status `c281d9b`, singular CLI + key presentation `2696a6c`, compatible
  occurrence status `dcee012`, mandatory local test SQL gate `2feaf68`.
  Worker targeted audit tests: 99 daemon/CLI/client/identity and 93 xtask passed;
  isolated PG backfill and bidirectional old/new status writes covered. Integrated
  full tests and native terminal acceptance have passed (see below).

## Mac follow-up lane — after API integration

- [x] **M-1:** Automations Assign to uses Select with stable option IDs
  `automations.agent.{id}`; pilot test keeps working.
- [x] **M-2:** Monospace font follows an appearance choice or is derived from
  the selected font, rather than fixed FONT_MONO; all usages follow it.
- [x] **M-3:** Native update actions use shared Objective-C NS_ENUM instead of
  bare 1..7; Rust NativeAction value contract remains tested. Reconcile if the
  independent updater migration supersedes this code.
- [x] **M-4:** Audit/trim tokio runtime features (including blocking Codex login),
  objc2 feature use and GPUI/accesskit version matching; justify retained deps.
  M-1…M-4: 109 Mac tests pass, including agent selection/dismissal and update
  action ABI values. System uses SF Mono and Helvetica Neue uses Menlo. Native
  actions are named in `update_actions.h`. Trimming Objective-C defaults removed
  four unused 0.3.2 framework packages from the lockfile; Tokio retains concurrent
  HTTP/runtime coordination, with test helpers dev-only. AccessKit resolves once
  at 0.24.1 for both app and GPUI. `cargo check --offline -p ainc-mac` passed.
- [x] **M-5, last:** Route::Ticket(id)/Conversation(id), detail-aware Back/Forward
  and tab restoration, preserving legacy UI-state files.
  113 Mac tests pass. Real click-to-detail, Back/Forward, switching tabs and
  reopening preserve record ids; Conversation restoration waits for the matching
  snapshot. Fixed focusing an absent signed-out composer. Rendered matrix extended
  to long record titles: 211 Metal frames pass. Precise scroll assertions exposed
  duplicate wheel handling; the strip now uses GPUI's single native scroll path,
  with tests for both wheel/trackpad input and both hard boundaries.

## Build/SDK lane — all handoff items

- [x] **B-3:** engine/mod.rs and testing/mod.rs become sibling foo.rs; remove
  their layout follow-up exceptions.
- [x] **B-4:** Merge Temporal-heavy Turnkeel integrations into one binary;
  update recovery/effect worker self-reexec exact paths; recovery tests pass.
- [ ] **B-5:** Remove release workflow's unnecessary libssl-dev,
  protobuf-compiler and macOS protoc download; prove via test=true Distribution.
- [x] **B-6:** Install taplo-cli, format TOML and enforce installed taplo in CI.
  Worker `b89b721` integrated as `1573d17`; CI pins taplo 0.10.0.
- [x] **B-1:** Measure cold daemon build, workspace nextest --no-run and target
  size before/after hakari in a fresh lease. Adopt only with measured savings;
  report numbers even if rejected. If adopted: flat generated workspace-hack,
  pinned hakari, generated-file header and AGENTS note, check generation diff
  and manage-deps dry-run with fix command; just fix regenerates.
  Rejected: daemon cold build 228→260 s wall / 922→1357 s CPU / 1.9→3.0 GB;
  workspace no-run 408→310 s wall / 1952→1501 s CPU / 5.3→5.1 GB. Workspace
  savings did not justify daemon regression or production test-support features.
  Worker measured under high host load; see `docs/cohesion-plan.md` for caveats.
- [x] **B-2:** Measure target after full tests plus rendered run. Reduce below
  15 GB using measured contributors/profile/incremental improvements or provide
  a concrete explanation.
  Integrated `just test` plus the 211-frame rendered suite: coordinator target
  is 10 GB, with rendered images another 51 MB. No profile change required.
  Stale incremental artifacts from earlier feature combinations were cleared
  once before integration; the measurement includes rebuilt incremental state.

## Verification and release gates

- [x] Follow-up allow-lists in xtask checks are empty; actual behavior tests
  supplement the static standards. No source-string tests as behavior evidence.
- [x] Static checks and full tests pass with isolated Postgres. Docker was
  unavailable at audit; use Homebrew Postgres, unique ports and disposable data.
  Never import the real application profile. No added sleeps/timers in tests.
  `DATABASE_URL=postgres://postgres@127.0.0.1:54336/postgres just test`: 361
  passed, 8 skipped (plus doctests). SQL prepare against both scratch databases
  passed. Nextest's one leaky discovery report passed cleanly on isolated recheck.
- [x] Native rendered matrix: minimum/normal/large windows; minimum/default/max
  sidebar; expanded/collapsed; all font sizes; 1/2/many tabs; first/middle/last
  selection; wheel/trackpad; hover and mouse-close; reopening persisted state;
  profile/support with/without updates and long names. Assert geometry and
  inspect real Metal images at logical resolution.
  Final integrated run: 211 frames passed; native Swift ShortcutTests: 2 passed.
- [x] Terminal bridge shortcut tests, pilot native interactions, terminal attach
  smoke and relevant detail navigation tests pass.
  Pilot acceptance passed Tickets, Comments, assignments, six statuses, Automation
  creation/editing/run/history, detail Back and Assistant navigation. Packaged
  terminal split-session quit/relaunch passed. Reopening Search with a recent
  command exposed duplicate native IDs; palette rows now include their group in
  their identity, and real native acceptance covers reopening without a crash.
  Updated stale pilot selectors and development updater expectations for Sparkle.
- [x] Build bundled app and confirm an actual isolated window before releasing.
  Visible updater/live Sparkle fixtures belong on the dedicated release Mac,
  never Calum's active desktop. Test the package, not just Cargo binaries.
  Ad-hoc signed automation bundle passed visible CLI/native WindowServer smoke:
  PID/title matched, 1360×828 window, Search/type/wait/capture/quit and endpoint
  cleanup passed. Packaged terminal smoke passed for new and restored terminals.
  Full pilot acceptance exposed a hidden-window launch-animation wait; its real
  capture suite now uses a visible owned window and waits on launch status.
- [x] Review all changes; commit coherent validated steps. Preserve work before
  returning owned leases; never force-return dirty worktrees.
  Final local full gate repeated after the Search crash fix: 361 passed, no leaks,
  8 opt-in tests skipped; explicit native pilot suites passed separately. Release
  notes are prepared at `docs/releases/0.8.0.md`. Distribution dry-run and exact-SHA
  Linux CI remain required before publication.
  Integrated and pushed `6d28801b276ecddfbb20192c5c44e61807bb6080` to remote
  `main` by fast-forward, preserving the independent updater commits. CI run
  `37166504918`; explicit Distribution `test=true,build=true` run `37166520760`.
  Both are being watched. Final full-test/rendered target measurement: 11 GB.
- [ ] Write docs/releases/0.8.0.md (source main already at 0.7.0), run authorized
  `just release minor` on clean main, and push version commit.
- [ ] Watch Distribution prepare/native/distribute/upgrade/publish to success;
  verify GitHub Latest release, version and update feed. Do not claim shipping
  based only on a push or successful local build. Never replace a published
  version; failed unpublished drafts can be repaired/retried at an exact commit.

## Current checkpoint

- Original handoff baseline: local main `afa450e`, product version 0.6.0.
  Fresh leases actually start at `1323ec1`, which includes the now-integrated
  Sparkle updater and version 0.7.0. Remote main has advanced to `c6b68d7`.
  Reconcile remote changes before integration; next minor may therefore be 0.8.0.
- Coordinator: `feat/tabs-cohesion-followups`, lease 12,
  ID `0e55267c7a52d7694616e7ddea5b152c`; owns Mac changes and integration.
- Daemon Sonnet worker completed through `73bafe1`; all commits integrated,
  its Postgres stopped and lease 14 returned. Coordinator found and fixed remaining
  retired endpoint callers in runtime smoke and stale API docs; process tests now
  synchronize on readiness rather than sleeping. Remote main through `ad088d5`
  merged as `6fcbc67`, preserving current release fixes.
- Build Sonnet worker completed; lease 13 returned after integration. SHAs:
  `d5ce2fd`→`3dc5835`, `f3e9193`→`b3befe2`, `b89b721`→`1573d17`,
  `2e53ddc`→`f582c85`, `78fdcb9`→`aa25412`. Worker full test: 336 passed,
  8 skipped, including all 37 SDK integration tests and recovery re-execs.
  Fresh full-test target was 5.5 GB (3.6 GB with incremental disabled, rejected
  for dev-loop cost). Integrated rendered/full-test size and Distribution dry-run
  remain coordinator gates. Use lease-local targets: shared xtask artifacts can
  otherwise race across leases.
- Disk audit: headroom reached 6.9 GiB; removed only coordinator's inactive
  incremental cache (4.7 GB), recovering 11 GiB free. Lease-local target is used
  for gates. Integrated `just test` runs with isolated UTF-8 Postgres on 54336
  under temporary `opencode/pg-tabs-integration`.
- Tab persistence, bounded header, shortcuts/terminal forwarding, compact sidebar
  and menu placement implemented. Mac library tests: 107 passed. Native Swift
  ShortcutTests: 2 passed. Rendered suite: 187 real Metal frames passed across
  760/1024/1360 px windows, expanded/collapsed rails and every font size, including
  hover-X pixel checks. Fixed first-frame restoration (GPUI item reveal had stale
  overflow state) and inactive tab backgrounds masking the panel border. Inspected
  saved images at logical resolution. `just check` passed before API integration;
  later integrated evidence above supersedes these initial counts. No release yet.
- Worker outputs: temporary opencode daemon-followups-result.md and
  build-followups-result.md; transfer final evidence here when integrating.
