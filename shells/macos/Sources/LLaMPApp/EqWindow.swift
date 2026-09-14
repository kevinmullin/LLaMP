import AppKit
import llamp_ffi

/// Borderless equalizer. Size and hit regions come from the core. Does not parse BMP.
public final class EqWindow: NSWindow {
    let chrome: EqChromeView
    private var shaded = false
    var skinScale = 1
    private var displayTimer: Timer?

    public init() {
        let size = llamp_eq_size()
        let frame = NSRect(x: 0, y: 0, width: CGFloat(size.width), height: CGFloat(size.height))
        chrome = EqChromeView(frame: frame)
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        isOpaque = true
        backgroundColor = NSColor(srgbRed: 0x80 / 255, green: 0x80 / 255, blue: 0x80 / 255, alpha: 1)
        hasShadow = false
        level = .normal
        contentView = chrome
        isMovableByWindowBackground = false
        title = String(cString: llamp_eq_caption())
        chrome.onPress = { [weak self] control in
            self?.apply(control)
        }
        chrome.onBackgroundDrag = { event in
            WindowDock.shared.track(which: 1, event: event)
        }
        resizeToSkin()
        chrome.reload()
    }

    public override func orderFront(_ sender: Any?) {
        startDisplayTimer()
        super.orderFront(sender)
    }

    public override func orderOut(_ sender: Any?) {
        stopDisplayTimer()
        super.orderOut(sender)
    }

    private func startDisplayTimer() {
        guard displayTimer == nil else { return }
        displayTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.pullDisplay()
            }
        }
    }

    private func stopDisplayTimer() {
        displayTimer?.invalidate()
        displayTimer = nil
    }

    public override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect {
        var frame = frameRect
        let size = skinPointSize()
        frame.size = size
        let vis = (screen ?? self.screen ?? NSScreen.main)?.visibleFrame
        if let vis {
            if frame.maxX > vis.maxX { frame.origin.x = vis.maxX - frame.width }
            if frame.minX < vis.minX { frame.origin.x = vis.minX }
            if frame.maxY > vis.maxY { frame.origin.y = vis.maxY - frame.height }
            if frame.minY < vis.minY { frame.origin.y = vis.minY }
        }
        return frame
    }

    var skinHeight: Int {
        shaded ? Int(llamp_shade_height()) : Int(llamp_eq_size().height)
    }

    public func toggleShade() {
        shaded.toggle()
        applyMask(mode: shaded ? 3 : 2)
        resizeToSkin()
        WindowDock.shared.syncFrames()
    }

    func pullDisplay() {
        let snap = llamp_playback_poll()
        let nextScale = snap.double_size == 0 ? 1 : 2
        if nextScale != skinScale {
            skinScale = nextScale
            resizeToSkin()
        }
        title = String(cString: llamp_eq_caption())
        chrome.reload()
    }

    private func apply(_ control: EqHit) {
        switch control.label {
        case "Shade":
            toggleShade()
        case "Close":
            orderOut(nil)
        case "Presets":
            guard llamp_playback_poll().supports_eq != 0 else { return }
            chrome.showPresets()
        default:
            break
        }
    }

    func resizeToSkin() {
        let size = skinPointSize()
        var frame = self.frame
        frame.size = size
        setFrame(frame, display: true)
        chrome.frame = NSRect(origin: .zero, size: size)
        chrome.setBackingScale(skinScale)
    }

    func skinPointSize() -> NSSize {
        let size = llamp_eq_size()
        let height = shaded ? llamp_shade_height() : size.height
        return NSSize(
            width: CGFloat(size.width * UInt32(skinScale)),
            height: CGFloat(height * UInt32(skinScale))
        )
    }

    private func applyMask(mode: UInt32) {
        let polygons = llamp_region_polygon_count(mode)
        guard polygons > 0 else {
            chrome.layer?.mask = nil
            return
        }
        let path = CGMutablePath()
        for polygon in 0..<polygons {
            let points = llamp_region_point_count(mode, polygon)
            guard points >= 3 else { continue }
            let first = llamp_region_point(mode, polygon, 0)
            path.move(to: CGPoint(x: CGFloat(first.x), y: CGFloat(first.y)))
            for index in 1..<points {
                let point = llamp_region_point(mode, polygon, index)
                path.addLine(to: CGPoint(x: CGFloat(point.x), y: CGFloat(point.y)))
            }
            path.closeSubpath()
        }
        let mask = CAShapeLayer()
        mask.path = path
        chrome.layer?.mask = mask
    }
}

