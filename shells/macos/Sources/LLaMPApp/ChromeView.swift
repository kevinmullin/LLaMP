import AppKit
import llamp_ffi

/// Main-window content view. Draws the core's atlas blit. Does not parse BMP.
final class ChromeView: NSView, CapturePainting {
    private var skinPixels: [UInt8] = []
    private var skinWidth = 0
    private var skinHeight = 0
    private var controls: [HitControl] = []
    private var elements: [ControlElement] = []
    private var backingScale = 1
    private var sliding: HitControl?
    var onControl: ((HitControl) -> Void)?
    var onBackgroundDrag: ((NSEvent) -> Void)?

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

    func loadSkin(_ wsz: Data) {
        let loaded = wsz.withUnsafeBytes { raw -> Int32 in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress, raw.count > 0 else {
                return LLAMP_ERR_INVALID
            }
            return llamp_skin_load(base, raw.count)
        }
        guard loaded == LLAMP_OK else { return }
        let image = llamp_skin_blit_main()
        defer { llamp_image_free(image.data, image.len) }
        guard let data = image.data, image.len > 0 else { return }
        skinWidth = Int(image.width)
        skinHeight = Int(image.height)
        skinPixels = Array(UnsafeBufferPointer(start: data, count: image.len))
        installContents()
        reloadControls()
    }

    func replaceBlit(_ image: LlampImage) {
        defer { llamp_image_free(image.data, image.len) }
        guard let data = image.data, image.len > 0 else { return }
        skinWidth = Int(image.width)
        skinHeight = Int(image.height)
        skinPixels = Array(UnsafeBufferPointer(start: data, count: image.len))
        installContents()
    }

    func setBackingScale(_ scale: Int) {
        backingScale = max(1, scale)
        layer?.contentsScale = CGFloat(backingScale)
    }

    func capturePixels(pixelsWide: Int, pixelsHigh: Int) -> [UInt8] {
        guard skinWidth > 0, skinHeight > 0, !skinPixels.isEmpty else {
            return [UInt8](repeating: 0, count: pixelsWide * pixelsHigh * 4)
        }
        if layer?.magnificationFilter == .nearest {
            return scaleNearest(pixelsWide: pixelsWide, pixelsHigh: pixelsHigh)
        }
        return scaleLinear(pixelsWide: pixelsWide, pixelsHigh: pixelsHigh)
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let x = Int(point.x)
        let y = Int(point.y)
        if let control = controls.first(where: { $0.contains(x: x, y: y) && $0.label != "Title bar" }) {
            if isSlider(control) {
                sliding = control
                setSlider(control, at: x)
                return
            }
            press(control)
            return
        }
        if let onBackgroundDrag {
            onBackgroundDrag(event)
        } else {
            window?.performDrag(with: event)
        }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let control = sliding else { return }
        let point = convert(event.locationInWindow, from: nil)
        setSlider(control, at: Int(point.x))
    }

