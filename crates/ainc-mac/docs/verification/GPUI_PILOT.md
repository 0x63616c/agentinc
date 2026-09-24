# GPUI Pilot Phase 1 acceptance

Verified locally on 23 September 2026: Apple M2 Pro, macOS 27.0 (26A428), Rust 1.98.1. App and driver use the unoptimized dev profile (`debug = 0`), a single worktree target directory, and Zed revision `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` plus the reproducible optional seam in `vendor/gpui-pilot.patch`.

## Automated results

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
cargo tree --locked -p agentinc-os -e normal --depth 1
python3 tests/pilot_cli_smoke.py
python3 scripts/vendor-pilot-gpui.py .local/cargo-home/git/checkouts/zed-a70e2ad075855582/4c902c9
```

The actual-app test launches its own executable, with distinct `AGENTINC_SESSION_PATH`, `AGENTINC_DATABASE_PATH`, `AGENTINC_CODEX_HOME`, and `AGENTINC_WINDOW_TITLE`. Every UI operation and business assertion uses the authenticated driver; there is no fixture-state setter or database read in the flow. The test removes its temporary data and waits for its own child to exit. Additional screenshots check independent header/sidebar/profile/Evee pixel regions.

Saved complete PNGs were inspected at 1360×828 logical resolution:

- [Initial Today](gpui-pilot/initial.png)
- [Created task](gpui-pilot/created.png)

The native shell retains its layout. The backdrop now explicitly occludes underlying GPUI hitboxes, matching its existing input-blocking behavior and allowing the driver to reject hidden pointer targets. Accessible annotations preserve switch, radio and checkbox state instead of treating every control as a button.

## Local latency sample

Fresh Unix socket connection per request, measured in Rust with `Instant`; no process/model startup in the samples. The warmed app had the single created task and a disconnected isolated Evee profile. Captures were 2720×1656 backing-resolution PNGs. Values are end-to-end round trips, including file encoding/writing for screenshots.

| Operation | Samples | Median | p95 |
| --- | ---: | ---: | ---: |
| Snapshot | 100 | 1.97 ms | 3.44 ms |
| Escape dispatch + committed frame | 50 | 12.03 ms | 14.13 ms |
| Retina PNG screenshot | 10 | 936.98 ms | 946.46 ms |

[Raw measurements](gpui-pilot/latency.json). These are small unoptimized samples, not proof of the investigation's optimized performance targets. Escape mostly measures a no-op input plus full commit; task creation is separately asserted. Screenshot p95 at n=10 is the maximum sample. No isolated GPU-only timing, optimized build, logical-resolution capture, 1,000-node workload or long soak is claimed.

## Separate OS boundary check

`tests/pilot_os_acceptance.swift` queries WindowServer for the exact owned PID and unique QA title, then asserts one visible native window and reasonable dimensions. It does not synthesize desktop events. The CLI QA instance passed at **1360×828**, independently of GPUI's snapshot dimensions; [recorded result](gpui-pilot/os-window.json).

```sh
swift tests/pilot_os_acceptance.swift OWNED_PID 'Agentinc Pilot CLI QA'
```

Native menus, system dialogs, screen-reader output and IME composition were not exercised in this driver task. Earlier native shell acceptance remains in `GPUI_UPGRADE.md`; the in-process test is not a substitute for those OS checks. No new macOS CI runner is configured. Linux CI is configured to run nonvisual tests and all-feature compilation; the local results above are macOS results, not a claim of Linux execution or hosted CI success.

## Delivery limits

Phase 1 registers one explicit main window and invalidates refs on every committed frame. Animation can require a new snapshot; only an explicit `stale_ref` permits safe pre-dispatch retry. No deduplication cache exists for uncertain acknowledgments. Semantic coverage follows annotated shared controls/TextInput plus the task flow; this is not comprehensive settings/Evee accessibility acceptance. Password values are suppressed and capture is refused while a password field is mounted. Captures remain owned session artifacts, bounded to 64 per session. Release packaging, notarization, MCP, Jev, extraction/publishing, drag/scroll/hover, and Linux rendering are out of scope.
