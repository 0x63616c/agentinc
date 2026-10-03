import AppKit
import XCTest
import GhosttyTerminal
@testable import AgentIncGhosttyBridge

private final class FocusableView: NSView {
    override var acceptsFirstResponder: Bool { true }
}

private final class CommandObservations {
    var values: [Bool] = []
}

private func recordCommand(_ context: UnsafeMutableRawPointer?, _ held: Bool) {
    guard let context else { return }
    Unmanaged<CommandObservations>.fromOpaque(context).takeUnretainedValue().values.append(held)
}

@MainActor
final class ShortcutTests: XCTestCase {
    private func key(_ text: String, modifiers: NSEvent.ModifierFlags, code: UInt16) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
                         timestamp: 0, windowNumber: 0, context: nil,
                         characters: text, charactersIgnoringModifiers: text,
                         isARepeat: false, keyCode: code)!
    }

    func testTerminalForwardsCommandTransitionsWithoutConsumingInput() throws {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let parent = try XCTUnwrap(window.contentView)
        let pointer = NSHomeDirectory().withCString { home in
            "background = #171717".withCString { colors in
                agentincGhosttyCreate(Unmanaged.passUnretained(parent).toOpaque(), home,
                                       nil, nil, colors, 0x333333, nil, nil)
            }
        }
        let host = try XCTUnwrap(pointer)
        defer { agentincGhosttyDestroy(host) }
        let observations = CommandObservations()
        agentincGhosttySetCommandCallback(host, recordCommand,
            Unmanaged.passUnretained(observations).toOpaque())
        agentincGhosttySetFrame(host, 0, 0, 400, 300, true, true)
        let pane = try XCTUnwrap(parent.subviews.flatMap(\.subviews)
            .compactMap { $0 as? AppTerminalView }.first)
        XCTAssertTrue(window.firstResponder === pane)
        func flags(_ modifiers: NSEvent.ModifierFlags) throws -> NSEvent {
            try XCTUnwrap(NSEvent.keyEvent(with: .flagsChanged, location: .zero,
                modifierFlags: modifiers, timestamp: 0, windowNumber: window.windowNumber,
                context: nil, characters: "", charactersIgnoringModifiers: "",
                isARepeat: false, keyCode: 55))
        }
        pane.flagsChanged(with: try flags(.command))
        pane.flagsChanged(with: try flags([.command, .shift]))
        pane.flagsChanged(with: try flags(.shift))
        pane.flagsChanged(with: try flags([]))
        XCTAssertEqual(observations.values, [true, true, false, false])

        // Navigation stays consumed by AgentInc; ordinary terminal input does
        // not. Both carry modifier snapshots if a flags event was missed.
        XCTAssertTrue(pane.performKeyEquivalent(with: key("5", modifiers: .command, code: 23)))
        XCTAssertEqual(observations.values.last, true)
        XCTAssertFalse(pane.performKeyEquivalent(with: key("x", modifiers: [], code: 7)))
        XCTAssertEqual(observations.values.last, false)

        agentincGhosttySetFrame(host, 0, 0, 400, 300, false, false)
        let count = observations.values.count
        pane.flagsChanged(with: try flags(.command))
        XCTAssertEqual(observations.values.count, count, "hidden panes must not own shell hints")
        agentincGhosttySetFrame(host, 0, 0, 400, 300, true, true)
        pane.flagsChanged(with: try flags(.command))
        pane.flagsChanged(with: try flags([]))
        XCTAssertEqual(Array(observations.values.suffix(2)), [true, false])
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
