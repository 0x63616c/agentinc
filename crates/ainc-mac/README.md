# AgentInc

A native macOS workspace built with Rust and GPUI, following the approved Control design. It includes a single tab, navigation, search, panel controls, a saved local session, daemon-backed Tasks and a Codex subscription-backed Evee chat panel and an Assistant conversation library. Agents, home, calendar, library and apps remain intentional placeholders.

## Build and run

Requires macOS, Xcode command-line tools and Rust installed through rustup. `rust-toolchain.toml` selects Rust 1.98.1. GPUI comes from Zed commit `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8`, pinned in `Cargo.toml` and `Cargo.lock`. This is current upstream source, although Zed still labels its core crate 0.2.2. Core/platform `font-kit` and platform `runtime_shaders` provide macOS text and Metal shaders.

```sh
crates/ainc-mac/scripts/bundle.sh
open 'crates/ainc-mac/dist/AgentInc Dev.app'
```

Run commands from the workspace root. The script builds and ad hoc signs a normal `.app` bundle. Use `crates/ainc-mac/scripts/bundle.sh release` for an optimized build.
For isolated native automation, `crates/ainc-mac/scripts/bundle.sh automation` builds the opt-in pilot variant; run the script without arguments afterward to restore the ordinary bundle.

Verify a built bundle with `codesign --verify --deep --strict --verbose=2 'crates/ainc-mac/dist/AgentInc Dev.app'`.

The updater uses AppKit windows for the release offer and download progress. To smoke
both windows with the checked-in test manifest, without downloading or installing:

```sh
crates/ainc-mac/scripts/bundle.sh automation
'crates/ainc-mac/dist/AgentInc.app/Contents/MacOS/agentinc-os' \
  --update-ui-smoke crates/ainc-mac/tests/fixtures/update-manifest.json \
  target/native-update-smoke
```

`offer.png` and `progress.png` are captures of each window's own AppKit content
view, so Screen Recording permission is not needed. The smoke path is available
only in the opt-in automation build. Rebuild without arguments to restore the
ordinary bundle.

## Development setup

Workspace CI runs formatting, Clippy and tests on Linux. Run the native rendered and pilot checks locally on macOS using the commands below and in `docs/GPUI_PILOT.md`.

The ordinary GPUI interaction tests run with `cargo test --locked`. On macOS, run the real Metal shell regression with:

```sh
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
```

It captures 38 frames across routes, two window sizes, dialogs and Evee visibility, and asserts that shell regions contain rendered pixels. Images go to `target/rendered-shell/`. This main-thread runner uses isolated test fixtures and is skipped on Linux; see [upgrade provenance and acceptance](docs/verification/GPUI_UPGRADE.md).

## Evee and Tasks

Install the official [Codex CLI](https://developers.openai.com/codex/cli), then open **Settings → Accounts & connections → Sign in with ChatGPT** and complete Codex’s browser sign-in. Evee uses your ChatGPT/Codex subscription; there is no API-key setup. Codex manages credentials in an app-specific profile. Settings shows the real connection status, sign out, and model choices returned by Codex.

Open **Assistant** in the sidebar to start, reopen, rename or delete conversations. Existing single-chat history migrates into “Previous conversation”. Send with Return or the composer’s arrow; Shift+Return inserts a newline. Failed replies remain retryable and conversations persist on the daemon, including when the app closes.

Tasks supports create, complete/reopen and delete. Conversations, Tasks and product preferences are owned by `aincd` in Postgres. Its repeat-safe one-way import reads the previous SQLite database without migrating it in place. See [daemon ownership and setup](../../docs/phase-2-ownership.md) and [native ownership verification](docs/verification/OWNERSHIP.md).

## Using the shell

Sidebar destinations and Search replace the destination in the single tab. Back and Forward navigate its history.

| Shortcut | Action |
| --- | --- |
| Cmd+K | Search spaces |
| Cmd+Option+Left / Right | Back / forward |
| Cmd+1…9 | Today, Tickets, Calendar, Assistant, Agents, Automations, Home, Library, My apps |
| Cmd+, | Settings |
| Cmd+B | Toggle sidebar |
| Cmd+Shift+E | Toggle Evee |
| Escape | Dismiss Search, task dialogs or notifications |

Search supports arrow/Return selection, pointer selection, bounded Tab/Shift+Tab focus and standard Mac text editing. Drag either side pane's divider to resize it; focus a divider and use Left/Right in 20-point steps or Home to reset its width. The profile opens Settings, including persisted font family and size controls that update the whole app immediately. Default type is two points larger than the original Control scale. The notification bell opens an empty notification panel until notifications are connected.

Development sessions save to `~/Library/Application Support/AgentInc Development/session.json`; installed production sessions retain `~/Library/Application Support/Agentinc OS/session.json`. See [channel isolation](../../docs/phase-5-distribution.md). Older multi-tab sessions restore the active destination into the single space view; missing or invalid state safely starts on Today. The account name/photo is read locally at runtime and is not bundled.

For an isolated session without changing the regular app's state:

```sh
mkdir -p .local
AGENTINC_SESSION_PATH="$PWD/.local/test-session.json" \
  AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" \
  AGENTINC_WINDOW_TITLE='Agentinc QA' \
  'crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc'
```

## Source and verification

- [Native acceptance report and screenshots](docs/verification/STATUS.md)
- [Workspace import and native flow evidence](docs/verification/IMPORT.md)
- `src/model.rs`: navigation and persistence, independent of the UI.
- `src/shell.rs`, `src/shell/`, `src/style.rs`: GPUI shell, its header/content/pane views, shared styling and interactions.
- `src/input.rs`: native text input adapted from the official GPUI example.
- [Third-party sources](THIRD_PARTY.md), [current GPUI acceptance](docs/verification/GPUI_UPGRADE.md).

## Opt-in GPUI automation

The unpublished `gpui-pilot` library and `gpui-pilot-cli` command provide typed snapshots, frame-scoped refs, GPUI input dispatch, condition waits and full Metal captures. Normal app/bundle builds exclude the driver. See [launch, protocol and validation](docs/GPUI_PILOT.md) for the explicit `automation` feature and isolated `scripts/pilot.sh` launch.