final class EqChromeView: NSView {
    private var skinPixels: [UInt8] = []
    private var skinWidth = 0
    private var skinHeight = 0
    private var controls: [EqHit] = []
    private var elements: [EqElement] = []
    private var sliding: EqHit?
    var onPress: ((EqHit) -> Void)?
    var onBackgroundDrag: ((NSEvent) -> Void)?
    private var presetMenu: EqPresetMenu?

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.magnificationFilter = .nearest
        layer?.minificationFilter = .nearest
        layer?.contentsScale = 1
        layer?.contentsGravity = .resize
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not used")
    }

    func setBackingScale(_ scale: Int) {
        layer?.contentsScale = CGFloat(max(1, scale))
    }

    func reload() {
        let image = llamp_eq_blit()
        defer { llamp_image_free(image.data, image.len) }
        guard let data = image.data, image.len > 0 else { return }
        skinWidth = Int(image.width)
        skinHeight = Int(image.height)
        skinPixels = Array(UnsafeBufferPointer(start: data, count: image.len))
        installContents()
        if controls.isEmpty { installControls() }
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let x = Int(point.x) / scale()
        let y = Int(point.y) / scale()
        if let control = controls.first(where: { $0.contains(x: x, y: y) && $0.label != "Title bar" }) {
            if isSlider(control) {
                sliding = control
                setSlider(control, at: y)
                return
            }
            llamp_eq_press(control.id)
            onPress?(control)
            reload()
            return
        }
        if let onBackgroundDrag {
            onBackgroundDrag(event)
        }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let control = sliding else { return }
        let point = convert(event.locationInWindow, from: nil)
        setSlider(control, at: Int(point.y) / scale())
    }

    override func mouseUp(with event: NSEvent) {
        sliding = nil
        super.mouseUp(with: event)
    }

    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .group }
    override func accessibilityChildren() -> [Any]? { elements }
    override func accessibilityLabel() -> String? { String(cString: llamp_eq_caption()) }

    func showPresets() {
        presetMenu?.orderOut(nil)
        let popup = EqPresetMenu(owner: self)
        presetMenu = popup
        if let window {
            let origin = window.convertPoint(toScreen: NSPoint(x: 52, y: bounds.height - 16))
            popup.setFrameOrigin(origin)
        }
        popup.orderFront(nil)
    }

    fileprivate func dismissPresets() {
        presetMenu?.orderOut(nil)
        presetMenu = nil
        reload()
    }

    private func scale() -> Int {
        max(1, Int(layer?.contentsScale ?? 1))
    }

    private func isSlider(_ control: EqHit) -> Bool {
        control.label == "Preamp" || control.label == "Band"
    }

    func dragBand(_ band: Int, millidb: Int32) {
        guard let control = controls.first(where: { $0.id == UInt32(7 + band) }) else { return }
        let span = max(control.h - 1, 1)
        let rel = Int((12_000 - Int(millidb)) * span / 24_000)
        setSlider(control, at: control.y + rel)
    }

    private func setSlider(_ control: EqHit, at y: Int) {
        guard llamp_playback_poll().supports_eq != 0, control.h > 1 else { return }
        let rel = min(max(y - control.y, 0), control.h - 1)
        let milli = 12_000 - Int32((rel * 24_000) / (control.h - 1))
        llamp_eq_drag(control.id, milli)
        reload()
    }

    private func installControls() {
        controls = []
        elements = []
        let count = llamp_eq_control_count()
        for index in 0..<count {
            let raw = llamp_eq_control_at(index)
            guard let label = raw.label.flatMap({ String(cString: $0) }), !label.isEmpty else { continue }
            let hit = EqHit(id: raw.id, label: label, x: Int(raw.x), y: Int(raw.y), w: Int(raw.w), h: Int(raw.h))
            controls.append(hit)
            elements.append(EqElement(control: hit, view: self))
        }
        setAccessibilityChildren(elements)
    }

    private func installContents() {
        guard skinWidth > 0, let space = CGColorSpace(name: CGColorSpace.sRGB) else { return }
        guard let data = CFDataCreate(nil, skinPixels, skinPixels.count),
              let provider = CGDataProvider(data: data),
              let image = CGImage(
                width: skinWidth,
                height: skinHeight,
                bitsPerComponent: 8,
                bitsPerPixel: 32,
                bytesPerRow: skinWidth * 4,
                space: space,
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider,
                decode: nil,
                shouldInterpolate: false,
                intent: .defaultIntent
              )
        else { return }
        layer?.contents = image
        layer?.magnificationFilter = .nearest
        layer?.minificationFilter = .nearest
    }
}

