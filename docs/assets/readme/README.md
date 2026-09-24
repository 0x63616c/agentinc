# README hero capture

The single hero uses a retina GPUI Pilot render of the running signed AgentInc app. The app pixels, including its own header and layout, stay untouched. Screen Recording was unavailable in the capture host, so the macOS traffic lights and corner alpha are taken from the repository's [native window capture](../../../crates/ainc-mac/docs/verification/native-capture-library.png), at the same retina width and configured control position. The shadow is reconstructed from that exact corner mask. There is no added title bar. The bright gradient and generous padding follow the approach of the open-source [Tokokino screenshot composer](https://github.com/ShivaBhattacharjee/Tokokino), implemented here as a deterministic Pillow script.

On an Apple Silicon Mac, from a fresh disposable worktree:

1. Start the isolated stack with `cargo xtask dev`; wait for `cargo xtask doctor` to report its API URL.
2. Build the signed capture bundle with `AINC_CHANNEL=production crates/ainc-mac/scripts/bundle.sh automation`.
3. Run `python3 scripts/capture-readme-screenshots.py .local/readme-native.png`. It creates synthetic Tickets through GPUI Pilot in the isolated database, restarts the app to clear UI hover state, and captures the actual app frame. The app closes after capture; it never signs in or calls a model.
4. Run `python3 scripts/compose-readme-screenshots.py .local/readme-native.png docs/assets/readme/hero.png`. Requires Pillow. The script checks the retina frame dimensions, native-control background match and 1 MB limit.
5. Inspect the PNG for identity leaks, matching colors, native control placement, corners and shadow. The raw capture and isolated session stay under ignored `.local/`.

The two `AGENTINC_CAPTURE_*` identity overrides work only in the opt-in automation build. A normal bundle ignores them. After capture, run `crates/ainc-mac/scripts/bundle.sh` to restore the ordinary bundle before default-build tests.
