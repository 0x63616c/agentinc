# Agentinc OS

A native macOS workspace built with Rust and GPUI, following the approved Control design. This first increment implements the application shell: tabs, navigation, search, panel controls and a saved local session. Tasks, agents, home, calendar, library, apps and Evee have intentional placeholder pages; no external services are connected.

## Build and run

Requires macOS, Xcode command-line tools and Rust installed through rustup. `rust-toolchain.toml` selects Rust 1.94.0. GPUI is pinned to 0.2.2 and `Cargo.lock` fixes the dependency graph. Its `font-kit` and `runtime_shaders` features provide macOS text and Metal shaders.

```sh
scripts/bundle.sh
open 'dist/Agentinc OS.app'
```

The script builds and ad hoc signs a normal `.app` bundle. It is intended for local use, not notarized distribution. Use `scripts/bundle.sh release` for an optimized build. The verified build is the default debug bundle.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'
```

## Using the shell

Sidebar destinations replace the current tab. The plus button opens a focused, searchable blank tab. Choosing an already-open destination in the picker or Search activates it and removes the unused blank. Tabs scroll when space runs out; closing the active tab selects a neighbor, and the last tab cannot be closed.

| Shortcut | Action |
| --- | --- |
| Cmd+N / Cmd+T | New tab / space picker |
| Cmd+K | Search spaces |
| Cmd+W | Close current tab, keeping at least one |
| Cmd+[ / Cmd+] | Previous / next tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Cmd+Option+Left / Right | Back / forward within this tab |
| Cmd+1…7 | Today, Tasks, Agents, Home, Calendar, Library, My apps |
| Cmd+B | Toggle sidebar |
| Cmd+Shift+E | Toggle Evee |
| Escape | Dismiss Search or notifications |

Search supports arrow/Return selection, pointer selection, bounded Tab/Shift+Tab focus and standard Mac text editing. Drag Evee's left divider to resize it. The profile opens Settings, including working panel visibility controls. The notification bell describes its unconnected state.

Sessions save to `~/Library/Application Support/Agentinc OS/session.json`: destination tabs, active tab, per-tab history, panel visibility and Evee width. Blank tabs are discarded on restore; missing or invalid state safely starts on Today. The account name/photo is read locally at runtime and is not bundled.

For an isolated session without changing the regular app's state:

```sh
mkdir -p .local
AGENTINC_SESSION_PATH="$PWD/.local/test-session.json" \
  AGENTINC_WINDOW_TITLE='Agentinc QA' \
  'dist/Agentinc OS.app/Contents/MacOS/agentinc-os'
```

## Source and verification

- [Native acceptance report and screenshots](docs/verification/STATUS.md)
- [Scope and handoff](docs/CONTROL_BUILD_HANDOFF.md), [implementation plan](docs/CONTROL_IMPLEMENTATION_PLAN.md)
- `src/model.rs`: navigation and persistence, independent of the UI.
- `src/shell.rs`, `src/style.rs`: GPUI shell, shared styling and interactions.
- `src/input.rs`: native text input adapted from the official GPUI example.
- [Third-party sources](THIRD_PARTY.md), [resolved text-rendering diagnosis](docs/verification/BLOCKER.md).

## Original design study

The portable browser prototype remains in `.lavish/agentinc-os-standalone.html`; open it directly in a browser. Editable sources are `.lavish/agentinc-os.{html,css,js}`. `PRODUCT.md`, `DESIGN.md`, `.impeccable/design.json` and `docs/control-reference/` record its product and visual direction.

The browser's sample tasks, agents, controls, events and conversations are illustrative. Its concept selector and review toolbar are excluded from the native app. Browser interaction checks remain in `tests/preview-smoke.js`.
