# Resolved: native text rendering

GPUI 0.2.2 was built with `default-features = false` without `font-kit`, selecting its no-op text system. Enabling `font-kit` fixes the official hello-world probe and the shell on this Mac (macOS 27). The pinned version remains 0.2.2. See `gpui-font-fixed.png`.

Historical diagnostic notes follow; they no longer represent an open blocker.

# Native text rendering blocker — September 22, 2026

Branch: `fm/agentinc-os-control-shell`. Implementation remains uncommitted and incomplete; no PR was opened.

The app compiles and launches as a real GPUI macOS window. Icons, borders, surfaces, native traffic lights and Cmd+T work, but **all text is invisible**. Reproduced on Today and New tab, and again after adding diagnostic logging and relaunching. `blocked-text-rendering.png` captures the actual native window. No GPUI warnings/errors appeared in `.local/diagnostic.log`. Root cause is not established; do not infer a confirmed framework defect from this observation.

Environment: macOS 27.0 (26A428), Xcode 27.0 (27A266a), Rust 1.94.0, official crates.io GPUI exactly 0.2.2. Initial build required Metal Toolchain, which is absent; GPUI's supported `runtime_shaders` feature allowed the native Metal build to succeed without changing system tools.

Runnable artifact: `dist/Agentinc OS.app` (ad-hoc signed, debug). Build with `scripts/bundle.sh`.

Reproduce with isolated session data:

```sh
AGENTINC_SESSION_PATH="$PWD/.local/session.json" \
  'dist/Agentinc OS.app/Contents/MacOS/agentinc-os' > .local/diagnostic.log 2>&1
```

Implemented but not acceptance-verified: testable destination/tab/session model, native IME-capable input adapted from the official matching GPUI example, shared outline icons, connected tab shoulders, sidebar, Evee panel, placeholder destinations, keyboard shortcuts, switcher and bundle script. Model tests have been written but not run. Clippy, full interaction verification, matched-dimension fidelity comparisons, and ready-PR delivery remain outstanding.

The standalone browser reference was opened with chrome-devtools-axi and exercised: blank tab, filtering/Enter replacement, command search, sidebar toggle, and Evee toggle. Computed styles confirmed 178px sidebar, 48px titlebar, 258px Evee, 10px gap, 50px toolbar, 14px corners, and 28px/26px page padding. Native appearance has not passed the comparison gate.

Stopped under the assigned repeated-obstacle rule. Suggested next diagnostic: isolate text shaping/rasterization in the pinned official hello-world example, check macOS 27 compatibility and font selection, then decide whether to pin a newer official GPUI revision. Do not substitute another UI stack.

## Relaunch diagnostic — September 23, 2026 UTC

Isolation rechecked at `/Users/calum/.treehouse/agentinc-os-8663d6/1/agentinc-os`; branch and original uncommitted shell retained. The specified inbox was absent.

Two native probe observations now reproduce the failure independently of Control:

1. Copied the official **published GPUI 0.2.2** `examples/hello_world.rs` into `examples/gpui_text_probe.rs`, compiled and launched it. Colored boxes, borders and window controls render; its Hello World text does not.
2. Added separate Helvetica, Arial and `.AppleSystemUIFont` text rows. All are still invisible. `gpui-font-probe.png` is an actual screenshot of this second native window. This rules out the shell-specific font choice as the sole cause, but does not establish an OS or framework root cause.

Reproduction: `cargo run --locked --example gpui_text_probe`. The example retains the upstream Apache-2.0 provenance. A diagnostic bundle is also at `.local/GPUI Text Probe.app`.

An ignored copy of GPUI at `.local/gpui-probe` was instrumented at the CoreText shaping and rasterization boundaries and built using `--config 'patch.crates-io.gpui.path=".local/gpui-probe"'`. `.local/raster-probe.log` stayed empty despite the visible probe window. This instrumentation is **inconclusive**: verify that the instrumented implementation is actually linked/reached before inferring anything about glyph data. The root manifest was not changed, and Cargo.lock has been returned to the official registry GPUI 0.2.2 source/checksum. No shared registry source was modified.

Current official upstream was read from `https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/hello_world.rs` and `https://github.com/zed-industries/zed/blob/main/crates/gpui_macos/src/text_system.rs`. Main splits platform initialization into `gpui_platform`; the macOS implementation still uses CoreText and an alpha-only bitmap for monochrome glyph rasterization. No verified macOS 27 text-rendering fix or suitable replacement revision was identified. A newer pin has **not** been attempted or validated.

Stopped under the brief's two-attempt obstacle rule. Next diagnostic should verify the live shaping implementation and inspect glyph counts/bounds before selecting an official revision. Do not claim the existing app is usable or raise a ready delivery PR yet.

Follow-up feedback remains outstanding: resizable Evee divider, last-tab close guard (including allowing Today to close when other tabs remain), tab shoulder/border visual pass, and actual Evee logo. Located and visually inspected the existing white Evee mark at `/Users/calum/Documents/ChatGPT/evee/web/public/evee-icon.png`; it is a candidate source asset, not yet copied or wired in.
