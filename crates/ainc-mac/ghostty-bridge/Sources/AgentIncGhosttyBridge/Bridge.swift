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
        guard key.count == 1, let digit = key.first?.wholeNumberValue,
              (1...5).contains(digit) else { return nil }
        return Int32(digit)
    }
}

private final class PaneView: AppTerminalView {
    weak var host: TerminalHost?
    var sessionID = ""

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

@MainActor
private indirect enum PaneNode {
    case pane(PaneView)
    case split(vertical: Bool, ratio: CGFloat, PaneNode, PaneNode)

    var panes: [PaneView] {
        switch self {
        case .pane(let pane): [pane]
        case .split(_, _, let first, let second): first.panes + second.panes
        }
    }

    var record: PaneRecord {
        switch self {
        case .pane(let pane): return PaneRecord(id: pane.sessionID)
        case .split(let vertical, let ratio, let first, let second):
            return PaneRecord(vertical: vertical, ratio: Double(ratio),
                              first: first.record, second: second.record)
        }
    }

    func inserting(_ newPane: PaneView, beside target: PaneView, vertical: Bool) -> PaneNode {
        switch self {
        case .pane(let pane) where pane === target:
            return .split(vertical: vertical, ratio: 0.5, .pane(pane), .pane(newPane))
        case .pane:
            return self
        case .split(let axis, let ratio, let first, let second):
            return .split(vertical: axis, ratio: ratio,
                          first.inserting(newPane, beside: target, vertical: vertical),
                          second.inserting(newPane, beside: target, vertical: vertical))
        }
    }

    func removing(_ target: PaneView) -> PaneNode? {
        switch self {
        case .pane(let pane): return pane === target ? nil : self
        case .split(let axis, let ratio, let first, let second):
            let left = first.removing(target)
            let right = second.removing(target)
            switch (left, right) {
            case (let left?, let right?): return .split(vertical: axis, ratio: ratio, left, right)
            case (let left?, nil): return left
            case (nil, let right?): return right
            case (nil, nil): return nil
            }
        }
    }

    var minimumSize: NSSize {
        switch self {
        case .pane: return NSSize(width: 100, height: 80)
        case .split(let vertical, _, let first, let second):
            let a = first.minimumSize, b = second.minimumSize
            return vertical
                ? NSSize(width: max(a.width, b.width), height: a.height + b.height + 2)
                : NSSize(width: a.width + b.width + 2, height: max(a.height, b.height))
        }
    }

    mutating func setRatio(_ ratio: CGFloat, at path: [Bool]) {
        guard case .split(let vertical, let current, var first, var second) = self else { return }
        if let branch = path.first {
            if branch { second.setRatio(ratio, at: Array(path.dropFirst())) }
            else { first.setRatio(ratio, at: Array(path.dropFirst())) }
        }
        self = .split(vertical: vertical, ratio: path.isEmpty ? ratio : current, first, second)
    }
}

private final class PaneRecord: Codable {
    var id: String?
    var vertical: Bool?
    var ratio: Double?
    var first: PaneRecord?
    var second: PaneRecord?

    init(id: String) { self.id = id }
    init(vertical: Bool, ratio: Double, first: PaneRecord, second: PaneRecord) {
        self.vertical = vertical
        self.ratio = ratio
        self.first = first
        self.second = second
    }

    var isValid: Bool {
        if let id { return UUID(uuidString: id) != nil }
        return vertical != nil && (ratio.map { $0.isFinite && (0...1).contains($0) } ?? true)
            && first?.isValid == true && second?.isValid == true
    }
}

private struct TerminalLayout: Codable {
    var tree: PaneRecord
    var zoomed: String?
}

private struct SplitDivider {
    let path: [Bool]
    let hitFrame: NSRect
    let parentFrame: NSRect
    let vertical: Bool
    let firstMinimum: CGFloat
    let secondMinimum: CGFloat
}

@MainActor
private final class SplitContainer: NSView {
    weak var host: TerminalHost?

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, let host else { return super.hitTest(point) }
        let local = convert(point, from: superview)
        if host.dividers.contains(where: { $0.hitFrame.contains(local) }) { return self }
        return super.hitTest(point)
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        guard let host else { return }
        for divider in host.dividers {
            addCursorRect(divider.hitFrame,
                          cursor: divider.vertical ? .resizeUpDown : .resizeLeftRight)
        }
    }

    override func mouseDown(with event: NSEvent) {
        host?.beginDrag(at: convert(event.locationInWindow, from: nil))
    }

    override func mouseDragged(with event: NSEvent) {
        host?.drag(to: convert(event.locationInWindow, from: nil))
    }

