# Control icons

A first set of 40 standalone SVGs for Control. The existing outline vocabulary is preserved from [the prototype](../../.lavish/agentinc-os.js), with a few matching utility additions. See [the contact sheet](contact-sheet.png) or open [its self-contained HTML](contact-sheet.html). These assets are not wired into the native shell.

## Drawing rules

- **Canvas:** `viewBox="0 0 24 24"`; intrinsic size 24 × 24. Usually keep geometry between 3 and 21, with optical overshoot to 2 or 22 for rays, arrows, and open forms. Keep strokes fully inside the canvas.
- **Stroke:** 1.5 units, round caps and round joins, no fill. Stroke scales with the icon; do not use `non-scaling-stroke`. The reference at 17 px therefore has a 1.0625 px stroke.
- **Corners:** 3-unit radius for containers, 4 for the task/checkbox pair, 1.5 for the four app tiles. Prefer simple paths and circles, generous internal gaps, and few details that survive at 14–17 px.
- **Optical size:** 17 px for sidebar rows, 14 px for tabs and compact controls, 20–24 px for larger destination cues. The sheet enlarges every glyph to 32 px for review and includes actual-size samples. Center by visual weight, not just path bounds.
- **Color:** `stroke="currentColor"`. Use `#a0a0a0` for resting icons, `#ededed` for emphasis/selection, `#888888` only for subdued decoration. Sage `#88b69b` and amber `#d6b77b` convey state and must have an accompanying label or indicator. Avoid multicolor glyphs, gradients, baked-in backgrounds, and decorative shadows.
- **Evee exception:** `evee.svg` preserves the reference's three inclined round-ended strokes; these use 3 units so the signature remains legible. Tint `#d5ded8`. It is an identity mark, separate from the panel visibility control. The native branch also has an Evee portrait; this library does not replace or alter it.
- **States:** reuse geometry and change foreground/context. Do not create a second arbitrary stroke weight for selected state. Hover, focus rings, notification dots, hit targets, and selected row fills belong to the control, not the SVG.

## Names and coverage

File names describe a purpose or familiar glyph. Reuse a glyph for related actions rather than adding aliases as duplicate files.

| Use / existing source name | Asset |
| --- | --- |
| Today, morning, weather / `sun` | `today.svg` |
| Tasks, completed task destination | `tasks.svg` |
| Agents / code | `agents.svg` |
| Home, Calendar | `home.svg`, `calendar.svg` |
| Library / `photos` | `library.svg` |
| My apps, workspace breadcrumb / `grid` | `apps.svg` |
| Evee identity | `evee.svg` |
| New tab, add | `plus.svg` |
| Close tab, dismiss | `close.svg` |
| Sidebar visibility | `sidebar-toggle.svg` (left divider) |
| Evee visibility / `panel` | `evee-panel.svg` (right divider) |
| Search, settings, profile, notifications | `search.svg`, `settings.svg`, `profile.svg`, `bell.svg` |
| Complete, unchecked | `check.svg`, `checkbox.svg` |
| Navigate, send, external destination | `arrow-right.svg`, `arrow-up.svg`, `arrow-up-right.svg` |
| Previous/next period, return hint | `chevron-left.svg`, `chevron-right.svg`, `return.svg` |
| Evening, focus, lighting, temperature | `moon.svg`, `coffee.svg`, `lamp.svg`, `temperature.svg` |
| Playback / `skip` | `play.svg`, `pause.svg`, `next-track.svg`, `volume.svg` |
| Run review, refresh / `reset` | `branch.svg`, `refresh.svg` |
| Music, reading, files, cloud | `music.svg`, `book.svg`, `folder.svg`, `cloud.svg` |
| Prototype assistant placeholder / `spark` | `spark.svg` (kept for compatibility; use `evee.svg` for the reference identity) |
| Overflow | `more.svg` |

Settings uses sliders so it cannot be confused with Today's sun. The native branch's current settings placeholder resembles the sun. Native macOS traffic lights, text/keyboard symbols, avatars, status dots, and album art remain platform controls, text, or content rather than library icons.

## Reuse and extension

Inline SVG inherits CSS `color`. An external `<img src="…svg">` does **not** inherit the containing page's color; inline it or use it as a tinted mask. For GPUI, use the app's SVG asset/tint path and verify rendering during the separate wiring task. The SVGs have no embedded accessible name: hide decorative SVGs from accessibility and label the enclosing button (for example, “Toggle sidebar”). Standalone meaningful images need a name supplied by the consuming component.

Add one SVG, following the rules above, then add its stem to `GROUPS` in `build-sheet.py`. This is the sheet's inventory; it checks that every SVG is represented exactly once and verifies the shared format. Run from the repository root:

```sh
python3 assets/icons/build-sheet.py
chrome-devtools-axi newpage "file://$PWD/assets/icons/contact-sheet.html"
chrome-devtools-axi resize 1440 1200
chrome-devtools-axi screenshot assets/icons/contact-sheet.png --full-page
```

Inspect the PNG and the 14/17 px samples before committing both review files. Match optical weight and spacing to the surrounding icons; a new icon should not introduce a different drawing system.

The sheet uses the native branch's workspace `#040404` and shell `#0c0c0c`, verified in `src/style.rs` at commit `dcdfa67f6284e80f0dbd609350059acca71ba739`. It also samples the original reference workspace `#101010`; that reference's shell is `#181818`. Colors remain properties of the consuming interface, never the SVG artwork.
