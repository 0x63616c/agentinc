# Native end-to-end testing

[`scripts/e2e/run.py`](../scripts/e2e/run.py) launches a complete `.app` with a fresh
profile. The app starts its bundled `aincd`, Postgres 16 and Temporal on isolated
loopback ports. The test waits for the daemon's published discovery file and
`/health/ready`, exercises new and restored terminal attach processes through
real PTYs, and keeps native screenshots, snapshots and logs in its output
directory. It drains only the daemon it started. A failure leaves the evidence
directory in place.

On every pull request and push to main, the `native-e2e` CI job runs on
`homelab-mini` with Xcode 27. It builds the complete app bundle and replaces
only the test copy's app executable with the opt-in Pilot build, then ad hoc
signs that copy. Pilot drives every sidebar route and Components through Search,
checks real Metal frames, changes Settings, relaunches and confirms the saved
font size. The native window path runs a terminal command, reads its visible
output and drags a split divider. The updater's AppKit offer and progress
windows are also rendered. The job uploads `native-e2e-COMMIT` evidence even
on failure. The runner must have an active desktop session and Accessibility
permission for its runner process so it can send native Ghostty input.

Distribution downloads the **signed, notarized production archive** into its
macOS `upgrade` job and runs the same harness without Pilot. It checks startup,
runtime readiness, terminal attach, visible window, sidebar shortcuts, a
command visible in Ghostty, and split resizing. The existing
[`upgrade-gate.py`](../scripts/release/upgrade-gate.py) then checks the signed
installer and update path. The `publish` job needs that `upgrade` job to pass.
The release job uploads `signed-e2e-COMMIT` evidence. The shipping app never
exposes Pilot.

The Linux `rust` job keeps formatting, Clippy, workspace tests and generated
API checks. It cannot exercise Metal, AppKit or the signed bundle. The two Mac
jobs share a GitHub Actions concurrency group to protect the runner's persistent
Cargo target directory. All UI checks have bounded waits, and failure evidence
includes app, daemon, Postgres and Temporal logs plus screenshots.

## Add a feature flow

Add a function beside `pilot_flows` in `scripts/e2e/run.py` and call it from
there. Use a fresh Pilot snapshot for each click, an exact semantic author ID
or name, and a state/readback assertion after the action. Capture a frame with
`capture` so CI retains the rendered result. Keep the profile synthetic and
local: tests must not use a real account, model provider or external service.
For native AppKit or Ghostty input, extend `native_input.swift` and assert the
result through a screenshot or the persisted layout. The suite is sequential
because these flows share one app and one profile. Do not add fixed sleeps; use
Pilot conditions, daemon readiness, or file publication with a deadline.

Locally, use the same two builds as CI: `cargo xtask release --profile debug`,
then `cargo build --locked -p agentinc-os -p gpui-pilot-cli --features
agentinc-os/automation`. Copy the generated bundle into `target/app-e2e`,
replace `Contents/MacOS/AgentInc` with the automation binary, and ad hoc sign
the test copy. Run `python3 scripts/e2e/run.py --app
target/app-e2e/AgentInc.app --pilot-cli target/debug/gpui-pilot --output
target/app-e2e-evidence`. For a shipping bundle, omit `--pilot-cli`; the
production binary deliberately rejects automation arguments.
