# Control native shell

1. Bootstrap a small macOS Rust application with the published official GPUI 0.2.2 release, exact dependency pin and Cargo.lock. Use its shipped window, input and menu examples as API authority; package a normal Agentinc OS.app.
2. Build shared Control surfaces/icons and a separately testable navigation/session model. Wire destinations, persistent tabs, blank-tab picker, command search, keyboard navigation and panel visibility. Integration pages stay explicit placeholders.
3. Run model tests, formatting, Clippy and build. Launch the bundle, exercise real interactions, compare screenshots with the reference at matching logical dimensions, and record packaging commands and verification evidence.

API check: https://gpui.rs and https://github.com/zed-industries/zed/tree/main/crates/gpui/examples checked September 22, 2026. Main now uses gpui_platform; this increment deliberately pins the published 0.2.2 API with Application::new rather than a moving git branch. The downloaded release includes its matching examples.
