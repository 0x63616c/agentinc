import AppKit
import GhosttyTerminal

public typealias ShortcutCallback = @convention(c) (UnsafeMutableRawPointer?, Int32) -> Void

// The AppKit terminal is first responder, so GPUI's window key bindings do
// not see these keys. Consume only AgentInc's navigation keys here; all other
// configured Ghostty bindings continue to reach Ghostty.
@MainActor
func appShortcutCommand(_ event: NSEvent, in pane: NSView, shown: Bool) -> Int32? {
    guard shown, pane.window?.firstResponder === pane,
          event.modifierFlags.contains(.command),
          !event.modifierFlags.contains(.shift),
          !event.modifierFlags.contains(.option),
          !event.modifierFlags.contains(.control)
    else { return nil }
    let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
    switch key {
    case "k": return -1 // AgentInc Search
    case ",": return -2 // AgentInc Settings
    default:
        guard key.count == 1, let digit = key.first?.wholeNumberValue else { return nil }
        return Int32(digit)
    }
}

private final class PaneView: AppTerminalView {
    weak var host: TerminalHost?

    override func becomeFirstResponder() -> Bool {
        host?.focused = self
        return super.becomeFirstResponder()
    }

    override func mouseDown(with event: NSEvent) {
        host?.focus(self)
        super.mouseDown(with: event)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if host?.shortcut(event, in: self) == true { return true }
        return super.performKeyEquivalent(with: event)
    }

    override func keyDown(with event: NSEvent) {
        if host?.shortcut(event, in: self) == true { return }
        super.keyDown(with: event)
    }
}

@MainActor
private final class ZoomIndicator: NSButton {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        image = NSImage(systemSymbolName: "arrow.up.left.and.arrow.down.right",
                        accessibilityDescription: "Restore terminal panes")
        isBordered = false
        imagePosition = .imageOnly
        contentTintColor = NSColor(calibratedWhite: 0.77, alpha: 1)
        wantsLayer = true
        layer?.backgroundColor = NSColor(calibratedWhite: 0.10, alpha: 0.94).cgColor
        layer?.cornerRadius = 6
        setAccessibilityLabel("Restore terminal panes")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}

private indirect enum PaneNode {
    case pane(PaneView)
    case split(vertical: Bool, PaneNode, PaneNode)

    var panes: [PaneView] {
        switch self {
        case .pane(let pane): [pane]
        case .split(_, let first, let second): first.panes + second.panes
        }
    }

    func inserting(_ newPane: PaneView, beside target: PaneView, vertical: Bool) -> PaneNode {
        switch self {
        case .pane(let pane) where pane === target:
            return .split(vertical: vertical, .pane(pane), .pane(newPane))
        case .pane:
            return self
        case .split(let axis, let first, let second):
            return .split(vertical: axis,
                          first.inserting(newPane, beside: target, vertical: vertical),
                          second.inserting(newPane, beside: target, vertical: vertical))
        }
    }

    func removing(_ target: PaneView) -> PaneNode? {
        switch self {
        case .pane(let pane): return pane === target ? nil : self
        case .split(let axis, let first, let second):
            let left = first.removing(target)
            let right = second.removing(target)
            switch (left, right) {
            case (let left?, let right?): return .split(vertical: axis, left, right)
            case (let left?, nil): return left
            case (nil, let right?): return right
            case (nil, nil): return nil
            }
        }
    }
}

@MainActor
private final class TerminalHost: NSObject {
    let container = NSView(frame: .zero)
    let zoomIndicator = ZoomIndicator(frame: .zero)
    let controller: TerminalController
    let home: String
    let navigate: ShortcutCallback?
    let context: UnsafeMutableRawPointer?
    var tree: PaneNode!
    weak var focused: PaneView?
    weak var zoomed: PaneView?
    var shown = false