    override func mouseUp(with event: NSEvent) {
        sliding = nil
        super.mouseUp(with: event)
    }

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 49:
            llamp_transport_toggle_play()
        case 123:
            seek(sign: -1)
        case 124:
            seek(sign: 1)
        default:
            super.keyDown(with: event)
        }
    }

    override func isAccessibilityElement() -> Bool { true }

    override func accessibilityRole() -> NSAccessibility.Role? { .group }

    override func accessibilityChildren() -> [Any]? { elements }

    private func press(_ control: HitControl) {
        llamp_transport_press(control.id)
        onControl?(control)
    }

    private func isSlider(_ control: HitControl) -> Bool {
        control.label == "Seek" || control.label == "Volume" || control.label == "Balance"
    }

    private func setSlider(_ control: HitControl, at x: Int) {
        guard control.w > 1 else { return }
        let rel = min(max(x - control.x, 0), control.w - 1)
        let ppm = UInt16((rel * 1000) / (control.w - 1))
        llamp_transport_set_slider(control.id, ppm)
    }

    private func seek(sign: Int32) {
        let rate = llamp_playback_poll().sample_rate
        guard let step = Int32(exactly: rate) else { return }
        llamp_transport_seek_by(step * sign)
    }

    private func reloadControls() {
        controls = []
        elements = []
        let count = llamp_control_count()
        for index in 0..<count {
            let raw = llamp_control_at(index)
            guard let label = raw.label.flatMap({ String(cString: $0) }), !label.isEmpty else { continue }
            let hit = HitControl(id: raw.id, label: label, x: Int(raw.x), y: Int(raw.y), w: Int(raw.w), h: Int(raw.h))
            controls.append(hit)
            let element = ControlElement(control: hit, view: self)
            elements.append(element)
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

    private func scaleNearest(pixelsWide: Int, pixelsHigh: Int) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: pixelsWide * pixelsHigh * 4)
        for y in 0..<pixelsHigh {
            let sy = min(skinHeight - 1, y * skinHeight / pixelsHigh)
            for x in 0..<pixelsWide {
                let sx = min(skinWidth - 1, x * skinWidth / pixelsWide)
                let si = (sy * skinWidth + sx) * 4
                let di = (y * pixelsWide + x) * 4
                out[di..<(di + 4)] = skinPixels[si..<(si + 4)]
            }
        }
        return out
    }

    private func scaleLinear(pixelsWide: Int, pixelsHigh: Int) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: pixelsWide * pixelsHigh * 4)
        for y in 0..<pixelsHigh {
            let sy = min(skinHeight - 1, y * skinHeight / pixelsHigh)
            let sy2 = min(skinHeight - 1, sy + 1)
            for x in 0..<pixelsWide {
                let sx = min(skinWidth - 1, x * skinWidth / pixelsWide)
                let a = (sy * skinWidth + sx) * 4
                let b = (sy2 * skinWidth + sx) * 4
                let di = (y * pixelsWide + x) * 4
                for channel in 0..<4 {
                    out[di + channel] = UInt8((Int(skinPixels[a + channel]) + Int(skinPixels[b + channel])) / 2)
                }
            }
        }
        return out
    }
}

struct HitControl {
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
final class ControlElement: NSAccessibilityElement {
    let control: HitControl
    weak var view: ChromeView?

    init(control: HitControl, view: ChromeView) {
        self.control = control
        self.view = view
        super.init()
        setAccessibilityLabel(control.label)
        setAccessibilityRole(role(for: control.label))
        setAccessibilityParent(view)
        setAccessibilityFrameInParentSpace(NSRect(x: control.x, y: control.y, width: control.w, height: control.h))
    }

    override func accessibilityActionNames() -> [NSAccessibility.Action] {
        switch accessibilityRole() {
        case .slider:
            return [.increment, .decrement]
        case .staticText:
            return []
        default:
            return [.press]
        }
    }

    override func accessibilityPerformPress() -> Bool {
        guard let view else { return false }
        llamp_transport_press(control.id)
        view.onControl?(control)
        return true
    }

    override func accessibilityPerformIncrement() -> Bool {
        if control.label == "Seek" {
            let rate = llamp_playback_poll().sample_rate
            if let step = Int32(exactly: rate) {
                llamp_transport_seek_by(step)
            }
        } else {
            nudgeSlider(id: control.id, label: control.label, by: 50)
        }
        return true
    }

    override func accessibilityPerformDecrement() -> Bool {
        if control.label == "Seek" {
            let rate = llamp_playback_poll().sample_rate
            if let step = Int32(exactly: rate) {
                llamp_transport_seek_by(-step)
            }
        } else {
            nudgeSlider(id: control.id, label: control.label, by: -50)
        }
        return true
    }
}

private func nudgeSlider(id: UInt32, label: String, by delta: Int) {
    let snap = llamp_playback_poll()
    let current = label == "Balance" ? Int(snap.balance_ppm) : Int(snap.volume_ppm)
    let next = min(max(current + delta, 0), 1000)
    llamp_transport_set_slider(id, UInt16(next))
}

private func role(for label: String) -> NSAccessibility.Role {
    switch label {
    case "Seek", "Volume", "Balance":
        return .slider
    case "Mono", "Stereo":
        return .staticText
    default:
        return .button
    }
}
