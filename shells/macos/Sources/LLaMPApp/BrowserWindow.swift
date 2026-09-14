import AppKit
import llamp_ffi

/// Fourth window. Fallback frame. CoreText only. No invented gen step.
public final class BrowserWindow: NSWindow {
    let list: BrowserListView

    public init() {
        let size = llamp_browser_size()
        let frame = NSRect(x: 120, y: 40, width: CGFloat(size.width), height: CGFloat(size.height))
        list = BrowserListView(frame: frame)
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        isOpaque = true
        backgroundColor = NSColor(srgbRed: 0x80 / 255, green: 0x80 / 255, blue: 0x80 / 255, alpha: 1)
        hasShadow = false
        title = "Library"
        contentView = list
        isMovableByWindowBackground = false
        list.onBackgroundDrag = { event in
            WindowDock.shared.track(which: 3, event: event)
        }
    }

    public var usesBitmapFont: Bool { false }

    /// Lists granted paths. CoreText only. Does not copy audio.
    public func reloadGranted() {
        list.reloadGranted()
    }
}

public final class BrowserListView: NSView {
    var rows: [String] = []
    var onBackgroundDrag: ((NSEvent) -> Void)?

    public override var isFlipped: Bool { true }

    public override func draw(_ dirtyRect: NSRect) {
        PleditText.color(llamp_text_bg()).setFill()
        bounds.fill()
        let chrome = llamp_gen_blit()
        defer { llamp_image_free(chrome.data, chrome.len) }
        if let data = chrome.data, chrome.width > 0 {
            let image = NSImage(size: NSSize(width: Int(chrome.width), height: Int(chrome.height)))
            image.lockFocus()
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
                bits.draw(in: NSRect(x: 0, y: 0, width: CGFloat(chrome.width), height: CGFloat(chrome.height)))
            }
            image.unlockFocus()
            image.draw(in: bounds, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.none])
        }
        let fg = PleditText.color(llamp_text_color())
        let bg = PleditText.color(llamp_text_bg())
        if rows.isEmpty {
            PleditText.draw("Library", in: NSRect(x: 10, y: 16, width: 80, height: 7), scale: 1, color: fg, background: bg)
            return
        }
        for (index, row) in rows.enumerated() {
            let rect = NSRect(x: 10, y: CGFloat(16 + index * 7), width: bounds.width - 20, height: 7)
            PleditText.draw(row, in: rect, scale: 1, color: fg, background: bg)
        }
    }

    public override func mouseDown(with event: NSEvent) {
        onBackgroundDrag?(event)
    }

    public func rowFontIsCoreText(_ text: String) -> Bool {
        text.withCString { llamp_browser_row_font($0) == 1 }
    }

    func reloadGranted() {
        let count = Int(llamp_browser_load_granted())
        rows = (0..<count).compactMap { index in
            var buf = [CChar](repeating: 0, count: 1024)
            guard llamp_browser_row_path(UInt32(index), &buf, buf.count) == LLAMP_OK else {
                return nil
            }
            let end = buf.firstIndex(of: 0) ?? buf.endIndex
            return String(decoding: buf[..<end].map { UInt8(bitPattern: $0) }, as: UTF8.self)
        }
        needsDisplay = true
    }
}
