# Side pane layout evidence

These full-window PNGs came from GPUI Pilot's Metal capture of the ad hoc signed `bundle.sh automation` app, using an isolated UI session at 1360 × 828 logical points (2× backing scale). The isolated daemon discovery path had no service, so Tickets shows its unavailable/loading state; no live account or daemon behavior is claimed here.

- `default.png`: left 178 pt, right 258 pt.
- `minimum.png`: left 150 pt, right 258 pt; the workspace title ends in an ellipsis.
- `resized.png`: left 278 pt, right 298 pt.

Pilot clicked each divider and exercised its 20-point keyboard steps. After resizing, `Cmd+B` closed and reopened the left pane while its saved width stayed 278 pt. The focused GPUI mouse test covers actual dragging and persistence. The layout-bounds test renders shortcuts at 150 and 178 pt and checks that all nine badges share a right edge, labels end before them, and the title and badges stay within the sidebar. The 38-frame Metal shell suite passed separately.
