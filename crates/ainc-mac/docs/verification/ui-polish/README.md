# Native UI polish verification

The before images are the supplied reference captures. The after images were captured from an isolated native QA bundle built from this branch, with a separate session and tasks database. The captures show the complete macOS window.

| Surface | Before | After |
| --- | --- | --- |
| Single tab, page title and profile | [tab](tab-plus.png), [breadcrumb](breadcrumb.png), [profile](profile.png) | [Tasks](after-tasks.png), [Settings](after-settings.png) |
| Tasks | [inline form and count](tasks-page.png) | [page](after-tasks.png), [add dialog](after-task-modal.png), [row menu](after-task-menu.png), [delete confirmation](after-delete-confirmation.png) |
| Notifications | [edge and spacing](before-notifications.png) | [floating panel](after-notifications.png) |
| Search selection | [green selection](space-picker-green.png) | [neutral grey selection](after-search.png) |

Native interaction checks: Add task with Return; open row menu; cancel and confirm deletion; dismiss a dialog by Escape and by clicking outside; clicking a sidebar destination behind the dialog dismisses it without navigation; switch fonts live and inspect the persisted `font` value in the isolated session. The old multi-tab session migration is covered by model tests.

Validation: `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `scripts/bundle.sh`, and `codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'`.
