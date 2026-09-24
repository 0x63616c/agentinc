# Neutral color tokens: GPUI Pilot evidence

The `before/` and `after/` directories contain direct GPUI Pilot captures at
1360 × 828 logical pixels. Each set covers Today, Tickets, Agents, Automations,
Home, Calendar, Library, My apps, Assistant, Settings, and command search.
The capture script is `../../../scripts/capture-color-pages.py`.

| Page | Before | After |
|---|---|---|
| Today | [before](before/today.png) | [after](after/today.png) |
| Tickets | [before](before/tickets.png) | [after](after/tickets.png) |
| Agents | [before](before/agents.png) | [after](after/agents.png) |
| Automations | [before](before/automations.png) | [after](after/automations.png) |
| Home | [before](before/home.png) | [after](after/home.png) |
| Calendar | [before](before/calendar.png) | [after](after/calendar.png) |
| Library | [before](before/library.png) | [after](after/library.png) |
| My apps | [before](before/my-apps.png) | [after](after/my-apps.png) |
| Assistant | [before](before/assistant.png) | [after](after/assistant.png) |
| Settings | [before](before/settings.png) | [after](after/settings.png) |
| Search | [before](before/search.png) | [after](after/search.png) |

The app used an isolated, unavailable daemon fixture and an unset local account
photo. These captures verify
the native shell and page appearance, including unavailable states. They do not
show populated Ticket or Agent data. The palette change is most visible on
keyboard focus, selected controls, text selection, and the Evee reply label;
some resting pages are pixel identical.

In the command search rectangle (logical pixels 420–940 × 80–585), the before
capture has 44 green-dominant pixels (`G > R + 7`, `G > B + 2`); the after
capture has none. The unchanged resting pages confirm this is a state-color
change rather than a layout or typography change. Both sets use the merged
default font-size setting.
