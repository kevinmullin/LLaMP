import AppKit
import llamp_ffi

/// Playlist window. One custom view. Hit testing is index math on the visible window.
public final class PlaylistWindow: NSWindow {
    let list: PlaylistListView

    public init() {
        let size = llamp_playlist_size()
        let frame = NSRect(x: 80, y: 80, width: CGFloat(size.width), height: CGFloat(size.height))
        list = PlaylistListView(frame: frame)
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        isOpaque = true
        backgroundColor = .black
        hasShadow = false
        title = "Playlist"
        contentView = list
        isMovableByWindowBackground = false
        list.onBackgroundDrag = { event in
            WindowDock.shared.track(which: 2, event: event)
        }
    }

    /// Rejects a size that is not 275+25n by 116+29n. A reject does not apply.
    @discardableResult
    public func proposeSize(width: Int32, height: Int32) -> Bool {
        guard llamp_playlist_propose_size(width, height) == LLAMP_OK else { return false }
        let size = llamp_playlist_size()
        setContentSize(NSSize(width: CGFloat(size.width), height: CGFloat(size.height)))
        list.frame = NSRect(x: 0, y: 0, width: CGFloat(size.width), height: CGFloat(size.height))
        return true
    }
}

public final class PlaylistListView: NSView {
    var entries: [String] = []
    var scroll: UInt32 = 0
    var rowsConsidered = 0
    /// Destination integer scale for CoreText. Not a 5×7 offscreen.
    var textScale = 1
    var onBackgroundDrag: ((NSEvent) -> Void)?
    private var slideUp: PlaylistMenu?
    private var axRows: [NSAccessibilityElement] = []
    private var axButtons: [NSAccessibilityElement] = []

    public override var isFlipped: Bool { true }

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.magnificationFilter = .nearest
        layer?.minificationFilter = .nearest
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    public override func draw(_ dirtyRect: NSRect) {
        PleditText.color(llamp_text_bg()).setFill()
        bounds.fill()
        let count = llamp_playlist_visible_count(UInt32(entries.count), scroll)
        rowsConsidered = Int(count)
        let fg = PleditText.color(llamp_text_color())
        let bg = PleditText.color(llamp_text_bg())
        let visible = (0..<Int(count)).compactMap { slot -> String? in
            let index = Int(scroll) + slot
            return index < entries.count ? entries[index] : nil
        }
        let listCoreText = Self.listFontIsCoreText(visible)
        for slot in 0..<Int(count) {
            let index = Int(scroll) + slot
            guard index < entries.count else { break }
            let row = NSRect(x: 0, y: CGFloat(slot * 7), width: bounds.width, height: 7)
            if listCoreText {
                PleditText.draw(entries[index], in: row, scale: max(textScale, 1), color: fg, background: bg)
            } else {
                drawBitmapRow(entries[index], in: row)
            }
        }
        for index in 0..<5 {
            let button = llamp_playlist_button_at(UInt32(index))
            let rect = NSRect(x: CGFloat(button.x), y: CGFloat(button.y), width: CGFloat(button.w), height: CGFloat(button.h))
            fg.setStroke()
            NSBezierPath(rect: rect).stroke()
            if let label = button.label {
                PleditText.draw(String(cString: label), in: rect, scale: 1, color: fg, background: bg)
            }
        }
        rebuildAccessibility()
    }

