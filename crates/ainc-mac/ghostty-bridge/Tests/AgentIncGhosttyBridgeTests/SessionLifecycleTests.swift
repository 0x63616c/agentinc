import AppKit
import Darwin
import Dispatch
import GhosttyTerminal
import XCTest
@testable import AgentIncGhosttyBridge

@MainActor
final class SessionLifecycleTests: XCTestCase {
    private func panes(in view: NSView) -> [AppTerminalView] {
        ([view as? AppTerminalView].compactMap { $0 })
            + view.subviews.flatMap { panes(in: $0) }
    }

    private func expectFile(_ url: URL, containing expected: String) {
        let result = expectation(description: "\(url.lastPathComponent) contains \(expected)")
        let descriptor = open(url.deletingLastPathComponent().path, O_EVTONLY)
        XCTAssertGreaterThanOrEqual(descriptor, 0)
        guard descriptor >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: descriptor, eventMask: .write, queue: .main)
        var fulfilled = false
        source.setEventHandler {
            if !fulfilled, (try? String(contentsOf: url, encoding: .utf8))?.contains(expected) == true {
                fulfilled = true
                result.fulfill()
            }
        }
        source.setCancelHandler { close(descriptor) }
        source.resume()
        if !fulfilled, (try? String(contentsOf: url, encoding: .utf8))?.contains(expected) == true {
            fulfilled = true
            result.fulfill()
        }
        wait(for: [result], timeout: 15)
        source.cancel()
    }

    func testSplitProcessContinuesWhileTerminalPageIsHidden() throws {
        _ = NSApplication.shared
        let directory = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent(".build/session-lifecycle-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let fifo = directory.appendingPathComponent("gate")
        let ready = directory.appendingPathComponent("ready")
        let continued = directory.appendingPathComponent("continued")
        XCTAssertEqual(mkfifo(fifo.path, 0o600), 0)

        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 500),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let parent = window.contentView!
        let home = NSHomeDirectory()
        let pointer = home.withCString { home in
            "background = #171717".withCString { colors in
                agentincGhosttyCreate(Unmanaged.passUnretained(parent).toOpaque(), home,
                                       nil, nil, colors, 0x333333, nil, nil)
            }
        }
        let host = try XCTUnwrap(pointer)
        defer { agentincGhosttyDestroy(host) }
        agentincGhosttySetFrame(host, 0, 0, 800, 500, true, true)
        let first = try XCTUnwrap(panes(in: parent).first)
        XCTAssertTrue(window.makeFirstResponder(first))

        let split = try XCTUnwrap(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: .command,
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "d", charactersIgnoringModifiers: "d",
            isARepeat: false, keyCode: 2))
        XCTAssertTrue(first.performKeyEquivalent(with: split))
        let splitPanes = panes(in: parent)
        XCTAssertEqual(splitPanes.count, 2)
        let second = try XCTUnwrap(splitPanes.first { $0 !== first })

        let command = "/bin/sh -c 'echo $$ > \(ready.path); IFS= read -r line < \(fifo.path); echo $$:$line > \(continued.path)'"
        XCTAssertTrue(second.paste(text: command))
        XCTAssertTrue(second.sendKey(.enter))
        expectFile(ready, containing: "\n")
        let pid = try String(contentsOf: ready, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        XCTAssertFalse(pid.isEmpty)

        // Route changes hide the AppKit child views without removing either
        // surface. The FIFO releases the process while the page is hidden.
        agentincGhosttySetFrame(host, 0, 0, 800, 500, false, false)
        XCTAssertEqual(panes(in: parent).count, 2)
        XCTAssertFalse(window.firstResponder is AppTerminalView)
        DispatchQueue.global().async { @Sendable [fifo] in
            let descriptor = open(fifo.path, O_WRONLY)
            guard descriptor >= 0 else { return }
            _ = "continued\n".withCString { write(descriptor, $0, 10) }
            close(descriptor)
        }
        expectFile(continued, containing: "\(pid):continued")

        agentincGhosttySetFrame(host, 0, 0, 800, 500, true, true)
        let restored = panes(in: parent)
        XCTAssertTrue(restored.contains { $0 === first })
        XCTAssertTrue(restored.contains { $0 === second })
        XCTAssertTrue(window.firstResponder === second)
    }

    func testSplitAndZoomRestoreAfterHostRecreation() throws {
        _ = NSApplication.shared
        let directory = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent(".build/layout-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let layout = directory.appendingPathComponent("terminal-layout.json")
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 500),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let parent = window.contentView!
        func create() -> UnsafeMutableRawPointer? {
            NSHomeDirectory().withCString { home in
                "/usr/bin/true".withCString { helper in
                    layout.path.withCString { path in
                        "background = #171717".withCString { colors in
                            agentincGhosttyCreate(Unmanaged.passUnretained(parent).toOpaque(),
                                                   home, helper, path, colors, 0x333333, nil, nil)
                        }
                    }
                }
            }
        }
        let firstHost = try XCTUnwrap(create())
        agentincGhosttySetFrame(firstHost, 0, 0, 800, 500, true, true)
        let first = try XCTUnwrap(panes(in: parent).first)
        XCTAssertTrue(window.makeFirstResponder(first))
        let split = try XCTUnwrap(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: .command,
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "d", charactersIgnoringModifiers: "d", isARepeat: false, keyCode: 2))
        XCTAssertTrue(first.performKeyEquivalent(with: split))
        XCTAssertEqual(panes(in: parent).count, 2)
        let second = try XCTUnwrap(panes(in: parent).first { $0 !== first })
        let zoom = try XCTUnwrap(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: [.command, .shift],
            timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36))
        XCTAssertTrue(second.performKeyEquivalent(with: zoom))
        let saved = try Data(contentsOf: layout)
        let firstIDs = try XCTUnwrap(JSONSerialization.jsonObject(with: saved) as? [String: Any])
        XCTAssertNotNil(firstIDs["tree"])
        XCTAssertNotNil(firstIDs["zoomed"])
        agentincGhosttyDestroy(firstHost)
        XCTAssertTrue(panes(in: parent).isEmpty)

        let secondHost = try XCTUnwrap(create())
        defer { agentincGhosttyDestroy(secondHost) }
        agentincGhosttySetFrame(secondHost, 0, 0, 800, 500, true, true)
        XCTAssertEqual(panes(in: parent).count, 2)
        XCTAssertEqual(try Data(contentsOf: layout), saved)
    }
}
