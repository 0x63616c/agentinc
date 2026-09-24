# GPUI, with our own component library

The Mac app is written in Rust on GPUI (Zed's framework), with components we build ourselves. We rejected web-view shells (Tauri, Dioxus) because the app should be native Rust rather than HTML, iced and egui because they do not reach the design bar, and a SwiftUI shell because everything stays in Rust.
