// Read-only WindowServer check for an owned hidden Pilot session.
import AppKit
import CoreGraphics
import Foundation

let args = CommandLine.arguments
if args.count == 2 && args[1] == "frontmost" {
    print(NSWorkspace.shared.frontmostApplication!.processIdentifier)
} else {
    precondition(args.count == 4, "Expected owned PID, exact title, and prior frontmost PID")
    let pid = Int(args[1])!
    let title = args[2]
    let prior = Int32(args[3])!
    let current = NSWorkspace.shared.frontmostApplication!.processIdentifier
    precondition(current == prior, "Pilot changed the frontmost application: \(prior) -> \(current)")
    precondition(current != pid, "Pilot became the frontmost application")
    let windows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
    let matching = windows.filter {
        ($0[kCGWindowOwnerPID as String] as? Int) == pid &&
        ($0[kCGWindowName as String] as? String) == title &&
        ($0[kCGWindowLayer as String] as? Int) == 0
    }
    precondition(matching.count == 1, "Expected one owned hidden native window")
    precondition((matching[0][kCGWindowIsOnscreen as String] as? Int) != 1, "Pilot window is on screen")
    print("Hidden Pilot window stayed off screen; frontmost PID \(current) unchanged")
}
