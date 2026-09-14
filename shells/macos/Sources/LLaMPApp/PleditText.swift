import AppKit
import CoreText
import llamp_ffi

/// One CoreText strategy for every missing glyph. System face, offscreen, nearest-neighbor onto the skin grid.
enum PleditText {
    static func draw(_ string: String, in rect: NSRect, scale: Int, color: NSColor, background: NSColor) {
        let destW = max(Int((rect.width * CGFloat(max(scale, 1))).rounded()), 1)
        let destH = max(Int((rect.height * CGFloat(max(scale, 1))).rounded()), 1)
        // 1:1 offscreen. A 4× render nearest-sampled into a 7 px cell drops the strokes.
        let factor = 1
        guard let src = renderOffscreen(
            string,
            width: destW * factor,
            height: destH * factor,
            color: color,
            background: background
        ) else { return }
        let pixels = nearest(src, srcW: destW * factor, srcH: destH * factor, destW: destW, destH: destH)
        drawPixels(pixels, width: destW, height: destH, in: rect)
    }

    static func color(_ rgb: UInt32) -> NSColor {
        NSColor(
            srgbRed: CGFloat((rgb >> 16) & 0xff) / 255,
            green: CGFloat((rgb >> 8) & 0xff) / 255,
            blue: CGFloat(rgb & 0xff) / 255,
            alpha: 1
        )
    }

    private static func renderOffscreen(
        _ string: String,
        width: Int,
        height: Int,
        color: NSColor,
        background: NSColor
    ) -> [UInt8]? {
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let drew = pixels.withUnsafeMutableBytes { raw -> Bool in
            guard let base = raw.baseAddress,
                  let space = CGColorSpace(name: CGColorSpace.sRGB) else { return false }
            let info = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
            guard let ctx = CGContext(
                data: base,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: space,
                bitmapInfo: info
            ) else { return false }
            ctx.interpolationQuality = .none
            ctx.setShouldAntialias(false)
            ctx.setAllowsAntialiasing(false)
            ctx.setShouldSmoothFonts(false)
            ctx.setFillColor(background.cgColor)
            ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
            let font = NSFont.monospacedSystemFont(ofSize: max(CGFloat(height) - 1, 1), weight: .regular)
            let attr = NSAttributedString(string: string, attributes: [
                .font: font,
                .foregroundColor: color,
            ])
            let line = CTLineCreateWithAttributedString(attr)
            var ascent: CGFloat = 0
            var descent: CGFloat = 0
            CTLineGetTypographicBounds(line, &ascent, &descent, nil)
            let glyphHeight = ascent + descent
            let baseline = (CGFloat(height) - glyphHeight) / 2 + descent
            ctx.textMatrix = .identity
            ctx.setFillColor(color.cgColor)
            ctx.textPosition = CGPoint(x: 1, y: max(baseline, 0))
            CTLineDraw(line, ctx)
            return true
        }
        return drew ? pixels : nil
    }

    private static func nearest(_ src: [UInt8], srcW: Int, srcH: Int, destW: Int, destH: Int) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: destW * destH * 4)
        for y in 0..<destH {
            let sy = min(srcH - 1, y * srcH / destH)
            for x in 0..<destW {
                let sx = min(srcW - 1, x * srcW / destW)
                let from = (sy * srcW + sx) * 4
                let to = (y * destW + x) * 4
                out[to..<(to + 4)] = src[from..<(from + 4)]
            }
        }
        return out
    }

    private static func drawPixels(_ pixels: [UInt8], width: Int, height: Int, in rect: NSRect) {
        guard let bits = NSBitmapImageRep(
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
        ), let plane = bits.bitmapData else { return }
        pixels.withUnsafeBytes { raw in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return }
            plane.update(from: base, count: pixels.count)
        }
        NSGraphicsContext.current?.cgContext.interpolationQuality = .none
        bits.draw(
            in: rect,
            from: .zero,
            operation: .copy,
            fraction: 1,
            respectFlipped: true,
            hints: [.interpolation: NSImageInterpolation.none]
        )
    }
}
