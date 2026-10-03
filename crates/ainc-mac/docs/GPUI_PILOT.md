# GPUI Pilot (Phase 1)

`gpui-pilot` is an opt-in library and `gpui-pilot-cli` builds the `gpui-pilot-cli` command. They contain no AgentInc navigation, storage or entity types. Both crates are unpublished. The app owns startup and accessible component declarations.

## Launch and use

Normal `cargo build` and `crates/ainc-mac/scripts/bundle.sh [release]` omit the library and its host feature. Normal app binaries reject automation arguments before opening a window. `automation` must be explicitly compiled **and** `--gpui-pilot-session ABSOLUTE_NEW_DIRECTORY` must be passed. An automation-enabled binary launched without this argument creates no driver endpoint and does not activate the semantic observer. Pilot sessions open an undisplayed, unfocused native Metal window and leave the current app frontmost. Pass `--gpui-pilot-visible` after the session directory only for checks that require an on-screen OS window, such as `tests/pilot_cli_smoke.rs`. Ordinary launches remain visible.

```sh
# Start cargo xtask dev first. This uses its isolated daemon and a fresh UI session.
crates/ainc-mac/scripts/pilot.sh "$PWD/.local/pilot-qa"
# In another terminal, use the explicit manifest printed at startup.
target/debug/gpui-pilot-cli --instance "$PWD/.local/pilot-qa/s/instance.json" snapshot
target/debug/gpui-pilot-cli --instance "$PWD/.local/pilot-qa/s/instance.json" --json snapshot
```

Commands: `hello`, `snapshot`, `click REF`, `press KEY`, `type REF --stdin`, `wait CONDITION_JSON [TIMEOUT_MS]`, `screenshot`. Keys use GPUI syntax, e.g. `cmd-k`, `enter`, `cmd-a`, `escape`. `type` inserts Unicode at the focused input's current selection through its normal GPUI input handler. Click the input first if it is not focused. There is no direct state-setting/fill endpoint. Read text from stdin to avoid putting it into process arguments.

```sh
printf 'Tickets' | target/debug/gpui-pilot-cli --instance "$PWD/.local/pilot-qa/s/instance.json" type 'REF_FROM_SNAPSHOT' --stdin
target/debug/gpui-pilot-cli --instance "$PWD/.local/pilot-qa/s/instance.json" wait '{"kind":"present","author_id":"tickets.create"}' 3000
```

Copy a **fresh** ref from the latest snapshot. Refs encode the random app session, explicit window, committed frame and AccessKit node ID. Any subsequent committed frame invalidates them, including animation frames. A `stale_ref` response means no input was dispatched: observe again. A timeout or broken connection after dispatch is uncertain; do not automatically replay a create/delete click. Phase 1 has no request replay cache.

Screenshots are full GPUI window content at the window's backing scale, captured using upstream `Window::render_to_image`. Responses identify frame, window, dimensions, scale and a PNG in the session directory. They exclude native title-bar/OS UI. Encoding and disk I/O run on the background executor; Metal render/readback runs on the UI thread. No arbitrary output path or file-read command is exposed. Phase 1 has no logical-resolution option or crop command.

## Protocol and safety contract

The source of truth is `crates/gpui-pilot/src/protocol.rs`. Transport is versioned newline-delimited JSON over a Unix socket, with one request per connection. The manifest identifies the owned process/window/session; the token is read from its private file, never passed on the CLI or printed. Server and client verify peer effective user IDs. Session directories are created exclusively with mode 0700; token/manifest/socket are 0600. Existing sessions are never reclaimed, even if apparently stale. A normal shutdown removes input/discovery endpoints and retains captures. A crash can leave files; choose a new directory.

The adapter reads typed AccessKit `TreeUpdate`s, not debug JSON. Snapshots retain parent refs, author IDs, roles, names/values, selected/checked/focus/disabled state, and logical bounds. They expose the currently rendered semantics; broad component coverage is deferred. Duplicate author IDs fail the snapshot with `ambiguous_target`. Password values are omitted and screenshots are refused while a password field is mounted. Use synthetic isolated data: ordinary assistant/Ticket text is intentionally observable.

Clicks revalidate the current committed frame, enabled state, the clipped semantic element hitbox and GPUI hit ownership, then dispatch move/down/up through GPUI. Press dispatches GPUI keydown/keymap handling and keyup. Type requires an editable focused node and commits through GPUI's platform input handler, preserving selection, change notifications and undo. Input is serialized by the foreground executor. Each action returns a newly committed frame with `path: gpui-input`; that is dispatch acknowledgment, not proof of a business outcome.

