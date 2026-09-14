import AppKit
import CoreText
import llamp_ffi

/// Sixth window. Core `gen` chrome. CoreText in the client hole at a readable size.
public final class LyricsWindow: NSWindow {
    let chrome: LyricsChromeView

    public init() {
        let min = llamp_lyrics_min_size()
        let frame = NSRect(x: 200, y: 20, width: CGFloat(min.width), height: CGFloat(min.height))
        chrome = LyricsChromeView(frame: frame)
        super.init(contentRect: frame, styleMask: [.borderless, .resizable], backing: .buffered, defer: false)
        isOpaque = true
        backgroundColor = NSColor(srgbRed: 0x80 / 255, green: 0x80 / 255, blue: 0x80 / 255, alpha: 1)
        hasShadow = false
        title = "Lyrics"
        minSize = NSSize(width: CGFloat(min.width), height: CGFloat(min.height))
        contentView = chrome
        isMovableByWindowBackground = false
        chrome.onBackgroundDrag = { event in
            WindowDock.shared.track(which: 5, event: event)
        }
        chrome.onClose = { [weak self] in self?.close() }
    }

    public func load(path: String) {
        _ = path.withCString { llamp_lyrics_load($0) }
        chrome.sidecarWarning = nil
        chrome.needsDisplay = true
    }

    public func stepOffset(_ steps: Int32) {
        chrome.stepOffset(steps)
    }

    @discardableResult
    public func saveOffset() -> Bool {
        chrome.saveOffset()
    }
}

public final class LyricsChromeView: NSView {
    var onBackgroundDrag: ((NSEvent) -> Void)?
    var onClose: (() -> Void)?
    var scale = 1
    var scrollPx: Int32 = 0
    var sidecarWarning: String?

    public override var isFlipped: Bool { true }

