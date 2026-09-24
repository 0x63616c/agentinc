# README screenshots

The five PNGs here are captures of the native GPUI app at 1360 × 828 logical points (2720 × 1656 retina pixels). They show the current source with a fresh, synthetic worktree database. The Evee panel is shown before ChatGPT sign-in; no account or model transcript is fabricated.

To regenerate on an Apple Silicon Mac:

1. Use a new disposable worktree. Start its isolated services with `cargo xtask dev`; wait until `cargo xtask doctor` reports an API URL.
2. Build Pilot: `cargo build --locked -p agentinc-os -p gpui-pilot-cli --features agentinc-os/automation`.
3. Capture: `python3 scripts/capture-readme-screenshots.py .local/readme-raw`. The script checks that Tickets and Agents are empty, creates only demo records through GPUI Pilot, captures the five views, then pauses its demo Automation.
4. Composite: `python3 scripts/compose-readme-screenshots.py .local/readme-raw docs/assets/readme`. This requires Pillow and replaces the local workspace label and macOS account photo/name with demo labels. Only the composited PNGs are committed.
5. Inspect all five PNGs for personal data, image quality and the current controls. Each output must be under 1 MB.

The raw Pilot captures and its private session stay under ignored `.local/`. The capture script requires an empty database, so use a fresh disposable worktree for a later release. The scripts do not sign in to ChatGPT or invoke an agent.

After capture, run `cargo build --locked -p agentinc-os` to restore the normal binary before running default-build tests such as `cargo test`.
