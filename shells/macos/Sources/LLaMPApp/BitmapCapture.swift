import AppKit
import CoreGraphics

/// The only automated screenshot path.
///
/// Renders an `NSView` into an `NSBitmapImageRep` whose color space is pinned
/// sRGB and which carries no display profile. This draws the view into that
/// bitmap. It does not call `CGWindowListCreateImage`, `screencapture`, or any
/// other window-server capture. A window-server capture is color-managed and
/// is a manual check only.
protocol CapturePainting: NSView {
    /// Top-down straight RGBA at `pixelsWide` by `pixelsHigh`.
    /// Scale with the layer magnification filter: nearest stays hard, linear blends.
    func capturePixels(pixelsWide: Int, pixelsHigh: Int) -> [UInt8]
}

enum BitmapCapture {
    struct Result {
        let image: NSBitmapImageRep
        /// Top-down straight RGBA8, row-major, no row padding. Read back from `image`.
        /// Same layout as the phase 2 PNG.
        let rgba: [UInt8]
    }

    @MainActor
    static func render(_ view: NSView, pixelsWide: Int, pixelsHigh: Int) -> Result {
        precondition(pixelsWide > 0 && pixelsHigh > 0)
        let bounds = view.bounds
        precondition(bounds.width > 0 && bounds.height > 0)
        view.wantsLayer = true
        let rowBytes = pixelsWide * 4
        var pixels = [UInt8](repeating: 0, count: rowBytes * pixelsHigh)
        let cgImage: CGImage = pixels.withUnsafeMutableBytes { raw in
            guard let base = raw.baseAddress else { preconditionFailure("bitmap") }
            guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { preconditionFailure("sRGB") }
            // `CGImageAlphaInfo.last` is not a supported bitmap-context format.
            // Premultiplied last is. The phase 2 golden is opaque, so the bytes
            // match straight RGBA. The read-back un-premultiplies any other alpha.
            let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
            guard let context = CGContext(
                data: base,
                width: pixelsWide,
                height: pixelsHigh,
                bitsPerComponent: 8,
                bytesPerRow: rowBytes,
                space: space,
                bitmapInfo: bitmapInfo
            ) else { preconditionFailure("context") }
            // Do not force interpolationQuality. The layer magnification filter
            // is what scales skin pixels into this backing store. Forcing `.none`
            // here would hide a linear filter.
            if let painter = view as? CapturePainting {
                let pixels = painter.capturePixels(pixelsWide: pixelsWide, pixelsHigh: pixelsHigh)
                pixels.withUnsafeBytes { src in
                    guard let from = src.bindMemory(to: UInt8.self).baseAddress else { return }
                    base.copyMemory(from: from, byteCount: min(pixels.count, rowBytes * pixelsHigh))
                }
            } else {
                let scaleX = CGFloat(pixelsWide) / bounds.width
                let scaleY = CGFloat(pixelsHigh) / bounds.height
                context.translateBy(x: 0, y: CGFloat(pixelsHigh))
                context.scaleBy(x: scaleX, y: -scaleY)
                view.layoutSubtreeIfNeeded()
                view.layer?.render(in: context)
            }
            guard let image = context.makeImage() else { preconditionFailure("cgImage") }
            return image
        }
        // `init(cgImage:)` keeps the source color space. This image was drawn in
        // CGColorSpace.sRGB. There is no display profile and no window-server capture.
        let rep = NSBitmapImageRep(cgImage: cgImage)
        return Result(image: rep, rgba: straightRGBA(from: rep))
    }

    /// Bytes from the bitmap rep, top-down, straight R, G, B, A.
    private static func straightRGBA(from rep: NSBitmapImageRep) -> [UInt8] {
        let width = rep.pixelsWide
        let height = rep.pixelsHigh
        guard let data = rep.bitmapData, rep.samplesPerPixel == 4, rep.bitsPerPixel == 32 else {
            preconditionFailure("bitmap layout")
        }
        let alphaFirst = rep.bitmapFormat.contains(.alphaFirst)
        let littleEndian = rep.bitmapFormat.contains(.thirtyTwoBitLittleEndian)
        let premultiplied = !rep.bitmapFormat.contains(.alphaNonpremultiplied)
        var out = [UInt8](repeating: 0, count: width * height * 4)
        for y in 0..<height {
            for x in 0..<width {
                let src = data.advanced(by: y * rep.bytesPerRow + x * 4)
                var channels = [src[0], src[1], src[2], src[3]]
                if littleEndian {
                    channels.reverse()
                }
                let (r, g, b, a): (UInt8, UInt8, UInt8, UInt8)
                if alphaFirst {
                    (r, g, b, a) = (channels[1], channels[2], channels[3], channels[0])
                } else {
                    (r, g, b, a) = (channels[0], channels[1], channels[2], channels[3])
                }
                let di = (y * width + x) * 4
                if premultiplied && a != 0 && a != 255 {
                    out[di] = UInt8(min(255, Int(r) * 255 / Int(a)))
                    out[di + 1] = UInt8(min(255, Int(g) * 255 / Int(a)))
                    out[di + 2] = UInt8(min(255, Int(b) * 255 / Int(a)))
                } else {
                    out[di] = r
                    out[di + 1] = g
                    out[di + 2] = b
                }
                out[di + 3] = a
            }
        }
        return out
    }
}
