# Settings refresh

`before.png` is the prior native Settings capture from `docs/verification/phase5/update-settings.png` at 1360×828. `after.png` is a GPUI Pilot capture of the signed automation bundle at the same 1360×828 logical size (2720×1656 Retina pixels). The after capture uses an isolated profile without a daemon, so the ChatGPT action is temporarily disabled while connection status is checked.

The shared controls live in `src/ui/selection.rs`:

- `settings_section`, `settings_row`, and `settings_divider` form grouped inset panels with a consistent row rhythm.
- `settings_switch` renders an accessible, keyboard-operable switch with a GPUI spring for knob movement. The owner keeps the boolean and persistence; pass its current value and toggle action.
- `settings_segments` and `settings_segment` render a compact single-choice control; `settings_button` and `settings_icon_button` cover row actions.

Settings uses those controls for font and size, update checks, download preferences, interval, and the ChatGPT connection. The update and account actions still call their original handlers.

Pilot interaction verified the switch's `Switch` role and checked state, pointer and Space activation, and the saved `updates/preferences.json` values for automatic checks, automatic download, and check frequency. The 38-frame Metal render suite passed at both window sizes.
