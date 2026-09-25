import AppKit
import XCTest
@testable import AgentIncGhosttyBridge

private final class FocusableView: NSView {
    override var acceptsFirstResponder: Bool { true }
}

@MainActor
final class ShortcutTests: XCTestCase {
    private func key(_ text: String, modifiers: NSEvent.ModifierFlags, code: UInt16) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
                         timestamp: 0, windowNumber: 0, context: nil,
                         characters: text, charactersIgnoringModifiers: text,
                         isARepeat: false, keyCode: code)!
    }

    func testAgentIncShortcutsRequireFocusedTerminalPane() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let pane = FocusableView(frame: window.contentView!.bounds)
        let other = FocusableView(frame: .zero)
        window.contentView!.addSubview(pane)
        window.contentView!.addSubview(other)
        XCTAssertTrue(window.makeFirstResponder(pane))

        let search = key("k", modifiers: .command, code: 40)
        XCTAssertEqual(appShortcutCommand(search, in: pane, shown: true), -1)
        XCTAssertEqual(appShortcutCommand(key(",", modifiers: .command, code: 43),
                                          in: pane, shown: true), -2)
        XCTAssertEqual(appShortcutCommand(key("5", modifiers: .command, code: 23),
                                          in: pane, shown: true), 5)
        XCTAssertNil(appShortcutCommand(key("0", modifiers: .command, code: 29),
                                        in: pane, shown: true))
        XCTAssertNil(appShortcutCommand(key("l", modifiers: .control, code: 37),
                                        in: pane, shown: true))
        XCTAssertNil(appShortcutCommand(search, in: pane, shown: false))

        XCTAssertTrue(window.makeFirstResponder(other))
        XCTAssertNil(appShortcutCommand(search, in: pane, shown: true))
    }
}