struct EqHit {
    let id: UInt32
    let label: String
    let x: Int
    let y: Int
    let w: Int
    let h: Int

    func contains(x px: Int, y py: Int) -> Bool {
        px >= x && py >= y && px < x + w && py < y + h
    }
}

@MainActor
final class EqElement: NSAccessibilityElement {
    let control: EqHit
    weak var view: EqChromeView?

    init(control: EqHit, view: EqChromeView) {
        self.control = control
        self.view = view
        super.init()
        setAccessibilityLabel(control.label)
        setAccessibilityHelp(control.label)
        setAccessibilityRole(control.label == "Preamp" || control.label == "Band" ? .slider : .button)
        setAccessibilityParent(view)
        setAccessibilityFrameInParentSpace(NSRect(x: control.x, y: control.y, width: control.w, height: control.h))
    }
}

public enum EqStore {
    public static func install() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        guard let dir = base?.appendingPathComponent("LLaMP", isDirectory: true) else { return }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = dir.appendingPathComponent("eq-presets.txt").path
        _ = path.withCString { llamp_eq_set_store($0) }
    }
}

@MainActor
public final class WindowDock {
    public static let shared = WindowDock()
    public weak var main: MainWindow?
    public weak var equalizer: EqWindow?
    public weak var playlist: PlaylistWindow?
    public weak var browser: BrowserWindow?
    public weak var visualizer: VisWindow?
    private var docked = false
    private var groupOrigin: NSPoint?
    private var lastMain = Frame(x: 0, y: 0)
    private var lastEq = Frame(x: 0, y: 0)

    public func attach(main: MainWindow, equalizer: EqWindow) {
        self.main = main
        self.equalizer = equalizer
    }

    public func attach(playlist: PlaylistWindow) {
        self.playlist = playlist
    }

    public func attach(browser: BrowserWindow) {
        self.browser = browser
    }

    public func attach(visualizer: VisWindow) {
        self.visualizer = visualizer
    }

    public func detachExtras() {
        playlist = nil
        browser = nil
        visualizer = nil
        groupOrigin = nil
    }

    public func showEqualizer() {
        guard let main, let equalizer else { return }
        equalizer.skinScale = main.skinScale
        equalizer.resizeToSkin()
        placePairOnScreen()
        syncFrames()
        equalizer.orderFront(nil)
    }

    func placePairOnScreen() {
        guard let main, let equalizer, let vis = (main.screen ?? NSScreen.main)?.visibleFrame else { return }
        let gap = CGFloat(max(main.skinScale, 1))
        let mainSize = main.frame.size
        let eqSize = equalizer.skinPointSize()
        let pairH = mainSize.height + gap + eqSize.height
        var x = main.frame.origin.x
        if x + mainSize.width > vis.maxX { x = vis.maxX - mainSize.width }
        if x < vis.minX { x = vis.minX }
        var eqY = vis.midY - pairH / 2
        if eqY < vis.minY { eqY = vis.minY }
        if eqY + pairH > vis.maxY { eqY = max(vis.minY, vis.maxY - pairH) }
        let mainY = eqY + eqSize.height + gap
        main.setFrame(NSRect(x: x, y: mainY, width: mainSize.width, height: mainSize.height), display: true)
        equalizer.setFrame(NSRect(x: x, y: eqY, width: eqSize.width, height: eqSize.height), display: true)
    }

    public func track(which: UInt32, event: NSEvent) {
        if playlist != nil || browser != nil || visualizer != nil {
            trackGroup(which: which, event: event)
            return
        }
        syncFrames()
        llamp_eq_begin_drag()
        var last = NSEvent.mouseLocation
        let scale = currentScale()
        while let next = NSApp.nextEvent(matching: [.leftMouseDragged, .leftMouseUp], until: .distantFuture, inMode: .eventTracking, dequeue: true) {
            let now = NSEvent.mouseLocation
            let dx = Int32(((now.x - last.x) / scale).rounded())
            let dy = Int32(((last.y - now.y) / scale).rounded())
            last = now
            if dx != 0 || dy != 0 {
                apply(llamp_eq_drag_window(which, dx, dy))
            }
            if next.type == .leftMouseUp { break }
        }
        apply(llamp_eq_end_drag())
        _ = event
    }

