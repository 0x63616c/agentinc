// Read-only WindowServer evidence when the owned test window is not visible.
import AppKit
import CoreGraphics
import Foundation

let pid = Int(CommandLine.arguments[1])!
let title = CommandLine.arguments[2]
let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print("target_pid=\(pid) expected_title=\(title)")
print("on_console=\(session["kCGSessionOnConsoleKey"] ?? "unknown") " +
      "screen_locked=\(session["CGSSessionScreenIsLocked"] ?? "unknown")")
print("frontmost_pid=\(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1)")
print("main_display=\(CGDisplayBounds(CGMainDisplayID()))")
for (label, options) in [("on-screen", CGWindowListOption.optionOnScreenOnly),
                         ("all", CGWindowListOption.optionAll)] {
    let windows = CGWindowListCopyWindowInfo([options, .excludeDesktopElements],
                                              kCGNullWindowID) as? [[String: Any]] ?? []
    let owned = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid }
    print("\(label)_total=\(windows.count) owned=\(owned.count)")
    for window in owned {
        print("  title=\(window[kCGWindowName as String] ?? "") " +
              "layer=\(window[kCGWindowLayer as String] ?? "") " +
              "bounds=\(window[kCGWindowBounds as String] ?? "") " +
              "on_screen=\(window[kCGWindowIsOnscreen as String] ?? "")")
    }
}
