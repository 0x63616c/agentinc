# README hero capture

The single hero is a real retina capture of the running signed AgentInc window. Its own title bar, inline controls, rounded corners, colors and macOS shadow are untouched. The only added pixels are the bright gradient outside the captured window. The composition follows the background and generous-padding approach of the open-source [Tokokino screenshot composer](https://github.com/ShivaBhattacharjee/Tokokino), implemented here as a deterministic Pillow script.

On an Apple Silicon Mac, from a fresh disposable worktree:

1. Start the isolated stack with `cargo xtask dev`; wait for `cargo xtask doctor` to report its API URL.
2. Build the signed capture bundle with `crates/ainc-mac/scripts/bundle.sh automation`.
3. Run `python3 scripts/capture-readme-screenshots.py .local/readme-native.png`. It creates synthetic Tickets through GPUI Pilot in the isolated database and uses `screencapture -l` on the owned native window. The app closes after capture; it never signs in or calls a model.
4. Run `python3 scripts/compose-readme-screenshots.py .local/readme-native.png docs/assets/readme/hero.png`. Requires Pillow. The script checks retina dimensions, native transparency and the 1 MB limit.
5. Inspect the PNG for identity leaks, the real controls, native corners and shadow. The raw capture and isolated session stay under ignored `.local/`.

The two `AGENTINC_CAPTURE_*` identity overrides work only in the opt-in automation build. A normal bundle ignores them. After capture, run `crates/ainc-mac/scripts/bundle.sh` to restore the ordinary bundle before default-build tests.
