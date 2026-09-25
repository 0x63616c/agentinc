# Ghostty bridge

AgentInc embeds Ghostty's actual AppKit/Metal terminal view and libghostty PTY
backend through [GhosttyKit](https://github.com/Lakr233/libghostty-spm).
`Package.swift` and `Package.resolved` pin GhosttyKit 1.6.20260922 (revision
`b7f888e3baf8585ea9d590ab1a45b49475e00d1c`), whose prebuilt libghostty
comes from Ghostty revision `3c47ca159368eb4a860ffe5333abdf4a85b2767b`.
No Zig installation is needed for app or release builds. Swift 6 and macOS 15
are required by this package.

`Sources/AgentIncGhosttyBridge/Bridge.swift` owns the persistent terminal
controller, AppKit child views, split tree and terminal-only key handling. The
app's `src/terminal.rs` dynamically loads its C ABI from the signed bundle and
places the child view in `src/ui/terminal.rs`'s GPUI canvas. The bridge reads
the user's XDG and macOS Ghostty config files through `config-file` directives,
so includes, fonts and keybinds are parsed by Ghostty. AgentInc's UI color
roles are appended last, deliberately overriding terminal colors and themes.
The bridge intercepts Cmd+K, Cmd+, and Cmd+number only while a terminal pane is
focused, forwarding them to AgentInc. Its Swift test verifies the focus rule;
all other Ghostty keybinds pass through.
`swift test` also starts a process in a split pane, hides the terminal page,
releases the process through a FIFO, and verifies that the same pane and
process output remain after showing the page again.

`scripts/stage-ghostty.sh` builds the pinned Swift package and stages the
dylib, GhosttyKit resource bundle, license notices, and built-in themes in
both development and release bundles. GhosttyKit's bundle has shell integration
and terminfo but omits the built-in themes. The checked-in
`ghostty-themes-1.3.1.tar.gz` supplies 463 theme files from the official
Ghostty 1.3.1 macOS distribution, with SHA-256
`db4a90e531f62b83627bc7f7610ae3b73ba04ed46a2a73f49d93fa44f3683a7f`.
It lets Ghostty parse existing theme references even though AgentInc replaces
their color values. The MIT license notices are in `licenses/` and the app
bundle's Resources directory.
