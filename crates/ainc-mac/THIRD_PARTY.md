The native text input in `src/input.rs` is adapted from Zed Industries' GPUI 0.2.2 `examples/input.rs`, distributed under Apache-2.0. It retains the native text input, Unicode grapheme selection, clipboard and IME implementation with Control styling and small cursor/marked-range fixes.
Source: https://docs.rs/crate/gpui/0.2.2/source/examples/input.rs
License: https://www.apache.org/licenses/LICENSE-2.0

Outline icon paths in `assets/` originated in the imported Agentinc OS prototype. The Evee logo `assets/evee.png` is copied unchanged from the existing Evee project’s `web/public/evee-icon.png`, as requested in the implementation feedback.

`assets/AppIcon.png` and `.icns` retain the same Evee artwork on an opaque black canvas, allowing macOS to apply the rounded icon mask without exposing a light backing at the edge. They are bundled locally; the user's profile photo is read at runtime and is not an app asset.

The current GPUI core/platform/macOS/Apple sources are pinned to Zed commit `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` (Apache-2.0). Their upstream license declarations and notices remain in the dependency sources. See `docs/verification/GPUI_UPGRADE.md` for resolved versions and validation.

## Vendored GPUI pilot seam

`vendor/gpui` is the Apache-2.0 core GPUI crate from Zed revision `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8`, with the opt-in adapter changes recorded in `vendor/gpui-pilot.patch`. Its license is retained at `vendor/gpui/LICENSE-APACHE`; `scripts/vendor-pilot-gpui.py` reproduces the source and standalone manifest. Other GPUI platform packages remain pinned upstream git dependencies.
