# Ticket and Automation spacing pilot

These are settled native Metal captures from `rendered_shell`, at 2× backing scale. The fixture has no companion daemon, so the screenshots compare control layout and page chrome rather than live Ticket or Automation data. The before frames were captured after adding test selectors and before changing the spacing contract.

| Page | Width | Before | After |
| --- | --- | --- | --- |
| Ticket dialog | 1360×828 | [PNG](before/1360x828/ticket.png) | [PNG](after/1360x828/ticket.png) |
| Ticket dialog | 1160×728 | [PNG](before/1160x728/ticket.png) | [PNG](after/1160x728/ticket.png) |
| Automation editor | 1360×828 | [PNG](before/1360x828/automation.png) | [PNG](after/1360x828/automation.png) |
| Automation editor | 1160×728 | [PNG](before/1160x728/automation.png) | [PNG](after/1160x728/automation.png) |

The Ticket label gap changes from 6 to 8 logical px and its button horizontal inset from 10 to 12 px, matching Automation. Both keep the existing `CONTROL_HEIGHT` value of 32. The shared field input inset remains 12 px. The header search's 9/4 px, sidebar identity's 6.5/6 px, and Evee header's 10/9 px optical insets are named in `style.rs`; the concurrent sidebar lane retains ownership of its call site.

The Metal suite now checks five layout relationships at both widths: header icon centering, Ticket field label/input edge and gap, paired Automation field edges and gaps, main content inset, and right content inset. It reads bounds only after settled frames, with a 1 logical px tolerance for Metal rounding. The source ratchet in `scripts/check-ui-spacing.py` emits CI warnings for new raw spacing calls in the migrated Header, Ticket, Automation and right-pane files.
