// Find the capture app's actual macOS window without desktop input.
import AppKit
import CoreGraphics
import Foundation

_ = NSApplication.shared
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let owned = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid }
let candidates = owned.map { window -> [String: Any] in
    ["id": window[kCGWindowNumber as String]!,
     "title": window[kCGWindowName as String] as? String ?? "",
     "layer": window[kCGWindowLayer as String]!,
     "bounds": window[kCGWindowBounds as String]!,
     "on_screen": window[kCGWindowIsOnscreen as String] as? Bool ?? false]
}
let matching = candidates.filter {
    let bounds = $0["bounds"] as! [String: Double]
    return ($0["layer"] as? Int) == 0 && ($0["on_screen"] as? Bool) == true &&
        bounds["Width"]! >= 800 && bounds["Height"]! >= 600
}
guard matching.count == 1 else {
    fputs("Expected one on-screen native window. Owned windows: \(candidates)\n", stderr)
    exit(1)
}
print(String(data: try JSONSerialization.data(withJSONObject: matching[0]), encoding: .utf8)!)