    public override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        for index in 0..<5 {
            let button = llamp_playlist_button_at(UInt32(index))
            let rect = NSRect(x: CGFloat(button.x), y: CGFloat(button.y), width: CGFloat(button.w), height: CGFloat(button.h))
            if rect.contains(point), let label = button.label {
                showMenu(String(cString: label), at: rect)
                return
            }
        }
        let row = hitRow(y: Int32(point.y))
        if row < 0 {
            onBackgroundDrag?(event)
        }
    }

    /// One index calculation. Does not walk the playlist.
    public func hitRow(y: Int32) -> Int32 {
        rowsConsidered = 1
        return llamp_playlist_hit_row(y, scroll, UInt32(entries.count))
    }

    public override func accessibilityChildren() -> [Any]? {
        axButtons + axRows
    }

    static func listFontIsCoreText(_ rows: [String]) -> Bool {
        rows.contains { row in
            row.withCString { llamp_playlist_row_font($0) == 1 }
        }
    }

    private func drawBitmapRow(_ text: String, in rect: NSRect) {
        var x = rect.minX
        for ch in text {
            let scalar = ch.unicodeScalars.first.map { UInt32($0.value) } ?? 0
            let cell = NSRect(x: x, y: rect.minY, width: 5, height: 7)
            let image = llamp_text_char_blit(scalar)
            defer { llamp_image_free(image.data, image.len) }
            if let data = image.data, image.width > 0 {
                drawAtlas(data, width: Int(image.width), height: Int(image.height), in: cell)
            }
            x += 5
        }
    }

    private func drawAtlas(_ data: UnsafeMutablePointer<UInt8>, width: Int, height: Int, in rect: NSRect) {
        let image = NSImage(size: NSSize(width: width, height: height))
        image.lockFocus()
        NSGraphicsContext.current?.cgContext.interpolationQuality = .none
        if let bits = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: width,
            pixelsHigh: height,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: width * 4,
            bitsPerPixel: 32
        ), let plane = bits.bitmapData {
            plane.update(from: data, count: width * height * 4)
            bits.draw(in: NSRect(x: 0, y: 0, width: width, height: height))
        }
        image.unlockFocus()
        let dest = NSRect(x: rect.minX, y: rect.minY, width: CGFloat(width), height: CGFloat(height))
        image.draw(in: dest, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.none])
    }

    private func rebuildAccessibility() {
        axButtons = (0..<5).map { index in
            let button = llamp_playlist_button_at(UInt32(index))
            let element = NSAccessibilityElement()
            element.setAccessibilityRole(.button)
            let label = button.label.map { String(cString: $0) } ?? ""
            element.setAccessibilityLabel(label)
            element.setAccessibilityParent(self)
            element.setAccessibilityFrameInParentSpace(NSRect(x: CGFloat(button.x), y: CGFloat(button.y), width: CGFloat(button.w), height: CGFloat(button.h)))
            return element
        }
        let count = Int(llamp_playlist_visible_count(UInt32(entries.count), scroll))
        axRows = (0..<count).map { slot in
            let element = NSAccessibilityElement()
            element.setAccessibilityRole(.row)
            let index = Int(scroll) + slot
            element.setAccessibilityLabel(index < entries.count ? entries[index] : "")
            element.setAccessibilityParent(self)
            element.setAccessibilityFrameInParentSpace(NSRect(x: 0, y: CGFloat(slot * 7), width: bounds.width, height: 7))
            return element
        }
    }

    private func showMenu(_ name: String, at rect: NSRect) {
        slideUp?.orderOut(nil)
        let popup = PlaylistMenu(title: name, items: PlaylistListView.items(name))
        slideUp = popup
        var origin = convert(NSPoint(x: rect.minX, y: rect.minY), to: nil)
        origin.y += 4
        popup.setFrameOrigin(origin)
        popup.orderFront(nil)
    }

    static func items(_ name: String) -> [String] {
        switch name {
        case "Add": return ["files", "folder", "URL", "playlist"]
        case "Rem": return ["selected", "crop", "all", "remove from library"]
        case "Sel": return ["all", "none", "invert"]
        case "Misc": return ["sort by title", "sort by artist", "sort by path", "reverse", "file info"]
        case "List": return ["new", "save", "load"]
        default: return []
        }
    }
}

final class PlaylistMenu: NSWindow {
    init(title: String, items: [String]) {
        let row = 7
        let frame = NSRect(x: 0, y: 0, width: 160, height: items.count * row)
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        isOpaque = true
        hasShadow = false
        level = .popUpMenu
        contentView = PlaylistMenuList(title: title, items: items)
    }
}

final class PlaylistMenuList: NSView {
    let items: [String]

    init(title: String, items: [String]) {
        self.items = items
        super.init(frame: NSRect(x: 0, y: 0, width: 160, height: items.count * 7))
        _ = title
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let fg = PleditText.color(llamp_text_color())
        let bg = PleditText.color(llamp_text_bg())
        bg.setFill()
        bounds.fill()
        for (index, item) in items.enumerated() {
            let rect = NSRect(x: 2, y: CGFloat(index * 7), width: 156, height: 7)
            PleditText.draw(item, in: rect, scale: 1, color: fg, background: bg)
        }
    }
}
