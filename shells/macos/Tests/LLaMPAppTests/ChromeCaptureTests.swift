import AppKit
import XCTest
@testable import LLaMPApp

/// Compares a pinned-sRGB content-view capture to the phase 2 PNG.
/// A window-server screenshot is not this test.
@MainActor
final class ChromeCaptureTests: XCTestCase {
    func testContentViewMatchesPhase2PNGAt1x() throws {
        let golden = try loadGolden()
        let view = try contentView(backingScale: 1)
        let capture = BitmapCapture.render(view, pixelsWide: 275, pixelsHigh: 116)
        XCTAssertEqual(capture.image.pixelsWide, 275)
        XCTAssertEqual(capture.image.pixelsHigh, 116)
        XCTAssertEqual(capture.image.colorSpace, NSColorSpace.sRGB)
        XCTAssertNotEqual(capture.image.colorSpace, NSColorSpace.deviceRGB)
        if capture.rgba != golden.rgba {
            reportMismatch(capture.rgba, golden.rgba)
        }
        XCTAssertEqual(capture.rgba, golden.rgba)
    }

    func testBackingScale2IsNearestNeighborNotABlend() throws {
        let golden = try loadGolden()
        let view = try contentView(backingScale: 2)
        let capture = BitmapCapture.render(view, pixelsWide: 550, pixelsHigh: 232)
        XCTAssertEqual(view.layer?.magnificationFilter, .nearest)
        XCTAssertEqual(capture.image.colorSpace, NSColorSpace.sRGB)
        XCTAssertNotEqual(capture.image.colorSpace, NSColorSpace.deviceRGB)
        let top = pixel(golden.rgba, width: 275, x: 8, y: 13)
        let bottom = pixel(golden.rgba, width: 275, x: 8, y: 14)
        XCTAssertNotEqual(top, bottom)
        // Same edge the phase 2 nearest-neighbor test samples, in the backing store.
        XCTAssertEqual(pixel(capture.rgba, width: 550, x: 16, y: 26), top)
        XCTAssertEqual(pixel(capture.rgba, width: 550, x: 17, y: 27), top)
        XCTAssertEqual(pixel(capture.rgba, width: 550, x: 16, y: 28), bottom)
        XCTAssertEqual(pixel(capture.rgba, width: 550, x: 17, y: 29), bottom)
        let blend = zip(top, bottom).map { UInt8((Int($0) + Int($1)) / 2) }
        XCTAssertNotEqual(pixel(capture.rgba, width: 550, x: 16, y: 27), blend)
        XCTAssertNotEqual(pixel(capture.rgba, width: 550, x: 16, y: 28), blend)
    }

    private func loadGolden() throws -> GoldenPNG.Image {
        let data = try Data(contentsOf: RepoPaths.goldenPNG)
        let golden = try GoldenPNG.decode(data)
        XCTAssertTrue(golden.srgb)
        XCTAssertFalse(golden.iccp)
        XCTAssertFalse(golden.gama)
        XCTAssertFalse(golden.chrm)
        XCTAssertEqual(pixel(golden.rgba, width: 275, x: 8, y: 13), [0x4A, 0x4A, 0x4A, 255])
        XCTAssertEqual(pixel(golden.rgba, width: 275, x: 8, y: 14), [0x1A, 0x1A, 0x1A, 255])
        return golden
    }

    private func contentView(backingScale: Int) throws -> ChromeView {
        let view = ChromeView(frame: NSRect(x: 0, y: 0, width: 275, height: 116))
        view.loadSkin(try Fixture.wsz())
        view.setBackingScale(backingScale)
        return view
    }

    private func reportMismatch(_ got: [UInt8], _ want: [UInt8]) {
        let n = min(got.count, want.count)
        var shown = 0
        var i = 0
        while i + 3 < n && shown < 8 {
            if got[i] != want[i] || got[i + 1] != want[i + 1] || got[i + 2] != want[i + 2] || got[i + 3] != want[i + 3] {
                let pixel = i / 4
                XCTFail("pixel \(pixel % 275),\(pixel / 275) \(Array(got[i...i + 3])) != \(Array(want[i...i + 3]))")
                shown += 1
            }
            i += 4
        }
    }
}

enum Fixture {
    static func wsz() throws -> Data {
        let dest = FileManager.default.temporaryDirectory.appendingPathComponent("llamp-fixture.wsz")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [
            "cargo", "run", "--quiet", "--manifest-path",
            RepoPaths.root.appendingPathComponent("xtask/Cargo.toml").path,
            "--", "emit", dest.path,
        ]
        process.currentDirectoryURL = RepoPaths.root
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw FixtureError("xtask emit exited \(process.terminationStatus)")
        }
        return try Data(contentsOf: dest)
    }
}

struct FixtureError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}
