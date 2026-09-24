# Native UI polish round 2

The before images are native captures from the approved architecture and design review. The after images are exact-window captures of this branch's native bundle at 1360 × 828 logical pixels. The isolated QA run used the README's `AGENTINC_SESSION_PATH`, `AGENTINC_DATABASE_PATH`, `AGENTINC_CODEX_HOME`, and `AGENTINC_WINDOW_TITLE` overrides. No sign-in or model request was made.

| Surface | Before | After |
| --- | --- | --- |
| Signed-out Evee | [before](before-evee.png) | [after](after-evee.png) |
| Assistant list | [before](before-assistant.png) | [after](after-assistant.png), [precise date](after-assistant-menu.png) |
| Settings | [before](before-settings.png) | [after](after-settings.png) |
| Empty notifications | [before](before-notifications.png) | [after](after-notifications.png) |

The Assistant and Evee after images show the same native window because both surfaces are visible together. Native interaction checks created an empty conversation, confirmed its selected row persisted after pointer movement and route changes, edited an Evee draft, opened Settings, and confirmed the draft remained on return. The send action stayed dimmed while disconnected. The notification bell opened the short empty panel at the existing top-right anchor with no Mark all read action.

Validation: `cargo fmt --check`, `cargo test --locked` (13 passed, 1 integration test ignored by its installed-Codex requirement), `cargo clippy --locked --all-targets -- -D warnings`, `scripts/bundle.sh`, and `codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'`.

GPUI 0.2.2 does not expose the custom conversation rows in the macOS accessibility tree. The menu presents the full local timestamp as native text; VoiceOver access to custom GPUI controls remains a framework-level validation gap.