    override func mouseUp(with event: NSEvent) {
        host?.endDrag()
    }
}

@MainActor
private final class TerminalHost: NSObject {
    let container = SplitContainer(frame: .zero)
    let zoomIndicator = ZoomIndicator(frame: .zero)
    let controller: TerminalController
    let home: String
    let helper: String?
    let layoutURL: URL?
    let navigate: ShortcutCallback?
    let context: UnsafeMutableRawPointer?
    var tree: PaneNode!
    weak var focused: PaneView?
    weak var zoomed: PaneView?
    var shown = false
    var dividers: [SplitDivider] = []
    private var dragging: SplitDivider?

    init(parent: NSView, home: String, helper: String?, layoutPath: String?,
         colors: String, dividerColor: UInt32,
         navigate: ShortcutCallback?,
         context: UnsafeMutableRawPointer?) {
        self.home = home
        self.helper = helper
        self.layoutURL = layoutPath.map { URL(fileURLWithPath: $0) }
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
        container.host = self
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
        if let layoutURL,
           let data = try? Data(contentsOf: layoutURL),
           let saved = try? JSONDecoder().decode(TerminalLayout.self, from: data),
           saved.tree.isValid,
           let restored = restore(saved.tree) {
            tree = restored
            focused = restored.panes.first
            zoomed = restored.panes.first { $0.sessionID == saved.zoomed }
        } else {
            let first = makePane()
            tree = .pane(first)
            focused = first
            saveLayout()
        }
        layoutPanes()
    }

    private func restore(_ record: PaneRecord) -> PaneNode? {
        if let id = record.id, UUID(uuidString: id) != nil {
            return .pane(makePane(id: id, existing: true))
        }
        guard let vertical = record.vertical,
              let first = record.first.flatMap(restore),
              let second = record.second.flatMap(restore) else { return nil }
        return .split(vertical: vertical, ratio: CGFloat(record.ratio ?? 0.5), first, second)
    }

    private func saveLayout() {
        guard let tree, let layoutURL else { return }
        do {
            let data = try JSONEncoder().encode(TerminalLayout(tree: tree.record, zoomed: zoomed?.sessionID))
            try FileManager.default.createDirectory(at: layoutURL.deletingLastPathComponent(),
                                                    withIntermediateDirectories: true)
            try data.write(to: layoutURL, options: .atomic)
        } catch {
            fputs("AgentInc terminal layout: \(error)\n", stderr)
        }
    }

    private func makePane(id: String = UUID().uuidString, existing: Bool = false) -> PaneView {
        let pane = PaneView(frame: .zero)
        pane.host = self
        pane.sessionID = id
        let command = helper.map {
            let quoted = "'\($0.replacingOccurrences(of: "'", with: "'\\''"))'"
            return "\(quoted) --terminal-attach \(id)\(existing ? " --existing" : "")"
        }
        pane.configuration = TerminalSurfaceOptions(workingDirectory: home, command: command,
                                                     waitAfterCommand: true)
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
        saveLayout()
    }

    private func close(_ pane: PaneView) {
        let panes = tree.panes
        guard panes.count > 1 else { return }
        let index = panes.firstIndex { $0 === pane } ?? 0
        tree = tree.removing(pane)
        if zoomed === pane { zoomed = nil }
        pane.removeFromSuperview()
        if let helper {
            let command = Process()
            command.executableURL = URL(fileURLWithPath: helper)
            command.arguments = ["--terminal-close", pane.sessionID]
            do { try command.run() } catch { fputs("AgentInc close terminal: \(error)\n", stderr) }
        }
        layoutPanes()
        focus(tree.panes[min(index, tree.panes.count - 1)])
        saveLayout()
    }

    private func toggleZoom(_ pane: PaneView) {
        zoomed = zoomed === pane ? nil : pane
        saveLayout()
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
        dividers.removeAll()
        if let zoomed {
            for pane in tree.panes {
                let visible = shown && pane === zoomed
                pane.isHidden = !visible
                pane.setSurfaceVisible(visible)
                if pane === zoomed { pane.frame = full }
            }
        } else {
            layout(tree, in: full, path: [])
        }
        container.window?.invalidateCursorRects(for: container)
        zoomIndicator.isHidden = zoomed == nil || tree.panes.count < 2 || !shown
        if !zoomIndicator.isHidden {
            zoomIndicator.frame = NSRect(x: full.maxX - 36, y: full.maxY - 36,
                                         width: 28, height: 28)
            container.addSubview(zoomIndicator, positioned: .above, relativeTo: nil)
        }
    }

