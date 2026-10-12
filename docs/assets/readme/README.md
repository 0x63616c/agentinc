# README hero capture

The single hero uses a retina GPUI Pilot render of the running signed AgentInc app. The app pixels, including its own header and layout, stay untouched. Screen Recording was unavailable in the capture host, so the macOS traffic lights and corner alpha are taken from the repository's [native window capture](../../../crates/ainc-mac/docs/verification/native-capture-library.png), at the same retina width and configured control position. The shadow is reconstructed from that exact corner mask. There is no added title bar. The bright gradient and generous padding follow the approach of the open-source [Tokokino screenshot composer](https://github.com/ShivaBhattacharjee/Tokokino), implemented here as a deterministic Rust composer (`cargo xtask readme-compose`).

On an Apple Silicon Mac, from a fresh disposable worktree:

1. Start the isolated stack with `cargo xtask dev`; wait for `cargo xtask doctor` to report its API URL.
2. Build the signed capture bundle with `AINC_CHANNEL=production crates/ainc-mac/scripts/bundle.sh automation`.
3. Run `cargo xtask readme-capture .local/readme-native.png`. It seeds a few Todos and Events through the isolated daemon's owner API, opens the app on the Overview through GPUI Pilot and captures the actual app frame. The app closes after capture.
4. Run `cargo xtask readme-compose .local/readme-native.png docs/assets/readme/hero.png`. It checks the retina frame dimensions and the 1 MB limit, and retints the native controls to the app header.
5. Inspect the PNG for identity leaks, matching colors, native control placement, corners and shadow. The raw capture and isolated session stay under ignored `.local/`.

The `AINC_CAPTURE_PROFILE` name override works only in the opt-in automation build. A normal bundle ignores them. After capture, run `crates/ainc-mac/scripts/bundle.sh` to restore the ordinary bundle before default-build tests.
