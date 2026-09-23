The native text input in `src/input.rs` is adapted from Zed Industries' GPUI 0.2.2 `examples/input.rs`, distributed under Apache-2.0. It retains the native text input, Unicode grapheme selection, clipboard and IME implementation with Control styling and small cursor/marked-range fixes.
Source: https://docs.rs/crate/gpui/0.2.2/source/examples/input.rs
License: https://www.apache.org/licenses/LICENSE-2.0

Outline icon paths in `assets/` are taken from this repository's approved `.lavish/agentinc-os.js` reference. The Evee logo `assets/evee.png` is copied unchanged from the existing Evee project’s `web/public/evee-icon.png`, as requested in the implementation feedback.

`examples/gpui_text_probe.rs` derives from the same official GPUI 0.2.2 `examples/hello_world.rs` (Apache-2.0), with font comparison rows added for diagnosis.

`assets/AppIcon.png` and `.icns` retain the same Evee artwork on an opaque black canvas, allowing macOS to apply the rounded icon mask without exposing a light backing at the edge. They are bundled locally; the user's profile photo is read at runtime and is not an app asset.
