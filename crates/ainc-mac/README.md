# AgentInc

A native macOS workspace built with Rust and GPUI. Its sidebar contains Tickets, Assistant, Agents, Automations, Terminal and Temporal, with the user row in the footer; Settings opens from the user menu or ⌘K. The Assistant provides full-page, Codex subscription-backed Evee conversations; there is no Today widget, Calendar page or Evee side pane. The design system behind every screen is documented in [docs/design-system.md](../../docs/design-system.md) and rendered live on the **Components** page (⌘K, "Components").

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
'crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc' \
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
the production art. Launch the development app bundle to show this icon in the
Dock; the bundled icon receives macOS's app-icon shape.

Workspace CI runs formatting, Clippy and tests on Linux. Run the native rendered and pilot checks locally on macOS using the commands below and in `docs/GPUI_PILOT.md`.

The ordinary GPUI interaction tests run with `cargo test --locked`. On macOS, run the real Metal shell regression with:

```sh
cargo test --locked -p ainc-mac --features rendered-tests --test rendered_shell
```

It captures frames across every route, three window sizes, dialogs, Temporal states and full-page Evee conversations. It checks rendered shell regions and the shared page frame, content width and Settings/Automations right edges. Images go to `target/rendered-shell/`. This main-thread runner uses isolated test fixtures and is skipped on Linux; see [upgrade provenance and acceptance](docs/verification/GPUI_UPGRADE.md).

## Evee and Tickets

Install the official [Codex CLI](https://developers.openai.com/codex/cli), then open **Settings → Connections → Sign in with ChatGPT** and complete Codex’s browser sign-in. Evee uses your ChatGPT/Codex subscription; there is no API-key setup. Codex manages credentials in an app-specific profile. Connections shows the real sign-in state, Sign Out, and the model choices returned by Codex.

Open **Assistant** in the sidebar to start, reopen, rename or delete Conversations. Existing single-Conversation history migrates into “Previous conversation”. Send with ↵ or the composer’s arrow; ⇧↵ inserts a new line. Failed replies remain retryable and Conversations persist on the daemon, including when the app closes.

**Tickets** opens on a Kanban board with a lane per status (Backlog, To do, In progress, Blocked, Done, Cancelled). Drag cards between and within lanes; the order is saved. Switch to the grouped list, search, and filter by priority, label or assignee. A Ticket's page edits its description, status, priority, assignee and labels, relates it to other Tickets (blocks, relates to, duplicates, parent and Sub-Ticket), and shows its history, Comments, Work and the Conversation it came from. Create from any lane's + or from ⌘K, where Tickets are also found by title. Conversations, Tickets and product preferences are owned by `aincd` in Postgres. Its repeat-safe one-way import reads the previous SQLite database without migrating it in place. See [daemon ownership and setup](../../docs/ownership.md) and [native ownership verification](docs/verification/OWNERSHIP.md).

## Using the shell

The app retains one multi-thread Tokio runtime for concurrent HTTP requests and
update-drain coordination; GPUI background tasks call into it. Codex login runs
in the daemon, not on the app's UI thread. Tokio macros and test socket helpers
are dev-only. The direct Objective-C dependencies enable only the About panel's
`NSApplication`, `NSDictionary` and `NSString` surfaces; GPUI selects its own
platform features. The app and the pinned GPUI resolve one AccessKit version
(0.24.1 in the lockfile).

Sidebar destinations and Search navigate the active tab. Each tab retains its own Back/Forward history. Open tabs with ⌘T or +, switch with ⇧⌘[ / ⇧⌘], and close with ⇧⌘W or the X revealed on hover. The tab strip scrolls horizontally when full; opening or selecting a tab brings it into view. Closing the final tab leaves a fresh Assistant tab.

<!-- shortcuts -->
| Shortcut | Action |
| --- | --- |
| ⌘K | Search |
| ⌘[ | Back |
| ⌘] | Forward |
| ⌘T | New tab |
| ⇧⌘W | Close tab |
| ⇧⌘[ | Previous tab |
| ⇧⌘] | Next tab |
| ⇧⌘U | Check for Updates |
| ⌘1–6 | Tickets, Assistant, Agents… |
| ⌘, | Settings |
| ⌘B | Toggle sidebar |
| ⎋ | Dismiss |
| ↵ | Send |
| ⇧↵ | New line |
<!-- /shortcuts -->

Search (⌘K) fuzzy-matches pages, actions and Tickets, groups the results, remembers your recent choices, and supports ↑↓ and ↵ selection, pointer selection, bounded ⇥ and ⇧⇥ focus and standard Mac text editing. Drag the sidebar divider to resize it; focus it and use Left/Right in 20-point steps or Home to reset its width. Collapsing the sidebar leaves a compact icon rail. The profile menu offers updates, Settings and an adjacent Support menu (Help Center, Send Feedback, About). Available updates also appear in the sidebar. Settings holds persisted font family and size controls that update the whole app immediately. Transient notices appear as toasts above the status bar.

Terminal hosts a live Ghostty session in your home directory. It loads your Ghostty configuration, including font, keybinds and included files, then applies AgentInc's colors. The session and split panes stay alive when you visit another page; drag their dividers to resize them. With a Terminal pane focused, ⌘D splits right, ⇧⌘D splits below, ⌘W closes the focused pane when another exists, and ⇧⌘↵ or ⇧⌘= toggles a pane to fill the Terminal page. ⌘K opens Search without clearing the terminal; tab shortcuts, ⇧⌘U, ⌘, and ⌘1–6 retain their app actions. Other Ghostty bindings, including ⌃L, remain available.

Development UI state saves to `~/Library/Application Support/AgentInc Development/session.json`; installed production state retains `~/Library/Application Support/Agentinc OS/session.json`. See [channel isolation](../../docs/distribution.md). Single-tab and older multi-tab files restore retained destinations and history; missing or invalid state starts on Assistant. The profile name/photo is read locally at runtime and is not bundled.

For an isolated session without changing the regular app's state:

```sh
mkdir -p .local
AINC_SESSION_PATH="$PWD/.local/test-session.json" \
  AINC_DISCOVERY_FILE="$PWD/.local/dev/api-url" \
  AINC_WINDOW_TITLE='AgentInc QA' \
  'crates/ainc-mac/dist/AgentInc Dev.app/Contents/MacOS/AgentInc'
```

## Source and verification

- [Native acceptance report and screenshots](docs/verification/STATUS.md)
- [Workspace import and native flow evidence](docs/verification/IMPORT.md)
- `src/routes.rs`, `src/ui_state.rs`: navigation and persisted UI state, independent of the UI.
- `src/shell.rs`, `src/shell/`, `src/ui/`: GPUI shell and the app-owned native component set.
- `src/input.rs`: native text input adapted from the official GPUI example.
- [Third-party sources](THIRD_PARTY.md), [current GPUI acceptance](docs/verification/GPUI_UPGRADE.md).

## Opt-in GPUI automation

The unpublished `gpui-pilot` library and `gpui-pilot-cli` command provide typed snapshots, frame-scoped refs, GPUI input dispatch, condition waits and full Metal captures. Normal app/bundle builds exclude the driver. See [launch, protocol and validation](docs/GPUI_PILOT.md) for the explicit `automation` feature and isolated `scripts/pilot.sh` launch.
