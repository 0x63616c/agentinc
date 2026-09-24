# App-owned UI core migration

The `src/ui/` module owns the native color, spacing, radius, size, type, button, field, list, Settings, overlay, and small display contracts. This moves the previous `style.rs`, `palette.rs`, `components.rs`, and `overlay.rs` helpers into one app-owned set. The Settings switch, segmented choices, and rows from the Settings redesign and the app identity tooltip retain their behavior. The named header, sidebar, and Evee optical insets remain in `src/ui/tokens.rs`.

The table links settled Metal captures from the same isolated `rendered_shell` fixture before and after the migration. Each frame is a full 2× backing-scale PNG. All 27 paired PNGs are byte-identical, so the consolidation made no visible layout or color changes in these fixtures. The suite also checks shared Button height, Field label alignment and gap, SettingsRow left/right insets, header icon centering, and pane content insets at both viewport sizes.

| Screen | 1360×828 before | 1360×828 after | 1160×728 before | 1160×728 after |
| --- | --- | --- | --- | --- |
| Today | [Before](before/1360x828/today.png) | [After](after/1360x828/today.png) | [Before](before/1160x728/today.png) | [After](after/1160x728/today.png) |
| Tickets | [Before](before/1360x828/tickets.png) | [After](after/1360x828/tickets.png) | [Before](before/1160x728/tickets.png) | [After](after/1160x728/tickets.png) |
| Calendar | [Before](before/1360x828/calendar.png) | [After](after/1360x828/calendar.png) | [Before](before/1160x728/calendar.png) | [After](after/1160x728/calendar.png) |
| Assistant | [Before](before/1360x828/assistant.png) | [After](after/1360x828/assistant.png) | [Before](before/1160x728/assistant.png) | [After](after/1160x728/assistant.png) |
| Agents | [Before](before/1360x828/agents.png) | [After](after/1360x828/agents.png) | [Before](before/1160x728/agents.png) | [After](after/1160x728/agents.png) |
| Automations | [Before](before/1360x828/automations.png) | [After](after/1360x828/automations.png) | [Before](before/1160x728/automations.png) | [After](after/1160x728/automations.png) |
| Home | [Before](before/1360x828/home.png) | [After](after/1360x828/home.png) | [Before](before/1160x728/home.png) | [After](after/1160x728/home.png) |
| Library | [Before](before/1360x828/library.png) | [After](after/1360x828/library.png) | [Before](before/1160x728/library.png) | [After](after/1160x728/library.png) |
| Apps | [Before](before/1360x828/apps.png) | [After](after/1360x828/apps.png) | [Before](before/1160x728/apps.png) | [After](after/1160x728/apps.png) |
| Settings | [Before](before/1360x828/settings.png) | [After](after/1360x828/settings.png) | [Before](before/1160x728/settings.png) | [After](after/1160x728/settings.png) |
| Ticket Dialog | [Before](before/1360x828/ticket-dialog.png) | [After](after/1360x828/ticket-dialog.png) | [Before](before/1160x728/ticket-dialog.png) | [After](after/1160x728/ticket-dialog.png) |
| Automation Fields | [Before](before/1360x828/automation-fields.png) | [After](after/1360x828/automation-fields.png) | [Before](before/1160x728/automation-fields.png) | [After](after/1160x728/automation-fields.png) |
| Search | [Before](before/1360x828/search.png) | [After](after/1360x828/search.png) | [Before](before/1160x728/search.png) | [After](after/1160x728/search.png) |

The explicitly requested app identity [tooltip before](before/1360x828/version-tooltip.png) and [tooltip after](after/1360x828/version-tooltip.png) are also byte-identical. The default fixture has no companion daemon, so list captures prove shared chrome and layout; populated Ticket, Automation, and Assistant interaction behavior is covered by Pilot acceptance. The native update offer and progress windows use AppKit and remain covered by their separate smoke test.
