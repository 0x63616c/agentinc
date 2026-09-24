# AgentInc workspace import verification

The app was imported from `0x63616c/agentinc-os` main at `49d1c5c9e80eb407b74336b28815cea3f958bf13`. Its 30-commit history was rewritten under `crates/ainc-mac` and merged as an unrelated history, then the pilot crates and vendored GPUI core were moved to their flat workspace locations. The source repository was not changed. `git log --follow -- crates/ainc-mac/src/shell.rs` traces into the imported history.

The app now displays **AgentInc** and uses bundle ID `co.worldwidewebb.agentinc`. Session, SQLite, and Codex profile paths intentionally remain under `~/Library/Application Support/Agentinc OS/` so existing local data is found. The storage and daemon migration belongs to the next phase 2 task.

Checks on macOS:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace` (including the app's interaction tests)
- `cargo test --locked -p agentinc-os --features rendered-tests --test rendered_shell` — 38 real Metal frames passed
- `cargo test --locked -p agentinc-os --features automation --test pilot_acceptance -- --nocapture` — real GPUI input, task creation, and captures passed
- `crates/ainc-mac/scripts/bundle.sh` and `codesign --verify --deep --strict --verbose=2 crates/ainc-mac/dist/AgentInc.app` passed

The ordinary bundle launched with `AGENTINC_SESSION_PATH`, `AGENTINC_DATABASE_PATH`, `AGENTINC_CODEX_HOME`, and `AGENTINC_WINDOW_TITLE` set to isolated worktree paths. It rendered in a native window. OS-level computer-use clicks returned `noWindowsAvailable` despite the visible window, so the native click-through used the **signed automation bundle** built with `crates/ainc-mac/scripts/bundle.sh automation`. GPUI Pilot clicked every main route, opened and dismissed Search and the task dialog, then created a task in the isolated SQLite database. The ordinary bundle was rebuilt afterward and rejected pilot flags as expected.

Captured pages: [Today](import-flow/today.png), [Tasks](import-flow/tasks.png), [Agents](import-flow/agents.png), [Home](import-flow/home.png), [Calendar](import-flow/calendar.png), [Library](import-flow/library.png), [My apps](import-flow/my-apps.png), and [Assistant](import-flow/assistant.png). Interactions: [Search open](import-flow/search-open.png), [task dialog](import-flow/task-dialog.png), and [created task](import-flow/task-created.png).
