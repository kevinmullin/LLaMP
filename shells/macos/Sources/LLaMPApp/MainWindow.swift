import AppKit
import llamp_ffi

/// Borderless main window. Size, shade height, and hit regions come from the core.
public final class MainWindow: NSWindow {
    let chrome: ChromeView
    private var shaded = false
    private var skinScale = 1
    private var marqueeSkip: UInt32 = 0
    private var lastPull = ""
    private var displayTimer: Timer?

    public init() {
        let size = llamp_main_size()
        let frame = NSRect(x: 0, y: 0, width: CGFloat(size.width), height: CGFloat(size.height))
        chrome = ChromeView(frame: frame)
        super.init(
            contentRect: frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .normal
        contentView = chrome
        isMovableByWindowBackground = false
        chrome.onControl = { [weak self] control in
            self?.apply(control)
        }
        displayTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.pullDisplay()
            }
        }
    }

    private func pullDisplay() {
        let snap = llamp_playback_poll()
        let key = "\(snap.position_frames)-\(snap.transport)-\(snap.time_remaining)-\(snap.volume_ppm)-\(snap.balance_ppm)-\(snap.title_len)"
        let idle = snap.transport == 0 && snap.position_frames == 0 && snap.title_len == 0
            && snap.time_remaining == 0 && snap.volume_ppm == 0 && snap.balance_ppm == 500
        guard !idle else { return }
        if key == lastPull { return }
        lastPull = key
        if snap.position_frames % 30 == 0 { marqueeSkip &+= 1 }
        chrome.replaceBlit(llamp_skin_blit_display(marqueeSkip))
    }

    var skinHeight: Int {
        shaded ? Int(llamp_shade_height()) : Int(llamp_main_size().height)
    }

    public func loadSkin(_ wsz: Data) {
        chrome.loadSkin(wsz)
        applyMask(mode: 0)
        resizeToSkin()
    }

    func toggleShade() {
        shaded.toggle()
        applyMask(mode: shaded ? 1 : 0)
        resizeToSkin()
    }

    private func apply(_ control: HitControl) {
        let snap = llamp_playback_poll()
        level = snap.always_on_top == 0 ? .normal : .floating
        let nextScale = snap.double_size == 0 ? 1 : 2
        if nextScale != skinScale {
            skinScale = nextScale
            resizeToSkin()
        }
        switch control.label {
        case "Shade":
            toggleShade()
        case "Close":
            close()
        case "Minimize":
            miniaturize(nil)
        default:
            break
        }
    }

    private func resizeToSkin() {
        let size = llamp_main_size()
        let height = shaded ? llamp_shade_height() : size.height
        let width = CGFloat(size.width * UInt32(skinScale))
        let points = CGFloat(height * UInt32(skinScale))
        setContentSize(NSSize(width: width, height: points))
        chrome.setFrameSize(NSSize(width: width, height: points))
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
