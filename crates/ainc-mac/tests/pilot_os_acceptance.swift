// Separate, read-only OS boundary check. No desktop input is synthesized.
// swift tests/pilot_os_acceptance.swift PID 'EXACT QA WINDOW TITLE'
import AppKit
import CoreGraphics
import Foundation

_ = NSApplication.shared
let args = CommandLine.arguments
precondition(args.count == 3, "Expected owned process PID and exact window title")
let pid = Int(args[1])!
let title = args[2]
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let matching = windows.filter {
    ($0[kCGWindowOwnerPID as String] as? Int) == pid &&
    ($0[kCGWindowName as String] as? String) == title &&
    ($0[kCGWindowLayer as String] as? Int) == 0
}
precondition(matching.count == 1, "Expected one visible native window with the exact owned PID/title")
let bounds = matching[0][kCGWindowBounds as String] as! [String: Double]
precondition(bounds["Width"]! >= 800 && bounds["Height"]! >= 600, "Native window is too small")
let result: [String: Any] = ["pid": pid, "title": title,
    "window_number": matching[0][kCGWindowNumber as String]!, "bounds": bounds,
    "check": "native WindowServer identity and visible dimensions"]
print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]), encoding: .utf8)!)