    private func splitLength(_ ratio: CGFloat, available: CGFloat,
                             firstMinimum: CGFloat, secondMinimum: CGFloat) -> CGFloat {
        guard available > 0 else { return 0 }
        guard firstMinimum + secondMinimum <= available else {
            return available * firstMinimum / (firstMinimum + secondMinimum)
        }
        return min(max(ratio * available, firstMinimum), available - secondMinimum)
    }

    private func layout(_ node: PaneNode, in frame: NSRect, path: [Bool]) {
        switch node {
        case .pane(let pane):
            pane.frame = frame
            pane.isHidden = !shown
            pane.setSurfaceVisible(shown)
        case .split(let vertical, let ratio, let first, let second):
            let gap: CGFloat = 2
            let firstMinimum = vertical ? first.minimumSize.height : first.minimumSize.width
            let secondMinimum = vertical ? second.minimumSize.height : second.minimumSize.width
            if vertical {
                let available = max(0, frame.height - gap)
                let firstHeight = splitLength(ratio, available: available,
                                              firstMinimum: firstMinimum, secondMinimum: secondMinimum)
                let secondHeight = available - firstHeight
                let line = NSRect(x: frame.minX, y: frame.minY + secondHeight,
                                  width: frame.width, height: gap)
                dividers.append(SplitDivider(path: path,
                    hitFrame: line.insetBy(dx: 0, dy: -4), parentFrame: frame, vertical: true,
                    firstMinimum: firstMinimum, secondMinimum: secondMinimum))
                layout(first, in: NSRect(x: frame.minX, y: line.maxY,
                                         width: frame.width, height: firstHeight), path: path + [false])
                layout(second, in: NSRect(x: frame.minX, y: frame.minY,
                                          width: frame.width, height: secondHeight), path: path + [true])
            } else {
                let available = max(0, frame.width - gap)
                let firstWidth = splitLength(ratio, available: available,
                                             firstMinimum: firstMinimum, secondMinimum: secondMinimum)
                let line = NSRect(x: frame.minX + firstWidth, y: frame.minY,
                                  width: gap, height: frame.height)
                dividers.append(SplitDivider(path: path,
                    hitFrame: line.insetBy(dx: -4, dy: 0), parentFrame: frame, vertical: false,
                    firstMinimum: firstMinimum, secondMinimum: secondMinimum))
                layout(first, in: NSRect(x: frame.minX, y: frame.minY,
                                         width: firstWidth, height: frame.height), path: path + [false])
                layout(second, in: NSRect(x: line.maxX, y: frame.minY,
                                          width: available - firstWidth, height: frame.height), path: path + [true])
            }
        }
    }

    fileprivate func beginDrag(at point: NSPoint) {
        dragging = dividers.reversed().first { $0.hitFrame.contains(point) }
    }

    fileprivate func drag(to point: NSPoint) {
        guard let dragging else { return }
        let available = max(0, (dragging.vertical
            ? dragging.parentFrame.height : dragging.parentFrame.width) - 2)
        guard available > 0 else { return }
        let proposed = dragging.vertical
            ? dragging.parentFrame.maxY - point.y - 1
            : point.x - dragging.parentFrame.minX - 1
        let length = splitLength(proposed / available, available: available,
                                 firstMinimum: dragging.firstMinimum,
                                 secondMinimum: dragging.secondMinimum)
        tree.setRatio(length / available, at: dragging.path)
        layoutPanes()
    }

    fileprivate func endDrag() {
        guard dragging != nil else { return }
        dragging = nil
        saveLayout()
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
                                  _ helper: UnsafePointer<CChar>?, _ layoutPath: UnsafePointer<CChar>?,
                                  _ colors: UnsafePointer<CChar>?,
                                  _ dividerColor: UInt32,
                                  _ navigate: ShortcutCallback?,
                                  _ context: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard let parent, let home, let colors else { return nil }
    let parentAddress = UInt(bitPattern: parent)
    let contextAddress = context.map { UInt(bitPattern: $0) }
    let homePath = String(cString: home)
    let helperPath = helper.map(String.init(cString:))
    let layout = layoutPath.map(String.init(cString:))
    let colorConfig = String(cString: colors)
    let address = MainActor.assumeIsolated { () -> UInt in
        let view = Unmanaged<NSView>.fromOpaque(UnsafeMutableRawPointer(bitPattern: parentAddress)!)
            .takeUnretainedValue()
        let host = TerminalHost(parent: view, home: homePath, helper: helperPath,
                                layoutPath: layout, colors: colorConfig,
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
