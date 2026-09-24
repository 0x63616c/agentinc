# Own the native updater in Rust

Build a Rust updater with Sparkle feature parity: app-menu Check for Updates, update settings, a release-notes window, Install and Relaunch / Remind Me Later / Skip This Version, progress and full changelog. This keeps update behavior and app presentation under our control. Sparkle through `objc2` is a fallback if parity cannot be delivered safely.
