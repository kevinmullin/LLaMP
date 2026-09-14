import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class BrowserWindowTests: XCTestCase {
    override func tearDown() {
        MainActor.assumeIsolated {
            WindowDock.shared.detachExtras()
        }
        super.tearDown()
    }

    func testBrowserListsGrantedLibraryInCoreText() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("llamp-browser-\(ProcessInfo.processInfo.processIdentifier)")
        let music = root.appendingPathComponent("music")
        try FileManager.default.createDirectory(at: music, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let track = music.appendingPathComponent("song.wav")
        try writeGrantedWav(track)
        let db = root.appendingPathComponent("library.sqlite")
        XCTAssertEqual(db.path.withCString { llamp_library_open($0) }, LLAMP_OK)
        XCTAssertEqual(music.path.withCString { llamp_library_grant($0) }, 1)
        let window = BrowserWindow()
        window.reloadGranted()
        XCTAssertTrue(window.list.rows.contains { $0.hasSuffix("song.wav") }, window.list.rows.joined(separator: ","))
        XCTAssertTrue(window.list.rowFontIsCoreText(window.list.rows[0]))
        XCTAssertFalse(window.usesBitmapFont)
        XCTAssertEqual("FIXTURE".withCString { llamp_browser_row_font($0) }, 1)
    }

    func testBrowserIsNotTheBitmapFont() {
        let window = BrowserWindow()
        XCTAssertFalse(window.usesBitmapFont)
        XCTAssertEqual(window.frame.width, 275)
        XCTAssertEqual(window.frame.height, 116)
        XCTAssertTrue(window.list.rowFontIsCoreText("FIXTURE"))
        XCTAssertTrue(window.list.rowFontIsCoreText("周杰伦"))
        XCTAssertEqual("FIXTURE".withCString { llamp_browser_row_font($0) }, 1)
    }

    func testFourWindowsSnapAsOneGroup() {
        let main = MainWindow()
        let eq = EqWindow()
        let playlist = PlaylistWindow()
        let browser = BrowserWindow()
        let dock = WindowDock.shared
        dock.detachExtras()
        llamp_group_reset()
        dock.attach(main: main, equalizer: eq)
        dock.attach(playlist: playlist)
        dock.attach(browser: browser)

        let topLeft = NSPoint(x: main.frame.minX, y: main.frame.maxY)
        place(eq, skinX: 8, skinY: 126, topLeft: topLeft)
        place(playlist, skinX: 283, skinY: 8, topLeft: topLeft)
        place(browser, skinX: 291, skinY: 140, topLeft: topLeft)

        dock.applyGroupDrag(which: 1, dx: -8, dy: -10)
        dock.applyGroupDrag(which: 2, dx: -8, dy: -8)
        dock.applyGroupDrag(which: 3, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 0, dx: 3, dy: 4)

        XCTAssertEqual(skin(main, topLeft: topLeft).x, 3)
        XCTAssertEqual(skin(eq, topLeft: topLeft).x, 3)
        XCTAssertEqual(skin(playlist, topLeft: topLeft).x, 278)
        XCTAssertEqual(skin(browser, topLeft: topLeft).x, 278)
        XCTAssertEqual(skin(main, topLeft: topLeft).y, 4)
        XCTAssertEqual(skin(playlist, topLeft: topLeft).y, 4)
        XCTAssertEqual(skin(eq, topLeft: topLeft).y, 120)
        XCTAssertEqual(skin(browser, topLeft: topLeft).y, 120)
    }

    private func place(_ window: NSWindow, skinX: CGFloat, skinY: CGFloat, topLeft: NSPoint) {
        let height = window.frame.height
        window.setFrameOrigin(NSPoint(x: topLeft.x + skinX, y: topLeft.y - skinY - height))
    }

    private func writeGrantedWav(_ url: URL) throws {
        var bytes: [UInt8] = []
        bytes += Array("RIFF".utf8)
        bytes += [52, 0, 0, 0]
        bytes += Array("WAVEfmt ".utf8)
        bytes += [16, 0, 0, 0, 1, 0, 2, 0]
        bytes += [64, 31, 0, 0, 0, 125, 0, 0, 4, 0, 16, 0]
        bytes += Array("data".utf8)
        bytes += [16, 0, 0, 0]
        bytes += Array(repeating: 0, count: 16)
        try Data(bytes).write(to: url)
    }

    private func skin(_ window: NSWindow, topLeft: NSPoint) -> (x: Int, y: Int) {
        (
            Int((window.frame.minX - topLeft.x).rounded()),
            Int((topLeft.y - window.frame.maxY).rounded())
        )
    }
}