Wait predicates are exact author-ID `present`, `absent`, `value`, `name`, `focused`, or `frame_after`. They re-evaluate on requested GPUI frames, with a separate monotonic deadline. Concurrent waits do not hold the mutation lane. Failures include the latest snapshot when available. The small Phase 1 client supports one explicitly registered main window; there is no arbitrary existing-window attach/discovery.

Bounds: 64 KiB requests, 16 KiB typed text, eight concurrent socket requests, 32 queued foreground requests, 10-second waits, 12-second admission expiry, 4096 semantic nodes, 16-million-pixel captures, 64 screenshots per session. Socket read/write deadlines bound slow peers. Invalid authentication never reaches the foreground executor. There is no TCP listener, shell execution, database API or model runner.

## The pinned GPUI seam

Upstream commit `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` exposes event dispatch and capture but not local accessibility activation or typed committed frames. `vendor/gpui` is that core crate with a small feature-gated seam; all other Zed crates remain on the same git revision. `vendor/gpui-pilot.patch` is the reviewable source delta. The generated standalone manifest expands upstream workspace dependencies to the exact git revision. Apache-2.0 licensing is retained.

The optional `pilot` feature adds local accessibility activation independent of the OS adapter, retains the typed tree, associates semantic nodes with clipped GPUI hitboxes, increments a generation after the scene commit, and exposes the normal input-handler text path. It forces complete view rendering only for pilot-enabled windows so cached views cannot omit semantics. The renderer itself is unchanged. This is a bounded compatibility seam, not a published fork; remove it when equivalent public upstream APIs become available.

Reproduce from an unmodified checkout of the pinned Zed revision:

```sh
cargo xtask vendor-pilot-gpui /path/to/pinned/zed
```

## Validation

Linux CI checks formatting, default workspace Clippy and tests. Protocol/client tests have no GPUI dependency unless the library's `host` feature is enabled. Real rendered tests are explicitly gated and macOS-only.

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked -p ainc-mac -p gpui-pilot-cli --features ainc-mac/automation
cargo test --locked -p ainc-mac --features automation --test pilot_acceptance -- --nocapture
cargo test --locked -p ainc-mac --features rendered-tests --test rendered_shell
cargo test --locked -p ainc-mac --features automation --test pilot_cli_smoke -- --ignored --nocapture
```

`pilot_acceptance` launches the actual app executable with isolated UI, daemon discovery, import/profile and title variables and a fresh private driver session. Its only UI operations/assertions use the socket driver. It follows Search → Tickets → Add Ticket → Unicode title → Create, checks the resulting Ticket row, four statuses, assignee changes, Comments, agent registration, and Assistant new-conversation/list navigation; it exercises stale refs, overlay rejection, selection/undo, a concurrent wait and a deadline, and checks screenshot pixels in independent shell regions. Artifacts and small-sample latency distributions are written to `target/pilot-acceptance/`. The test process stops only its own child.

The separate CLI smoke opts into visibility for its native OS-window check.

The `rendered_shell` suite remains the broader capture-integrity gate. The OS acceptance run is recorded in the [acceptance section](#phase-1-acceptance) below; in-process tests do not prove native menus, OS prompts, screen-reader behavior or IME composition.

Out of scope: drag/hover/scroll commands, broad component coverage, MCP, Jev, model policy, multiwindow selection, stable refs across frames, request deduplication, Linux pixels, publishing and UI redesign.

## Phase 1 acceptance

Verified locally on 23 September 2026: Apple M2 Pro, macOS 27.0 (26A428), Rust 1.98.1. App and driver use the unoptimized dev profile (`debug = 0`), a single worktree target directory, and Zed revision `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` plus the reproducible optional seam in `vendor/gpui-pilot.patch`.

### Automated results

| Check | Observed result |
| --- | --- |
| Workspace format | Passed |
| Workspace nonvisual tests | 22 passed; one existing live-Codex test ignored |
| All-feature/all-target workspace Clippy | Passed with warnings denied |
| Default app launch gate | Automation flags rejected with exit 2; no endpoint directory created |
| Default app dependency tree | No `gpui-pilot` dependency |
| Actual-app socket acceptance | Search → Tasks → create `Pilot café 👋` passed |
| Failure/editing paths | Stale ref, obscured target, empty-submit disabled state, exact wait timeout, Unicode selection/undo, concurrent observer wait passed |
| Normal shutdown | Owned socket/token/manifest removed; regression asserted in acceptance test |
| Existing real Metal shell suite | 38 frames passed, including region-removal negative controls |
| CLI smoke | `hello`, `snapshot`, `press`, `type --stdin`, `wait`, `screenshot` passed |
| Pinned vendor regeneration | Byte-identical after regeneration |

Commands used from the task worktree (Cargo runs used `CARGO_HOME="$PWD/.local/cargo-home"` to reuse its existing upstream downloads):

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --workspace --features automation
cargo test --locked --features automation --test pilot_acceptance -- --nocapture
cargo test --locked --features rendered-tests --test rendered_shell
cargo tree --locked -p ainc-mac -e normal --depth 1
cargo test --locked --features automation --test pilot_cli_smoke -- --ignored --nocapture
cargo xtask vendor-pilot-gpui .local/cargo-home/git/checkouts/zed-a70e2ad075855582/4c902c9
```

