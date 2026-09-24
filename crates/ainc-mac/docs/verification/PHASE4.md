# Phase 4 native acceptance

Automations is a native GPUI page in the existing Control shell, with Cmd-9 navigation,
even control insets, the existing brief hover transitions, keyboard-operable controls
and no hover tooltips. Rule forms keep their opening revision across background
refresh, focus the name field on opening and restore page focus after saving.

The generated client keeps operation IDs for unacknowledged writes. The page shows
Pending until the rule revision is applied, and displays connection failures instead
of presenting an empty success state. Occurrences link to the normal Ticket detail;
missed and overlap counts remain in history. The initial form interval is 30 minutes.

## Evidence

- Workspace tests, strict Clippy including the automation feature, and contract
  generation verification.
- All 38 rendered Metal frames, including the existing capture-integrity negative
  controls, at 1360 × 828 and 1160 × 728 logical dimensions.
- The real `pilot_acceptance` flow exercises create, agent selection, save, edit to
  45 minutes, pause, Run now while paused, history-to-Ticket navigation, resume and
  pause again. It also retains all existing Tickets/Today acceptance coverage.
- A separate native window displays actual retained missed-firing counts after the
  isolated persistent scheduling server is stopped and restarted. An API-only
  harness over that same disposable database exposes a pending occurrence while
  the daemon and workers are stopped.

[Create rule](phase4/create.png), [history](phase4/history.png),
[linked Ticket](phase4/ticket.png), [missed firings and worker wait](phase4/missed-and-waiting.png)
are actual gpui-pilot captures at 1360 × 828 logical pixels. No browser or OS click
automation is used. The first three use the isolated loopback model fixture; the
last uses `ainc-daemon/examples/offline_workers.rs`, with an explicit fixture token.

Backend policy, recovery/effect tests and the outage setup are documented in
[phase-4 Automations](../../../../docs/phase-4-automations.md). Linux CI and the final
pushed revision remain verified on the PR, not inferred from these captures.

```sh
DATABASE_URL=... cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --features agentinc-os/automation -- -D warnings
cargo xtask generate --check
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
AINC_DISCOVERY_FILE=... cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --test-threads=1 --nocapture
```
