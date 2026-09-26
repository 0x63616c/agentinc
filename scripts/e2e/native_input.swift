// Native AppKit child views are outside Pilot's GPUI input and screenshot tree.
// This tool sends actual window events to the one app PID owned by the test.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

let args = CommandLine.arguments
precondition(args.count == 4, "native_input PID WINDOW_TITLE drag|command|nav-N|settings")
guard let pid = Int32(args[1]), AXIsProcessTrusted() else {
    fatalError("CI runner needs Accessibility permission for native terminal input")
}
let title = args[2]
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let matches = windows.filter {
    ($0[kCGWindowOwnerPID as String] as? Int32) == pid &&
    ($0[kCGWindowName as String] as? String) == title &&
    ($0[kCGWindowLayer as String] as? Int) == 0
}
precondition(matches.count == 1, "Expected exactly one visible owned app window")
let bounds = matches[0][kCGWindowBounds as String] as! [String: Double]
let origin = CGPoint(x: bounds["X"]!, y: bounds["Y"]!)
let width = bounds["Width"]!
let height = bounds["Height"]!
precondition(width >= 1200 && height >= 700, "Unexpected app window geometry")
let source = CGEventSource(stateID: .hidSystemState)
precondition(source != nil, "Cannot create native event source")
_ = NSRunningApplication(processIdentifier: pid)?.activate()

func mouse(_ kind: CGEventType, _ point: CGPoint) {
    guard let event = CGEvent(mouseEventSource: source, mouseType: kind, mouseCursorPosition: point,
                              mouseButton: .left) else { fatalError("Cannot create mouse event") }
    event.postToPid(pid)
}
func key(_ code: CGKeyCode, _ down: Bool, text: String? = nil, command: Bool = false) {
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down) else {
        fatalError("Cannot create key event")
    }
    if let text {
        let utf16 = Array(text.utf16)
        utf16.withUnsafeBufferPointer { buffer in
            event.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: buffer.baseAddress!)
        }
    }
    if command { event.flags = .maskCommand }
    event.postToPid(pid)
}

// The fixed review window has a 216-point sidebar. Both input targets are in
// the middle of the native terminal surface, clear of its toolbar and edges.
let contentLeft = origin.x + 224
let contentWidth = width - 240
let y = origin.y + height * 0.55
switch args[3] {
case "drag":
    let start = CGPoint(x: contentLeft + contentWidth * 0.5, y: y)
    let end = CGPoint(x: start.x + 120, y: y)
    mouse(.leftMouseDown, start)
    for step in 1...8 {
        mouse(.leftMouseDragged, CGPoint(x: start.x + CGFloat(step) * 15, y: y))
    }
    mouse(.leftMouseUp, end)
case "command":
    let point = CGPoint(x: contentLeft + contentWidth * 0.2, y: y)
    mouse(.leftMouseDown, point)
    mouse(.leftMouseUp, point)
    for character in "printf AGENTINCUIOUTPUT" {
        key(0, true, text: String(character))
        key(0, false, text: String(character))
    }
    key(36, true)
    key(36, false)
case "nav-1", "nav-2", "nav-3", "nav-4", "nav-5", "nav-6", "settings":
    let codes: [String: CGKeyCode] = ["nav-1": 18, "nav-2": 19, "nav-3": 20,
                                   "nav-4": 21, "nav-5": 23, "nav-6": 22,
                                   "settings": 43]
    let code = codes[args[3]]!
    key(code, true, command: true)
    key(code, false, command: true)
default:
    fatalError("Expected drag or command")
}
