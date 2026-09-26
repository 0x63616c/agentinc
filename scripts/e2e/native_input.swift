// Native AppKit child views are outside Pilot's GPUI input and screenshot tree.
// This tool sends actual window events to the one app PID owned by the test.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

let args = CommandLine.arguments
precondition(args.count == 4 || args.count == 5, "native_input PID WINDOW_TITLE drag-start|drag-move|drag-end|command|nav-N|settings [OFFSET]")
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
_ = NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows])
let accessibilityApp = AXUIElementCreateApplication(pid)
let frontmost = AXUIElementSetAttributeValue(accessibilityApp,
                                             kAXFrontmostAttribute as CFString,
                                             kCFBooleanTrue)
precondition(frontmost == .success, "Cannot make owned app frontmost: \(frontmost.rawValue)")
var accessibilityWindows: CFTypeRef?
let windowStatus = AXUIElementCopyAttributeValue(accessibilityApp,
                                                  kAXWindowsAttribute as CFString,
                                                  &accessibilityWindows)
precondition(windowStatus == .success, "Cannot access owned app windows")
for window in accessibilityWindows as? [AXUIElement] ?? [] {
    var windowTitle: CFTypeRef?
    if AXUIElementCopyAttributeValue(window, kAXTitleAttribute as CFString, &windowTitle) == .success,
       windowTitle as? String == title {
        precondition(AXUIElementPerformAction(window, kAXRaiseAction as CFString) == .success,
                     "Cannot raise owned app window")
    }
}

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

// The test window is fixed at 1360 logical points (clamped to the runner's
// screen). The sidebar is 216, with 8-point panel gaps and 8-point padding.
// The restored split starts at a 0.5 ratio. Offsets let the caller account for
// a narrow WindowServer frame border without assuming the divider is 1 pixel.
let dividerX = origin.x + width / 2 + 104 + (args.count == 5 ? Double(args[4])! : 0)
let paneY = origin.y + height * 0.55
switch args[3] {
case "drag-start":
    let start = CGPoint(x: dividerX, y: paneY)
    mouse(.leftMouseDown, start)
case "drag-move":
    mouse(.leftMouseDragged, CGPoint(x: dividerX + 120, y: paneY))
case "drag-end":
    mouse(.leftMouseUp, CGPoint(x: dividerX + 120, y: paneY))
case "command":
    let point = CGPoint(x: origin.x + 216 + (dividerX - origin.x - 216) / 2, y: paneY)
    mouse(.leftMouseDown, point)
    mouse(.leftMouseUp, point)
    // The first key can race AppKit's focus transfer from the click. Escape is
    // inert at an empty prompt; the second command checks the settled focus.
    key(53, true)
    key(53, false)
    for _ in 0..<2 {
        for character in "echo AI1UI" {
            key(0, true, text: String(character))
            key(0, false, text: String(character))
        }
        key(36, true)
        key(36, false)
    }
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
