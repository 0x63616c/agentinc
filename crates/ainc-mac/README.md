# Agentinc OS

A native macOS workspace built with Rust and GPUI, following the approved Control design. It includes a single tab, navigation, search, panel controls, a saved local session, SQLite-backed Tasks and an OpenAI-backed Evee chat panel. Agents, home, calendar, library and apps remain intentional placeholders.

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

## Evee and Tasks

Evee → Setup accepts an OpenAI API key and stores it in macOS Keychain. The default model is `gpt-5-mini`; the setup field can select another Responses-compatible model. Send with Return or **Send**. Failures remain retryable and chat history persists locally. An existing `OPENAI_API_KEY` environment variable takes precedence; normal Finder launches should use in-app setup.

Tasks supports create, complete/reopen and delete. Chat and tasks save to `~/Library/Application Support/Agentinc OS/assistant.sqlite3`; credentials are never stored there. See the [assistant handoff and verification](docs/EVEE_ASSISTANT_HANDOFF.md) for limits and the explicit live-credential verification gap.

## Using the shell

Sidebar destinations and Search replace the destination in the single tab. Back and Forward navigate its history.

| Shortcut | Action |
| --- | --- |
| Cmd+K | Search spaces |
| Cmd+Option+Left / Right | Back / forward |
| Cmd+1…7 | Today, Tasks, Agents, Home, Calendar, Library, My apps |
| Cmd+B | Toggle sidebar |
| Cmd+Shift+E | Toggle Evee |
| Escape | Dismiss Search, task dialogs or notifications |

Search supports arrow/Return selection, pointer selection, bounded Tab/Shift+Tab focus and standard Mac text editing. Drag Evee's left divider to resize it. The profile opens Settings, including a persisted font choice between System (SF Pro) and Helvetica Neue. The notification bell opens an empty notification panel until notifications are connected.

Sessions save to `~/Library/Application Support/Agentinc OS/session.json`: the single destination, its history, font choice, panel visibility and Evee width. Older multi-tab sessions restore the active destination into one tab; missing or invalid state safely starts on Today. The account name/photo is read locally at runtime and is not bundled.

For an isolated session without changing the regular app's state:

```sh
mkdir -p .local
AGENTINC_SESSION_PATH="$PWD/.local/test-session.json" \
  AGENTINC_DATABASE_PATH="$PWD/.local/test-assistant.sqlite3" \
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
