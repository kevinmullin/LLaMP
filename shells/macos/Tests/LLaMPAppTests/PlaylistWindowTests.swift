import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class PlaylistWindowTests: XCTestCase {
    override func setUp() {
        super.setUp()
        _ = llamp_playlist_propose_size(275, 116)
    }

    func testMidStepResizeIsRejected() {
        let window = PlaylistWindow()
        XCTAssertFalse(window.proposeSize(width: 310, height: 145))
        XCTAssertEqual(window.frame.width, 275)
        XCTAssertEqual(window.frame.height, 116)
        XCTAssertTrue(window.proposeSize(width: 300, height: 145))
        XCTAssertEqual(window.frame.width, 300)
        XCTAssertEqual(window.frame.height, 145)
    }

    func testTenThousandClickDoesNotWalkEveryRow() {
        let window = PlaylistWindow()
        window.list.entries = (0..<10_000).map { "row-\($0)" }
        window.list.scroll = 40
        XCTAssertEqual(window.list.hitRow(y: 7), 41)
        XCTAssertEqual(window.list.rowsConsidered, 1)
        _ = BitmapCapture.render(window.list, pixelsWide: 275, pixelsHigh: 116)
        XCTAssertLessThan(window.list.rowsConsidered, 100)
        XCTAssertGreaterThan(window.list.rowsConsidered, 0)
        let children = window.list.accessibilityChildren() ?? []
        XCTAssertEqual(children.count, window.list.rowsConsidered + 5)
        XCTAssertNotEqual(children.count, 10_000)
    }

    func testVisibleMissingGlyphPromotesTheList() throws {
        let wsz = try Fixture.wsz()
        let loaded = wsz.withUnsafeBytes { raw -> Int32 in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return LLAMP_ERR_INVALID }
            return llamp_skin_load(base, raw.count)
        }
        XCTAssertEqual(loaded, LLAMP_OK)
        XCTAssertEqual("FIXTURE".withCString { llamp_playlist_row_font($0) }, 0)
        XCTAssertEqual("F日".withCString { llamp_playlist_row_font($0) }, 1)
        XCTAssertEqual(llamp_playlist_char_font(UInt32(UnicodeScalar("F").value)), 0)
        XCTAssertEqual(llamp_playlist_char_font(0x65E5), 1)
        XCTAssertFalse(PlaylistListView.listFontIsCoreText(["FIXTURE"]))
        XCTAssertTrue(PlaylistListView.listFontIsCoreText(["FIXTURE", "F日"]))

        let bitmapOnly = PlaylistWindow()
        bitmapOnly.list.entries = ["FIXTURE"]
        bitmapOnly.list.textScale = 4
        let bitmapCapture = BitmapCapture.render(bitmapOnly.list, pixelsWide: 1100, pixelsHigh: 464)
        let bitmapF = cell(bitmapCapture.rgba, width: 1100, scale: 4, row: 0, col: 0)

        let window = PlaylistWindow()
        window.list.entries = ["FIXTURE", "F日"]
        window.list.textScale = 4
        let capture = BitmapCapture.render(window.list, pixelsWide: 1100, pixelsHigh: 464)
        let url = URL(fileURLWithPath: "/tmp/llamp-list-font-4x.png")
        let png = capture.image.representation(using: .png, properties: [:])
        try XCTUnwrap(png).write(to: url)

        let promoted = cell(capture.rgba, width: 1100, scale: 4, row: 0, col: 0)
        let cjk = cell(capture.rgba, width: 1100, scale: 4, row: 1, col: 0)
        XCTAssertNotEqual(promoted, bitmapF, "a visible missing glyph must not leave a bitmap row")
        XCTAssertFalse(flat(cjk, rgb(llamp_text_bg())), "CoreText must not be a blank box")
        XCTAssertNotEqual(cjk, bitmapF)
    }

    private func rgb(_ packed: UInt32) -> [UInt8] {
        [UInt8((packed >> 16) & 0xff), UInt8((packed >> 8) & 0xff), UInt8(packed & 0xff)]
    }

    private func cell(_ rgba: [UInt8], width: Int, scale: Int, row: Int, col: Int, originX: Int = 0) -> [UInt8] {
        let w = 5 * scale
        let h = 7 * scale
        var out = [UInt8](repeating: 0, count: w * h * 4)
        let x0 = originX * scale + col * w
        let y0 = row * h
        for y in 0..<h {
            for x in 0..<w {
                let from = ((y0 + y) * width + x0 + x) * 4
                let to = (y * w + x) * 4
                out[to..<(to + 4)] = rgba[from..<(from + 4)]
            }
        }
        return out
    }

    private func flat(_ rgba: [UInt8], _ color: [UInt8]) -> Bool {
        var i = 0
        while i + 3 < rgba.count {
            if rgba[i] != color[0] || rgba[i + 1] != color[1] || rgba[i + 2] != color[2] {
                return false
            }
            i += 4
        }
        return true
    }
}
