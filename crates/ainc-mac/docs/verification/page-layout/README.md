# Page layout Pilot captures

Archived pre-trim evidence: Today and the Evee side pane shown here were removed in the Assistant full-page navigation change. These images are historical, not current acceptance targets.

These are full native Metal window captures from `gpui-pilot screenshot` at 2×
backing scale. Each page was opened through its keyboard route shortcut in an
isolated Pilot session. The companion daemon was unavailable in both sets, so
Tickets and Automations show their existing unavailable/loading states; the
captures compare the page chrome and header layout.

| Page | 1360×828 before | 1360×828 after | 1160×728 before | 1160×728 after |
| --- | --- | --- | --- | --- |
| Today | [Before](before/1360x828/today.png) | [After](after/1360x828/today.png) | [Before](before/1160x728/today.png) | [After](after/1160x728/today.png) |
| Tickets | [Before](before/1360x828/tickets.png) | [After](after/1360x828/tickets.png) | [Before](before/1160x728/tickets.png) | [After](after/1160x728/tickets.png) |
| Automations | [Before](before/1360x828/automations.png) | [After](after/1360x828/automations.png) | [Before](before/1160x728/automations.png) | [After](after/1160x728/automations.png) |

The 1160×728 Pilot launch uses `AGENTINC_PILOT_NARROW=1`; ordinary app launches
retain their existing window size. The old `toggle-evee` observations below
the pre-trim baseline no longer apply to the current app.
