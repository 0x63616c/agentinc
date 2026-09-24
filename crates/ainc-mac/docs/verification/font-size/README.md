# Font size verification

These are full native GPUI Metal captures at 1160 × 728 logical pixels (2× backing scale), from isolated rendered fixtures. The Default scale adds two points to each original type size. The Larger scale applies the selected step to that new baseline.

| State | Capture |
| --- | --- |
| Settings, Default | [settings-default.png](settings-default.png) |
| Settings, Larger | [settings-larger.png](settings-larger.png) |
| Tickets, Larger | [tickets-larger.png](tickets-larger.png) |
| Assistant and Evee, Larger | [assistant-larger.png](assistant-larger.png) |

Both the Default and Larger 38-frame rendered suites passed at their tested window sizes. An automation-enabled signed bundle was also driven with GPUI Pilot: Settings exposed four radio choices, choosing Larger selected it immediately, and `session.json` saved `"font_size": "larger"`.