    public func syncFrames() {
        guard let main, let equalizer else { return }
        let scale = currentScale()
        let mainH = Int32(main.skinHeight)
        let eqH = Int32(equalizer.skinHeight)
        let dx = Int32(((equalizer.frame.minX - main.frame.minX) / scale).rounded())
        let dy = Int32(((main.frame.maxY - equalizer.frame.maxY) / scale).rounded())
        let mainFrame = LlampFrame(x: 0, y: 0, w: 275, h: mainH)
        let eqFrame = LlampFrame(x: dx, y: dy, w: 275, h: eqH)
        llamp_eq_set_frames(mainFrame, eqFrame)
        lastMain = Frame(x: 0, y: 0)
        lastEq = Frame(x: dx, y: dy)
    }

    /// Pair path stays when playlist and browser are absent. This is the four-window path.
    public func applyGroupDrag(which: UInt32, dx: Int32, dy: Int32) {
        syncGroup()
        llamp_group_begin_drag()
        applyGroup(llamp_group_drag(which, dx, dy))
        applyGroup(llamp_group_end_drag())
    }

    public func refreshLevel(mainOnTop: Bool) {
        guard docked else { return }
        let level: NSWindow.Level = mainOnTop ? .floating : .normal
        main?.level = level
        equalizer?.level = level
    }

    private func apply(_ dock: LlampDock) {
        let scale = currentScale()
        move(main, from: lastMain, toX: dock.main_x, toY: dock.main_y, scale: scale)
        if equalizer?.isVisible == true {
            move(equalizer, from: lastEq, toX: dock.eq_x, toY: dock.eq_y, scale: scale)
        }
        lastMain = Frame(x: dock.main_x, y: dock.main_y)
        lastEq = Frame(x: dock.eq_x, y: dock.eq_y)
        docked = dock.docked != 0
        if docked {
            let level: NSWindow.Level = dock.group_on_top == 0 ? .normal : .floating
            main?.level = level
            equalizer?.level = level
        }
    }

    private func move(_ window: NSWindow?, from: Frame, toX: Int32, toY: Int32, scale: CGFloat) {
        guard let window else { return }
        var origin = window.frame.origin
        origin.x += CGFloat(toX - from.x) * scale
        origin.y -= CGFloat(toY - from.y) * scale
        window.setFrameOrigin(origin)
    }

    private func currentScale() -> CGFloat {
        CGFloat(max(main?.skinScale ?? 1, 1))
    }

    private func trackGroup(which: UInt32, event: NSEvent) {
        syncGroup()
        llamp_group_begin_drag()
        var last = NSEvent.mouseLocation
        let scale = currentScale()
        while let next = NSApp.nextEvent(matching: [.leftMouseDragged, .leftMouseUp], until: .distantFuture, inMode: .eventTracking, dequeue: true) {
            let now = NSEvent.mouseLocation
            let dx = Int32(((now.x - last.x) / scale).rounded())
            let dy = Int32(((last.y - now.y) / scale).rounded())
            last = now
            if dx != 0 || dy != 0 {
                applyGroup(llamp_group_drag(which, dx, dy))
            }
            if next.type == .leftMouseUp { break }
        }
        applyGroup(llamp_group_end_drag())
        _ = event
    }

    private func syncGroup() {
        if groupOrigin == nil, let main {
            groupOrigin = NSPoint(x: main.frame.minX, y: main.frame.maxY)
        }
        publish(0, main, fallback: (0, 0, 275, 116))
        publish(1, equalizer, fallback: (100_000, 100_000, 0, 0))
        publish(2, playlist, fallback: (100_000, 0, 0, 0))
        publish(3, browser, fallback: (100_000, 100_000, 0, 0))
        publish(4, visualizer, fallback: (100_000, 200_000, 0, 0))
    }