    public override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.magnificationFilter = .nearest
        layer?.minificationFilter = .nearest
        postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(frameChanged),
            name: NSView.frameDidChangeNotification,
            object: self
        )
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    public override func draw(_ dirtyRect: NSRect) {
        let size = llamp_lyrics_propose_size(Int32(bounds.width.rounded()), Int32(bounds.height.rounded()))
        let chrome = llamp_lyrics_chrome_blit(size.width, size.height)
        defer { llamp_image_free(chrome.data, chrome.len) }
        if let data = chrome.data, chrome.width > 0,
           let bits = NSBitmapImageRep(
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
            bits.draw(in: bounds, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.none])
        }
        drawText()
    }

    private func drawText() {
        let kind = llamp_lyrics_kind()
        let normal = PleditText.color(llamp_text_color())
        let current = PleditText.color(llamp_text_current())
        let bg = PleditText.color(llamp_text_bg())
        let selected = PleditText.color(llamp_text_selected_bg())
        if kind == 0 {
            drawEmpty(normal: normal, background: bg)
            return
        }
        let count = Int(llamp_lyrics_line_count())
        let active = Int(llamp_lyrics_active_line())
        let activeWord = Int(llamp_lyrics_active_word())
        for index in 0..<count {
            let rect = llamp_lyrics_line_rect(Int32(index), scrollPx, Int32(scale))
            let frame = NSRect(x: CGFloat(rect.x), y: CGFloat(rect.y), width: CGFloat(rect.w), height: CGFloat(rect.h))
            if kind == 1 {
                drawReadable(lineText(index), in: frame, color: normal, background: bg)
                continue
            }
            if index == active {
                selected.setFill()
                frame.fill()
                if llamp_lyrics_word_count(UInt32(index)) > 0 {
                    drawWords(line: index, in: frame, activeWord: activeWord, current: current, idle: normal, background: selected)
                } else {
                    drawReadable(lineText(index), in: frame, color: current, background: selected)
                }
            } else {
                drawReadable(lineText(index), in: frame, color: normal, background: bg)
            }
        }
        if llamp_lyrics_from_lrclib() != 0 {
            let note = NSRect(x: 12, y: bounds.height - 18, width: bounds.width - 24, height: 14)
            drawReadable("LRCLIB", in: note, color: normal, background: bg)
        }
        drawOffsetBar(normal: normal, background: bg)
    }

    private func drawEmpty(normal: NSColor, background: NSColor) {
        let mark = llamp_lyrics_mark_rect(Int32(scale))
        let markRect = NSRect(x: CGFloat(mark.x), y: CGFloat(mark.y), width: CGFloat(mark.w), height: CGFloat(mark.h))
        if let image = markImage() {
            image.draw(
                in: markRect,
                from: NSRect.zero,
                operation: NSCompositingOperation.copy,
                fraction: 1,
                respectFlipped: true,
                hints: [NSImageRep.HintKey.interpolation: NSImageInterpolation.none]
            )
        } else {
            normal.setFill()
            markRect.frame()
        }
        let message = NSRect(x: 16, y: markRect.maxY + 8, width: bounds.width - 32, height: 36)
        drawReadable("No lyrics", in: message, color: normal, background: background)
        if llamp_lyrics_consent() == 0, let note = privacyNote() {
            let privacy = NSRect(x: 16, y: message.maxY + 4, width: bounds.width - 32, height: 48)
            drawReadable(note, in: privacy, color: normal, background: background)
        }
    }

    private func drawWords(line: Int, in rect: NSRect, activeWord: Int, current: NSColor, idle: NSColor, background: NSColor) {
        let count = Int(llamp_lyrics_word_count(UInt32(line)))
        var x = rect.minX
        for index in 0..<count {
            let word = wordText(line, index)
            let color = index == activeWord ? current : idle
            let width = readableWidth(word, height: rect.height)
            drawReadable(word, in: NSRect(x: x, y: rect.minY, width: width, height: rect.height), color: color, background: background)
            x += width + 6
        }
    }

    private func drawReadable(_ text: String, in rect: NSRect, color: NSColor, background: NSColor) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setShouldAntialias(true)
        ctx.setFillColor(background.cgColor)
        let font = NSFont.systemFont(ofSize: max(rect.height - 2, 11), weight: .regular)
        let attr = NSAttributedString(string: text, attributes: [
            .font: font,
            .foregroundColor: color,
        ])
        let line = CTLineCreateWithAttributedString(attr)
        ctx.textMatrix = .identity
        ctx.textPosition = CGPoint(x: rect.minX + 1, y: rect.minY + 2)
        CTLineDraw(line, ctx)
    }

    private func readableWidth(_ text: String, height: CGFloat) -> CGFloat {
        let font = NSFont.systemFont(ofSize: max(height - 2, 11), weight: .regular)
        let attr = NSAttributedString(string: text, attributes: [.font: font])
        return CGFloat(CTLineGetTypographicBounds(CTLineCreateWithAttributedString(attr), nil, nil, nil)) + 2
    }

    private func lineText(_ index: Int) -> String {
        var buf = [CChar](repeating: 0, count: 2048)
        guard llamp_lyrics_line_text(UInt32(index), &buf, buf.count) == LLAMP_OK else { return "" }
        return decodeCString(buf)
    }

    private func wordText(_ line: Int, _ word: Int) -> String {
        var buf = [CChar](repeating: 0, count: 256)
        guard llamp_lyrics_word_text(UInt32(line), UInt32(word), &buf, buf.count) == LLAMP_OK else { return "" }
        return decodeCString(buf)
    }

    private func privacyNote() -> String? {
        guard let ptr = llamp_lyrics_privacy_note() else { return nil }
        return String(cString: ptr)
    }

    private func sidecarNote() -> String {
        guard let ptr = llamp_lyrics_sidecar_note() else {
            return "The sidecar was not written."
        }
        return String(cString: ptr)
    }

    func stepOffset(_ steps: Int32) {
        _ = llamp_lyrics_step_offset(steps)
        needsDisplay = true
    }

    @discardableResult
    func saveOffset() -> Bool {
        let ok = llamp_lyrics_save_offset() == LLAMP_OK
        if ok {
            sidecarWarning = llamp_lyrics_sidecar_written() == 0 ? sidecarNote() : nil
        }
        needsDisplay = true
        return ok
    }

    private func drawOffsetBar(normal: NSColor, background: NSColor) {
        if llamp_lyrics_kind() == 0 { return }
        let minus = offsetHit(.earlier)
        let save = offsetHit(.save)
        let plus = offsetHit(.later)
        drawReadable("−", in: minus, color: normal, background: background)
        drawReadable("Save", in: save, color: normal, background: background)
        drawReadable("+", in: plus, color: normal, background: background)
        let ms = llamp_lyrics_offset_ms()
        let label = NSRect(x: plus.maxX + 6, y: plus.minY, width: 80, height: plus.height)
        drawReadable("\(ms) ms", in: label, color: normal, background: background)
        if let warning = sidecarWarning {
            let note = NSRect(x: 12, y: minus.minY - 16, width: bounds.width - 24, height: 14)
            drawReadable(warning, in: note, color: normal, background: background)
        }
    }

    fileprivate func offsetHit(_ which: LyricsOffsetAction) -> NSRect {
        let client = llamp_lyrics_client_rect(Int32(bounds.width.rounded()), Int32(bounds.height.rounded()))
        let y = CGFloat(client.y + client.h - 16)
        switch which {
        case .earlier: return NSRect(x: 12, y: y, width: 16, height: 14)
        case .save: return NSRect(x: 32, y: y, width: 40, height: 14)
        case .later: return NSRect(x: 76, y: y, width: 16, height: 14)
        }
    }

    fileprivate func performOffset(_ action: LyricsOffsetAction) {
        switch action {
        case .earlier: stepOffset(-1)
        case .later: stepOffset(1)
        case .save: _ = saveOffset()
        }
    }

    public override func accessibilityChildren() -> [Any]? {
        guard llamp_lyrics_kind() != 0 else { return nil }
        return [LyricsOffsetAction.earlier, .save, .later].map { action in
            let element = LyricsOffsetElement()
            element.action = action
            element.view = self
            element.setAccessibilityRole(.button)
            element.setAccessibilityLabel(action.label)
            element.setAccessibilityParent(self)
            element.setAccessibilityFrameInParentSpace(offsetHit(action))
            return element
        }
    }

    private func decodeCString(_ buf: [CChar]) -> String {
        let end = buf.firstIndex(of: 0) ?? buf.endIndex
        return String(decoding: buf[..<end].map { UInt8(bitPattern: $0) }, as: UTF8.self)
    }

    /// Canonical mark is `assets/brand/llamp-mark.png`. LLaMPApp is not a resource module.
    private func markImage() -> NSImage? {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            Bundle.main.url(forResource: "llamp-mark", withExtension: "png"),
            cwd.appendingPathComponent("assets/brand/llamp-mark.png"),
            cwd.appendingPathComponent("../../assets/brand/llamp-mark.png"),
        ]
        for url in candidates.compactMap({ $0 }) where FileManager.default.fileExists(atPath: url.path) {
            if let image = NSImage(contentsOf: url) {
                return image
            }
        }
        return nil
    }

    public override func mouseDown(with event: NSEvent) {
        let p = convert(event.locationInWindow, from: nil)
        if p.x > bounds.width - 24 && p.y < 14 {
            onClose?()
            return
        }
        if llamp_lyrics_kind() == 0 && llamp_lyrics_consent() == 0 && p.y > bounds.midY {
            _ = llamp_lyrics_set_consent(1)
            _ = llamp_lyrics_lookup()
            needsDisplay = true
            return
        }
        if llamp_lyrics_kind() != 0 {
            for action in [LyricsOffsetAction.earlier, .save, .later] {
                if offsetHit(action).contains(p) {
                    performOffset(action)
                    return
                }
            }
        }
        onBackgroundDrag?(event)
    }

    public override func scrollWheel(with event: NSEvent) {
        scrollPx += Int32((-event.scrollingDeltaY).rounded())
        if scrollPx < 0 { scrollPx = 0 }
        needsDisplay = true
    }

    @objc private func frameChanged() {
        _ = llamp_lyrics_propose_size(Int32(bounds.width.rounded()), Int32(bounds.height.rounded()))
        needsDisplay = true
    }
}

enum LyricsOffsetAction {
    case earlier, later, save

    var label: String {
        switch self {
        case .earlier: return "Offset earlier"
        case .later: return "Offset later"
        case .save: return "Save offset"
        }
    }
}

final class LyricsOffsetElement: NSAccessibilityElement {
    var action: LyricsOffsetAction = .save
    weak var view: LyricsChromeView?

    override func accessibilityPerformPress() -> Bool {
        view?.performOffset(action)
        return true
    }
}
