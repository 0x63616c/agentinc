# AgentInc

A native macOS workspace built with Rust and GPUI. Its sidebar contains Tickets, Assistant, Agents, Automations and Terminal, with Settings in the footer. The Assistant provides full-page, Codex subscription-backed Evee conversations; there is no Today widget, Calendar page or Evee side pane.

## Build and run

Requires macOS 15 or later, Xcode command-line tools, Swift 6 and Rust installed through rustup. `rust-toolchain.toml` selects Rust 1.98.1. GPUI comes from Zed commit `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8`, pinned in `Cargo.toml` and `Cargo.lock`. Core/platform `font-kit` and platform `runtime_shaders` provide macOS text and Metal shaders. The bundle script also resolves the pinned GhosttyKit Swift package and stages its libghostty renderer, shell integration, terminfo and themes before signing. See `ghostty-bridge/README.md` for provenance and release packaging.

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

The development Dock/Finder icon is generated from `assets/AppIcon.png` with
`scripts/build-dev-icon.sh` (requires ImageMagick, librsvg and macOS `iconutil`).
The default border leaves Evee unobscured; pass `banner` to compare the angled
strip alternative. Commit the generated `assets/AppIconDev.icns` when changing
the production art so unbundled development runs show the same Dock icon.

Workspace CI runs formatting, Clippy and tests on Linux. Run the native rendered and pilot checks locally on macOS using the commands below and in `docs/GPUI_PILOT.md`.

The ordinary GPUI interaction tests run with `cargo test --locked`. On macOS, run the real Metal shell regression with:

```sh
cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell
```

It captures frames across every route, three window sizes, dialogs, Temporal states and full-page Evee conversations. It checks rendered shell regions and the shared page frame, content width and Settings/Automations right edges. Images go to `target/rendered-shell/`. This main-thread runner uses isolated test fixtures and is skipped on Linux; see [upgrade provenance and acceptance](docs/verification/GPUI_UPGRADE.md).

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
| Cmd+1…6 | Tickets, Assistant, Agents, Automations, Terminal, Temporal |
| Cmd+, | Settings |
| Cmd+B | Toggle sidebar |
| Escape | Dismiss Search, task dialogs or notifications |

Search supports arrow/Return selection, pointer selection, bounded Tab/Shift+Tab focus and standard Mac text editing. Drag the sidebar divider to resize it; focus it and use Left/Right in 20-point steps or Home to reset its width. The profile opens Settings, including persisted font family and size controls that update the whole app immediately. Default type is two points larger than the original Control scale. The notification bell opens an empty notification panel until notifications are connected.

Terminal hosts a live Ghostty session in your home directory. It loads your Ghostty configuration, including font, keybinds and included files, then applies AgentInc's colors. The session and split panes stay alive when you visit another page. With a Terminal pane focused, Cmd+D splits right, Cmd+Shift+D splits below, Cmd+W closes the focused pane when another exists, and Cmd+Shift+Enter or Cmd+Shift+= toggles a pane to fill the Terminal page. Cmd+K opens AgentInc Search without clearing the terminal; Cmd+, and Cmd+number retain their app navigation actions. Other Ghostty bindings, including Ctrl+L, remain available.

Development sessions save to `~/Library/Application Support/AgentInc Development/session.json`; installed production sessions retain `~/Library/Application Support/Agentinc OS/session.json`. See [channel isolation](../../docs/phase-5-distribution.md). Older multi-tab sessions restore retained destinations into the single space view; removed destinations and missing or invalid state start on Assistant. The account name/photo is read locally at runtime and is not bundled.

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
- `src/shell.rs`, `src/shell/`, `src/ui/`: GPUI shell and the app-owned native component set.
- `src/input.rs`: native text input adapted from the official GPUI example.
- [Third-party sources](THIRD_PARTY.md), [current GPUI acceptance](docs/verification/GPUI_UPGRADE.md).

## Opt-in GPUI automation

The unpublished `gpui-pilot` library and `gpui-pilot-cli` command provide typed snapshots, frame-scoped refs, GPUI input dispatch, condition waits and full Metal captures. Normal app/bundle builds exclude the driver. See [launch, protocol and validation](docs/GPUI_PILOT.md) for the explicit `automation` feature and isolated `scripts/pilot.sh` launch.
