# Native token comparison

Captured from the native `Agentinc OS.app` at 1360 × 828 with isolated route session files and exact window IDs. The before bundle was built from `60fc662`; the after bundle was built from this branch. Each pair uses the same route, sidebar, Evee width, font, and local storage state.

| Surface | Before | After |
|---|---|---|
| Today | [PNG](before-today.png) | [PNG](after-today.png) |
| Tasks | [PNG](before-tasks.png) | [PNG](after-tasks.png) |
| Assistant | [PNG](before-assistant.png) | [PNG](after-assistant.png) |
| Settings | [PNG](before-settings.png) | [PNG](after-settings.png) |

An RGB pixel comparison found zero changed pixels in the main panel (`x=180..1079, y=100..699`), upper Evee rail (`x=1090..1359, y=100..699`), and macOS window controls (`x=0..99, y=0..64`) for all four pairs. The disabled Evee send icon changed at 878 pixels in its `70 × 70` comparison region because disabled opacity is now shared at `0.45` (Assistant previously used `0.4`). Selected conversation rows now use `#252525` instead of `#242424`; the empty Assistant capture does not display a row. Keyboard focus also uses the shared focus treatment and is outside the resting captures.

The GPUI 0.2.2 macOS accessibility tree remained limited to the window and menu bar during inspection, so the semantic labels supplied to the shared button interface are not claimed as VoiceOver exposure.
