import AppKit
import QuartzCore
import llamp_ffi

/// Fifth window. Core chrome owns the border. The wgpu view is the client hole.
public final class VisWindow: NSWindow {
    let chrome: VisChromeView
    private var generation: UInt64 = 1
    private var fullscreened = false

    public init() {
        let min = llamp_vis_min_size()
        let frame = NSRect(x: 160, y: 20, width: CGFloat(min.width), height: CGFloat(min.height))
        chrome = VisChromeView(frame: frame)
        super.init(contentRect: frame, styleMask: [.borderless, .resizable], backing: .buffered, defer: false)
        isOpaque = true
        backgroundColor = NSColor(srgbRed: 0x80 / 255, green: 0x80 / 255, blue: 0x80 / 255, alpha: 1)
        hasShadow = false
        title = "Visualizer"
        minSize = NSSize(width: CGFloat(min.width), height: CGFloat(min.height))
        contentView = chrome
        isMovableByWindowBackground = false
        chrome.onBackgroundDrag = { [weak self] event in
            _ = self
            WindowDock.shared.track(which: 4, event: event)
        }
        chrome.onClose = { [weak self] in self?.close() }
        chrome.onFullscreen = { [weak self] in self?.toggleFullscreen() }
        bindLayer()
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(occlusionChanged),
            name: NSWindow.didChangeOcclusionStateNotification,
            object: self
        )
    }

    @objc private func occlusionChanged() {
        llamp_vis_set_occluded(occlusionState.contains(.visible) ? 0 : 1)
    }

    public var isFullscreened: Bool { fullscreened }

    public func toggleFullscreen() {
        fullscreened.toggle()
        chrome.fullscreen = fullscreened
        if fullscreened, let screen = screen ?? NSScreen.main {
            setFrame(screen.frame, display: true)
        } else {
            let min = llamp_vis_min_size()
            setContentSize(NSSize(width: CGFloat(min.width), height: CGFloat(min.height)))
        }
        chrome.layoutGpu()
    }

    public override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            if fullscreened { toggleFullscreen() }
            return
        }
        super.keyDown(with: event)
    }

    public override func close() {
        llamp_vis_surface_invalidate(generation)
        generation &+= 1
        super.close()
    }

    public func replaceLayer() {
        llamp_vis_surface_invalidate(generation)
        generation &+= 1
        chrome.rebuildLayer()
        bindLayer()
    }

    func bindLayer() {
        chrome.gpu.wantsLayer = true
        let layer = chrome.gpu.makeMetalLayer()
        let view = UnsafeMutableRawPointer(Unmanaged.passUnretained(chrome.gpu).toOpaque())
        let metal = UnsafeMutableRawPointer(Unmanaged.passUnretained(layer).toOpaque())
        _ = llamp_vis_surface_bind(metal, generation)
        _ = llamp_vis_surface_bind_view(view, generation)
        chrome.layoutGpu()
        let size = chrome.gpu.bounds.size
        _ = llamp_vis_surface_resize(UInt32(max(size.width, 1)), UInt32(max(size.height, 1)), generation)
    }

}

public final class VisChromeView: NSView {
    let gpu = VisGpuView(frame: .zero)
    var fullscreen = false
    var onBackgroundDrag: ((NSEvent) -> Void)?
    var onClose: (() -> Void)?
    var onFullscreen: (() -> Void)?

    public override var isFlipped: Bool { true }

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        addSubview(gpu)
        postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(frameChanged),
            name: NSView.frameDidChangeNotification,
            object: self
        )
    }

    required init?(coder: NSCoder) { fatalError("init(coder:)") }

    public override func draw(_ dirtyRect: NSRect) {
        if fullscreen {
            NSColor.black.setFill()
            bounds.fill()
            return
        }
        let size = llamp_vis_propose_size(Int32(bounds.width.rounded()), Int32(bounds.height.rounded()))
        let chrome = llamp_vis_chrome_blit(size.width, size.height)
        defer { llamp_image_free(chrome.data, chrome.len) }
        guard let data = chrome.data, chrome.width > 0 else { return }
        if let bits = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: Int(chrome.width),
            pixelsHigh: Int(chrome.height),
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: Int(chrome.width) * 4,
            bitsPerPixel: 32
        ), let plane = bits.bitmapData {
            plane.update(from: data, count: Int(chrome.width) * Int(chrome.height) * 4)
            bits.draw(in: bounds)
        }
    }

    public override func mouseDown(with event: NSEvent) {
        let p = convert(event.locationInWindow, from: nil)
        if !fullscreen && p.x > bounds.width - 24 && p.y < 14 {
            onClose?()
            return
        }
        if !fullscreen && p.x > bounds.width - 48 && p.y < 14 {
            onFullscreen?()
            return
        }
        onBackgroundDrag?(event)
    }

    func rebuildLayer() {
        gpu.rebuildLayer()
    }

    func layoutGpu() {
        let hole = llamp_vis_client_rect(
            Int32(bounds.width.rounded()),
            Int32(bounds.height.rounded()),
            fullscreen ? 1 : 0
        )
        gpu.frame = NSRect(x: CGFloat(hole.x), y: CGFloat(hole.y), width: CGFloat(hole.w), height: CGFloat(hole.h))
        let size = gpu.bounds.size
        _ = llamp_vis_surface_resize(
            UInt32(max(size.width, 1)),
            UInt32(max(size.height, 1)),
            llamp_vis_surface_generation()
        )
        needsDisplay = true
    }

    @objc private func frameChanged() {
        layoutGpu()
    }
}

public final class VisGpuView: NSView {
    public override var wantsUpdateLayer: Bool { true }
    public override var isFlipped: Bool { true }

    public override func makeBackingLayer() -> CALayer {
        makeMetalLayer()
    }

    func makeMetalLayer() -> CAMetalLayer {
        let layer = CAMetalLayer()
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        self.layer = layer
        wantsLayer = true
        return layer
    }

    func rebuildLayer() {
        _ = makeMetalLayer()
    }
}
