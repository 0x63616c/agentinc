# GPUI Pilot (Phase 1)

`gpui-pilot` is an opt-in library and `gpui-pilot-cli` builds the `gpui-pilot` command. They contain no Agentinc navigation, storage or entity types. Both crates are unpublished. The app owns startup and accessible component declarations.

## Launch and use

Normal `cargo build` and `crates/ainc-mac/scripts/bundle.sh [release]` omit the library and its host feature. Normal app binaries reject automation arguments before opening a window. `automation` must be explicitly compiled **and** `--gpui-pilot-session ABSOLUTE_NEW_DIRECTORY` must be passed. An automation-enabled binary launched without this argument creates no driver endpoint and does not activate the semantic observer.

```sh
# Start cargo xtask dev first. This uses its isolated daemon and a fresh UI session.
crates/ainc-mac/scripts/pilot.sh "$PWD/.local/pilot-qa"
# In another terminal, use the explicit manifest printed at startup.
target/debug/gpui-pilot --instance "$PWD/.local/pilot-qa/s/instance.json" snapshot
target/debug/gpui-pilot --instance "$PWD/.local/pilot-qa/s/instance.json" --json snapshot
```

Commands: `hello`, `snapshot`, `click REF`, `press KEY`, `type REF --stdin`, `wait CONDITION_JSON [TIMEOUT_MS]`, `screenshot`. Keys use GPUI syntax, e.g. `cmd-k`, `enter`, `cmd-a`, `escape`. `type` inserts Unicode at the focused input's current selection through its normal GPUI input handler. Click the input first if it is not focused. There is no direct state-setting/fill endpoint. Read text from stdin to avoid putting it into process arguments.

```sh
printf 'Tasks' | target/debug/gpui-pilot --instance "$PWD/.local/pilot-qa/s/instance.json" type 'REF_FROM_SNAPSHOT' --stdin
target/debug/gpui-pilot --instance "$PWD/.local/pilot-qa/s/instance.json" wait '{"kind":"present","author_id":"tasks.create"}' 3000
```

Copy a **fresh** ref from the latest snapshot. Refs encode the random app session, explicit window, committed frame and AccessKit node ID. Any subsequent committed frame invalidates them, including animation frames. A `stale_ref` response means no input was dispatched: observe again. A timeout or broken connection after dispatch is uncertain; do not automatically replay a create/delete click. Phase 1 has no request replay cache.

Screenshots are full GPUI window content at the window's backing scale, captured using upstream `Window::render_to_image`. Responses identify frame, window, dimensions, scale and a PNG in the session directory. They exclude native title-bar/OS UI. Encoding and disk I/O run on the background executor; Metal render/readback runs on the UI thread. No arbitrary output path or file-read command is exposed. Phase 1 has no logical-resolution option or crop command.

## Protocol and safety contract

The source of truth is `crates/gpui-pilot/src/protocol.rs`. Transport is versioned newline-delimited JSON over a Unix socket, with one request per connection. The manifest identifies the owned process/window/session; the token is read from its private file, never passed on the CLI or printed. Server and client verify peer effective user IDs. Session directories are created exclusively with mode 0700; token/manifest/socket are 0600. Existing sessions are never reclaimed, even if apparently stale. A normal shutdown removes input/discovery endpoints and retains captures. A crash can leave files; choose a new directory.

The adapter reads typed AccessKit `TreeUpdate`s, not debug JSON. Snapshots retain parent refs, author IDs, roles, names/values, selected/checked/focus/disabled state, and logical bounds. They expose the currently rendered semantics; broad component coverage is deferred. Duplicate author IDs fail the snapshot with `ambiguous_target`. Password values are omitted and screenshots are refused while a password field is mounted. Use synthetic isolated data: ordinary assistant/task text is intentionally observable.

Clicks revalidate the current committed frame, enabled state, the clipped semantic element hitbox and GPUI hit ownership, then dispatch move/down/up through GPUI. Press dispatches GPUI keydown/keymap handling and keyup. Type requires an editable focused node and commits through GPUI's platform input handler, preserving selection, change notifications and undo. Input is serialized by the foreground executor. Each action returns a newly committed frame with `path: gpui-input`; that is dispatch acknowledgment, not proof of a business outcome.

Wait predicates are exact author-ID `present`, `absent`, `value`, `name`, `focused`, or `frame_after`. They re-evaluate on requested GPUI frames, with a separate monotonic deadline. Concurrent waits do not hold the mutation lane. Failures include the latest snapshot when available. The small Phase 1 client supports one explicitly registered main window; there is no arbitrary existing-window attach/discovery.

Bounds: 64 KiB requests, 16 KiB typed text, eight concurrent socket requests, 32 queued foreground requests, 10-second waits, 12-second admission expiry, 4096 semantic nodes, 16-million-pixel captures, 64 screenshots per session. Socket read/write deadlines bound slow peers. Invalid authentication never reaches the foreground executor. There is no TCP listener, shell execution, database API or model runner.

## The pinned GPUI seam

Upstream commit `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` exposes event dispatch and capture but not local accessibility activation or typed committed frames. `vendor/gpui` is that core crate with a small feature-gated seam; all other Zed crates remain on the same git revision. `vendor/gpui-pilot.patch` is the reviewable source delta. The generated standalone manifest expands upstream workspace dependencies to the exact git revision. Apache-2.0 licensing is retained.

The optional `pilot` feature adds local accessibility activation independent of the OS adapter, retains the typed tree, associates semantic nodes with clipped GPUI hitboxes, increments a generation after the scene commit, and exposes the normal input-handler text path. It forces complete view rendering only for pilot-enabled windows so cached views cannot omit semantics. The renderer itself is unchanged. This is a bounded compatibility seam, not a published fork; remove it when equivalent public upstream APIs become available.

Reproduce from an unmodified checkout of the pinned Zed revision:

```sh
python3 crates/ainc-mac/scripts/vendor-pilot-gpui.py /path/to/pinned/zed
```

## Validation

Linux CI checks formatting, default workspace Clippy and tests. Protocol/client tests have no GPUI dependency unless the library's `host` feature is enabled. Real rendered tests are explicitly gated and macOS-only.

```sh
cargo fmt --all --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked -p agentinc-os -p gpui-pilot-cli --features agentinc-os/automation
cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --nocapture
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
python3 crates/ainc-mac/tests/pilot_cli_smoke.py
```

`pilot_acceptance` launches the actual app executable with isolated UI, daemon discovery, import/profile and title variables and a fresh private driver session. Its only UI operations/assertions use the socket driver. It follows Search → Tasks → Add task → Unicode title → Create, checks the resulting task row, exercises stale refs, overlay rejection, selection/undo, a concurrent wait and a deadline, and checks screenshot pixels in independent shell regions. Artifacts and small-sample latency distributions are written to `target/pilot-acceptance/`. The test process stops only its own child.

The existing 38-frame `rendered_shell` suite remains the broader capture-integrity gate. Separate OS acceptance is recorded in `docs/verification/GPUI_PILOT.md`; in-process tests do not prove native menus, OS prompts, screen-reader behavior or IME composition.

Out of scope: drag/hover/scroll commands, broad component coverage, MCP, Jev, model policy, multiwindow selection, stable refs across frames, request deduplication, Linux pixels, publishing and UI redesign.