The actual-app test launches its own executable, with distinct `AINC_SESSION_PATH`, `AINC_DATABASE_PATH`, `AINC_CODEX_HOME`, and `AINC_WINDOW_TITLE`. Every UI operation and business assertion uses the authenticated driver; there is no fixture-state setter or database read in the flow. The test removes its temporary data and waits for its own child to exit. Additional screenshots check independent header/sidebar/profile/Evee pixel regions.

Saved complete PNGs were inspected at 1360×828 logical resolution:

- [Initial Today](verification/gpui-pilot/initial.png)
- [Created task](verification/gpui-pilot/created.png)

The native shell retains its layout. The backdrop now explicitly occludes underlying GPUI hitboxes, matching its existing input-blocking behavior and allowing the driver to reject hidden pointer targets. Accessible annotations preserve switch, radio and checkbox state instead of treating every control as a button.

### Local latency sample

Fresh Unix socket connection per request, measured in Rust with `Instant`; no process/model startup in the samples. The warmed app had the single created task and a disconnected isolated Evee profile. Captures were 2720×1656 backing-resolution PNGs. Values are end-to-end round trips, including file encoding/writing for screenshots.

| Operation | Samples | Median | p95 |
| --- | ---: | ---: | ---: |
| Snapshot | 100 | 1.97 ms | 3.44 ms |
| Escape dispatch + committed frame | 50 | 12.03 ms | 14.13 ms |
| Retina PNG screenshot | 10 | 936.98 ms | 946.46 ms |

[Raw measurements](verification/gpui-pilot/latency.json). These are small unoptimized samples, not proof of the investigation's optimized performance targets. Escape mostly measures a no-op input plus full commit; task creation is separately asserted. Screenshot p95 at n=10 is the maximum sample. No isolated GPU-only timing, optimized build, logical-resolution capture, 1,000-node workload or long soak is claimed.

### Separate OS boundary check

`tests/pilot_os_acceptance.swift` queries WindowServer for the exact owned PID and unique QA title, then asserts one visible native window and reasonable dimensions. It does not synthesize desktop events. The CLI QA instance passed at **1360×828**, independently of GPUI's snapshot dimensions; [recorded result](verification/gpui-pilot/os-window.json).

```sh
swift tests/pilot_os_acceptance.swift OWNED_PID 'AgentInc Pilot CLI QA'
```

Native menus, system dialogs, screen-reader output and IME composition were not exercised in this driver task. Earlier native shell acceptance remains in [`verification/GPUI_UPGRADE.md`](verification/GPUI_UPGRADE.md); the in-process test is not a substitute for those OS checks. No new macOS CI runner is configured. Linux CI is configured to run nonvisual tests and all-feature compilation; the local results above are macOS results, not a claim of Linux execution or hosted CI success.

### Delivery limits

Phase 1 registers one explicit main window and invalidates refs on every committed frame. Animation can require a new snapshot; only an explicit `stale_ref` permits safe pre-dispatch retry. No deduplication cache exists for uncertain acknowledgments. Semantic coverage follows annotated shared controls/TextInput plus the task flow; this is not comprehensive settings/Evee accessibility acceptance. Password values are suppressed and capture is refused while a password field is mounted. Captures remain owned session artifacts, bounded to 64 per session. Release packaging, notarization, MCP, Jev, extraction/publishing, drag/scroll/hover, and Linux rendering are out of scope.