    init(parent: NSView, home: String, colors: String, dividerColor: UInt32,
         navigate: ShortcutCallback?,
         context: UnsafeMutableRawPointer?) {
        self.home = home
        self.navigate = navigate
        self.context = context
        let xdg = ProcessInfo.processInfo.environment["XDG_CONFIG_HOME"] ?? "\(home)/.config"
        let folders = ["\(xdg)/ghostty", "\(home)/Library/Application Support/com.mitchellh.ghostty"]
        let files = folders.flatMap { folder in
            ["config.ghostty", "config"].map { "\(folder)/\($0)" }
        }.filter { FileManager.default.fileExists(atPath: $0) }
        let includes = files.map { "config-file = \"\($0)\"" }.joined(separator: "\n")
        controller = TerminalController(configSource: .generated(includes + "\n" + colors),
                                        theme: TerminalTheme())
        if let issue = controller.lastConfigurationIssue {
            fputs("AgentInc Ghostty configuration: \(issue)\n", stderr)
        }
        super.init()
        container.wantsLayer = true
        container.layer?.masksToBounds = true
        container.layer?.backgroundColor = NSColor(
            calibratedRed: CGFloat((dividerColor >> 16) & 0xff) / 255,
            green: CGFloat((dividerColor >> 8) & 0xff) / 255,
            blue: CGFloat(dividerColor & 0xff) / 255,
            alpha: 1
        ).cgColor
        parent.addSubview(container)
        zoomIndicator.target = self
        zoomIndicator.action = #selector(restorePanes)
        zoomIndicator.isHidden = true
        container.addSubview(zoomIndicator)
        let first = makePane()
        tree = .pane(first)
        focused = first
        layoutPanes()
    }

    private func makePane() -> PaneView {
        let pane = PaneView(frame: .zero)
        pane.host = self
        pane.configuration = TerminalSurfaceOptions(workingDirectory: home)
        pane.controller = controller
        pane.setAccessibilityElement(true)
        pane.setAccessibilityIdentifier("terminal.pane")
        pane.setAccessibilityLabel("Ghostty terminal")
        container.addSubview(pane)
        return pane
    }

    func setFrame(x: Double, y: Double, width: Double, height: Double, visible: Bool,
                  focusOnShow: Bool) {
        guard let parent = container.superview else { return }
        let frameY = parent.isFlipped ? y : Double(parent.bounds.height) - y - height
        let next = NSRect(x: x, y: frameY, width: width, height: height)
        if container.frame != next { container.frame = next }
        if !visible && shown, let window = container.window,
           window.firstResponder is PaneView {
            window.makeFirstResponder(parent)
        }
        shown = visible
        container.isHidden = !visible
        layoutPanes()
        if visible && focusOnShow, let focused { focus(focused) }
    }

    func focus(_ pane: PaneView) {
        focused = pane
        if shown { pane.window?.makeFirstResponder(pane) }
    }

    func shortcut(_ event: NSEvent, in pane: PaneView) -> Bool {
        if let command = appShortcutCommand(event, in: pane, shown: shown) {
            navigate?(context, command)
            return true
        }
        guard shown, pane.window?.firstResponder === pane,
              event.modifierFlags.contains(.command),
              !event.modifierFlags.contains(.option),
              !event.modifierFlags.contains(.control)
        else { return false }
        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        let shifted = event.modifierFlags.contains(.shift)
        if shifted && (event.keyCode == 36 || event.keyCode == 76) {
            toggleZoom(pane)
            return true
        }
        switch (key, shifted) {
        case ("d", false): split(pane, vertical: false)
        case ("d", true): split(pane, vertical: true)
        case ("w", false): close(pane)
        case ("=", true), ("+", true): toggleZoom(pane)
        default: return false
        }
        return true
    }

    private func split(_ pane: PaneView, vertical: Bool) {
        let newPane = makePane()
        tree = tree.inserting(newPane, beside: pane, vertical: vertical)
        zoomed = nil
        layoutPanes()
        focus(newPane)
    }

    private func close(_ pane: PaneView) {
        let panes = tree.panes
        guard panes.count > 1 else { return }
        let index = panes.firstIndex { $0 === pane } ?? 0
        tree = tree.removing(pane)
        if zoomed === pane { zoomed = nil }
        pane.removeFromSuperview()
        layoutPanes()
        focus(tree.panes[min(index, tree.panes.count - 1)])
    }

    private func toggleZoom(_ pane: PaneView) {
        zoomed = zoomed === pane ? nil : pane
        layoutPanes()
        focus(pane)
    }

    @objc private func restorePanes() {
        guard let pane = zoomed else { return }
        toggleZoom(pane)
    }