    private func publish(_ which: UInt32, _ window: NSWindow?, fallback: (Int32, Int32, Int32, Int32)) {
        let frame: LlampFrame
        if let window, let origin = groupOrigin {
            let scale = currentScale()
            frame = LlampFrame(
                x: Int32(((window.frame.minX - origin.x) / scale).rounded()),
                y: Int32(((origin.y - window.frame.maxY) / scale).rounded()),
                w: Int32((window.frame.width / scale).rounded()),
                h: Int32((window.frame.height / scale).rounded())
            )
        } else {
            frame = LlampFrame(x: fallback.0, y: fallback.1, w: fallback.2, h: fallback.3)
        }
        llamp_group_set_frame(which, frame)
    }

    private func applyGroup(_ group: LlampGroup) {
        guard let origin = groupOrigin else { return }
        let scale = currentScale()
        place(main, group.main, origin, scale)
        if equalizer != nil { place(equalizer, group.eq, origin, scale) }
        if playlist != nil { place(playlist, group.playlist, origin, scale) }
        if browser != nil { place(browser, group.browser, origin, scale) }
        if visualizer != nil { place(visualizer, group.vis, origin, scale) }
    }

    private func place(_ window: NSWindow?, _ frame: LlampFrame, _ origin: NSPoint, _ scale: CGFloat) {
        guard let window else { return }
        let width = CGFloat(frame.w) * scale
        let height = CGFloat(frame.h) * scale
        window.setFrame(NSRect(
            x: origin.x + CGFloat(frame.x) * scale,
            y: origin.y - CGFloat(frame.y) * scale - height,
            width: width,
            height: height
        ), display: false)
    }
}

private struct Frame {
    var x: Int32
    var y: Int32
}

private final class EqPresetMenu: NSWindow {
    private let list: EqPresetList

    init(owner: EqChromeView) {
        list = EqPresetList(owner: owner)
        let frame = NSRect(x: 0, y: 0, width: 180, height: list.height)
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        isOpaque = true
        hasShadow = false
        level = .popUpMenu
        contentView = list
    }
}

private final class EqPresetList: NSView {
    weak var owner: EqChromeView?
    private var names: [String] = []
    private var field: NSTextField?
    private let row = 14
    let height: CGFloat

    init(owner: EqChromeView) {
        self.owner = owner
        let count = Int(llamp_eq_preset_count())
        for index in 0..<count {
            var buf = [CChar](repeating: 0, count: 128)
            if llamp_eq_preset_name(UInt32(index), &buf, buf.count) == LLAMP_OK {
                names.append(String(decoding: buf.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }, as: UTF8.self))
            }
        }
        height = CGFloat((names.count + 3) * row)
        super.init(frame: NSRect(x: 0, y: 0, width: 180, height: height))
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let fg = color(llamp_text_color())
        let bg = color(llamp_text_bg())
        bg.setFill()
        bounds.fill()
        let font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        let rows = names + ["Save Preset", "Save Auto-load Preset", "Save Default"]
        for (index, title) in rows.enumerated() {
            let text = NSAttributedString(string: title, attributes: [.foregroundColor: fg, .font: font])
            text.draw(at: NSPoint(x: 4, y: CGFloat(index * row) + 1))
        }
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let index = Int(point.y) / row
        if index < names.count {
            let name = names[index]
            _ = name.withCString { llamp_eq_load_preset($0) }
            owner?.dismissPresets()
            return
        }
        switch index - names.count {
        case 0:
            askName()
        case 1:
            _ = llamp_eq_save_autoload()
            owner?.dismissPresets()
        case 2:
            _ = llamp_eq_save_default()
            owner?.dismissPresets()
        default:
            break
        }
    }

    private func askName() {
        let input = NSTextField(frame: NSRect(x: 4, y: height - CGFloat(row) - 2, width: 172, height: 16))
        input.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        input.textColor = color(llamp_text_color())
        input.backgroundColor = color(llamp_text_bg())
        addSubview(input)
        window?.makeFirstResponder(input)
        field = input
        input.target = self
        input.action = #selector(commitName)
    }

    @objc private func commitName() {
        let name = field?.stringValue ?? ""
        guard !name.isEmpty else { return }
        _ = name.withCString { llamp_eq_save_preset($0) }
        owner?.dismissPresets()
    }

    private func color(_ packed: UInt32) -> NSColor {
        NSColor(
            srgbRed: CGFloat((packed >> 16) & 0xFF) / 255,
            green: CGFloat((packed >> 8) & 0xFF) / 255,
            blue: CGFloat(packed & 0xFF) / 255,
            alpha: 1
        )
    }
}
