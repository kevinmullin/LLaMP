import Foundation
import GoldenInflate
import XCTest

/// Decodes the phase 2 PNG as raw RGBA. Does not use ImageIO or a display profile.
enum GoldenPNG {
    struct Image {
        let width: Int
        let height: Int
        let rgba: [UInt8]
        let srgb: Bool
        let iccp: Bool
        let gama: Bool
        let chrm: Bool
    }

    static func decode(_ data: Data) throws -> Image {
        let bytes = [UInt8](data)
        let sig: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        guard bytes.starts(with: sig) else { throw DecodeError("not a png") }
        var offset = 8
        var width = 0
        var height = 0
        var bitDepth = 0
        var colorType = 0
        var interlace = 0
        var idat = [UInt8]()
        var srgb = false
        var iccp = false
        var gama = false
        var chrm = false
        while offset + 12 <= bytes.count {
            let length = Int(readU32(bytes, offset))
            let type = String(bytes: bytes[(offset + 4)..<(offset + 8)], encoding: .ascii) ?? ""
            let start = offset + 8
            let end = start + length
            guard end + 4 <= bytes.count else { throw DecodeError("truncated chunk") }
            let chunk = Array(bytes[start..<end])
            switch type {
            case "IHDR":
                width = Int(readU32(chunk, 0))
                height = Int(readU32(chunk, 4))
                bitDepth = Int(chunk[8])
                colorType = Int(chunk[9])
                interlace = Int(chunk[12])
            case "IDAT":
                idat.append(contentsOf: chunk)
            case "sRGB":
                srgb = true
            case "iCCP":
                iccp = true
            case "gAMA":
                gama = true
            case "cHRM":
                chrm = true
            case "IEND":
                offset = bytes.count
                continue
            default:
                break
            }
            offset = end + 4
        }
        guard width == 275, height == 116, bitDepth == 8, colorType == 6, interlace == 0 else {
            throw DecodeError("unexpected ihdr \(width)x\(height) depth \(bitDepth) type \(colorType)")
        }
        let inflated = try inflate(idat, expected: (width * 4 + 1) * height)
        let rgba = try unfilter(inflated, width: width, height: height)
        return Image(width: width, height: height, rgba: rgba, srgb: srgb, iccp: iccp, gama: gama, chrm: chrm)
    }

    private static func inflate(_ src: [UInt8], expected: Int) throws -> [UInt8] {
        let dst = UnsafeMutablePointer<UInt8>.allocate(capacity: expected)
        defer { dst.deallocate() }
        var written = 0
        let status = src.withUnsafeBufferPointer { buffer -> Int32 in
            guard let base = buffer.baseAddress else { return -1 }
            return golden_inflate(base, src.count, dst, expected, &written)
        }
        guard status == 0, written == expected else {
            throw DecodeError("inflate status \(status) wrote \(written), expected \(expected)")
        }
        return Array(UnsafeBufferPointer(start: dst, count: written))
    }

    private static func unfilter(_ src: [UInt8], width: Int, height: Int) throws -> [UInt8] {
        let stride = width * 4
        var out = [UInt8](repeating: 0, count: stride * height)
        var srcIndex = 0
        for y in 0..<height {
            guard srcIndex < src.count else { throw DecodeError("short idat") }
            let filter = src[srcIndex]
            srcIndex += 1
            let row = y * stride
            for x in 0..<stride {
                let raw = src[srcIndex]
                srcIndex += 1
                let a = x >= 4 ? out[row + x - 4] : 0
                let b = y > 0 ? out[row - stride + x] : 0
                let c = (y > 0 && x >= 4) ? out[row - stride + x - 4] : 0
                let value: UInt8
                switch filter {
                case 0:
                    value = raw
                case 1:
                    value = raw &+ a
                case 2:
                    value = raw &+ b
                case 3:
                    value = raw &+ UInt8((Int(a) + Int(b)) / 2)
                case 4:
                    value = raw &+ paeth(a, b, c)
                default:
                    throw DecodeError("filter \(filter)")
                }
                out[row + x] = value
            }
        }
        return out
    }

    private static func paeth(_ a: UInt8, _ b: UInt8, _ c: UInt8) -> UInt8 {
        let ia = Int(a)
        let ib = Int(b)
        let ic = Int(c)
        let p = ia + ib - ic
        let pa = abs(p - ia)
        let pb = abs(p - ib)
        let pc = abs(p - ic)
        if pa <= pb && pa <= pc { return a }
        if pb <= pc { return b }
        return c
    }

    private static func readU32(_ bytes: [UInt8], _ offset: Int) -> UInt32 {
        (UInt32(bytes[offset]) << 24)
            | (UInt32(bytes[offset + 1]) << 16)
            | (UInt32(bytes[offset + 2]) << 8)
            | UInt32(bytes[offset + 3])
    }

    struct DecodeError: Error, CustomStringConvertible {
        let description: String
        init(_ description: String) { self.description = description }
    }
}

enum RepoPaths {
    static var root: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    static var goldenPNG: URL {
        root.appendingPathComponent("crates/llamp-skin/tests/fixtures/golden/main-275x116.png")
    }
}

func pixel(_ rgba: [UInt8], width: Int, x: Int, y: Int) -> [UInt8] {
    let i = (y * width + x) * 4
    return Array(rgba[i..<(i + 4)])
}