    private func layoutPanes() {
        guard let tree else { return }
        let full = container.bounds
        if let zoomed {
            for pane in tree.panes {
                let visible = shown && pane === zoomed
                pane.isHidden = !visible
                pane.setSurfaceVisible(visible)
                if pane === zoomed { pane.frame = full }
            }
        } else {
            layout(tree, in: full)
        }
        zoomIndicator.isHidden = zoomed == nil || tree.panes.count < 2 || !shown
        if !zoomIndicator.isHidden {
            zoomIndicator.frame = NSRect(x: full.maxX - 36, y: full.maxY - 36,
                                         width: 28, height: 28)
            container.addSubview(zoomIndicator, positioned: .above, relativeTo: nil)
        }
    }

    private func layout(_ node: PaneNode, in frame: NSRect) {
        switch node {
        case .pane(let pane):
            pane.frame = frame
            pane.isHidden = !shown
            pane.setSurfaceVisible(shown)
        case .split(let vertical, let first, let second):
            let gap: CGFloat = 2
            if vertical {
                let half = max(0, (frame.height - gap) / 2)
                layout(first, in: NSRect(x: frame.minX, y: frame.minY + half + gap,
                                         width: frame.width, height: half))
                layout(second, in: NSRect(x: frame.minX, y: frame.minY,
                                          width: frame.width, height: half))
            } else {
                let half = max(0, (frame.width - gap) / 2)
                layout(first, in: NSRect(x: frame.minX, y: frame.minY,
                                         width: half, height: frame.height))
                layout(second, in: NSRect(x: frame.minX + half + gap, y: frame.minY,
                                          width: half, height: frame.height))
            }
        }
    }

    func dispose() {
        container.removeFromSuperview()
        tree = nil
        focused = nil
        zoomed = nil
    }
}

@_cdecl("agentinc_ghostty_create")
public func agentincGhosttyCreate(_ parent: UnsafeMutableRawPointer?, _ home: UnsafePointer<CChar>?,
                                  _ colors: UnsafePointer<CChar>?,
                                  _ dividerColor: UInt32,
                                  _ navigate: ShortcutCallback?,
                                  _ context: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard let parent, let home, let colors else { return nil }
    let parentAddress = UInt(bitPattern: parent)
    let contextAddress = context.map { UInt(bitPattern: $0) }
    let homePath = String(cString: home)
    let colorConfig = String(cString: colors)
    let address = MainActor.assumeIsolated { () -> UInt in
        let view = Unmanaged<NSView>.fromOpaque(UnsafeMutableRawPointer(bitPattern: parentAddress)!)
            .takeUnretainedValue()
        let host = TerminalHost(parent: view, home: homePath, colors: colorConfig,
                                dividerColor: dividerColor, navigate: navigate,
                                context: contextAddress.flatMap(UnsafeMutableRawPointer.init(bitPattern:)))
        return UInt(bitPattern: Unmanaged.passRetained(host).toOpaque())
    }
    return UnsafeMutableRawPointer(bitPattern: address)
}

@_cdecl("agentinc_ghostty_set_frame")
public func agentincGhosttySetFrame(_ pointer: UnsafeMutableRawPointer?, _ x: Double, _ y: Double,
                                    _ width: Double, _ height: Double, _ visible: Bool,
                                    _ focus: Bool) {
    let address = pointer.map { UInt(bitPattern: $0) }
    MainActor.assumeIsolated {
        guard let address, let pointer = UnsafeMutableRawPointer(bitPattern: address) else { return }
        Unmanaged<TerminalHost>.fromOpaque(pointer).takeUnretainedValue()
            .setFrame(x: x, y: y, width: width, height: height, visible: visible, focusOnShow: focus)
    }
}

@_cdecl("agentinc_ghostty_destroy")
public func agentincGhosttyDestroy(_ pointer: UnsafeMutableRawPointer?) {
    let address = pointer.map { UInt(bitPattern: $0) }
    MainActor.assumeIsolated {
        guard let address, let pointer = UnsafeMutableRawPointer(bitPattern: address) else { return }
        let host = Unmanaged<TerminalHost>.fromOpaque(pointer).takeRetainedValue()
        host.dispose()
    }
}
