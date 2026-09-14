import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class LyricsWindowTests: XCTestCase {
    func testChromeAndTextShareIntegerOrigins() {
        for scale in [Int32(1), 2, 4] {
            let origin = llamp_lyrics_text_origin(scale)
            XCTAssertEqual(origin.x % scale, 0)
            XCTAssertEqual(origin.y % scale, 0)
            let mark = llamp_lyrics_mark_rect(scale)
            XCTAssertEqual(mark.x % scale, 0)
            XCTAssertEqual(mark.w % scale, 0)
            XCTAssertEqual(mark.h % scale, 0)
        }
    }

    func testSixWindowsSnapAsOneGroup() {
        let main = MainWindow()
        let eq = EqWindow()
        let playlist = PlaylistWindow()
        let browser = BrowserWindow()
        let vis = VisWindow()
        let lyrics = LyricsWindow()
        let dock = WindowDock.shared
        dock.detachExtras()
        llamp_group_reset()
        dock.attach(main: main, equalizer: eq)
        dock.attach(playlist: playlist)
        dock.attach(browser: browser)
        dock.attach(visualizer: vis)
        dock.attach(lyrics: lyrics)

        let topLeft = NSPoint(x: main.frame.minX, y: main.frame.maxY)
        place(eq, skinX: 8, skinY: 126, topLeft: topLeft)
        place(playlist, skinX: 283, skinY: 8, topLeft: topLeft)
        place(browser, skinX: 291, skinY: 140, topLeft: topLeft)
        place(vis, skinX: 8, skinY: 256, topLeft: topLeft)
        place(lyrics, skinX: 291, skinY: 256, topLeft: topLeft)

        dock.applyGroupDrag(which: 1, dx: -8, dy: -10)
        dock.applyGroupDrag(which: 2, dx: -8, dy: -8)
        dock.applyGroupDrag(which: 3, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 4, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 5, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 0, dx: 3, dy: 4)

        XCTAssertEqual(skin(main, topLeft: topLeft).x, 3)
        XCTAssertEqual(skin(vis, topLeft: topLeft).x, 3)
        XCTAssertEqual(skin(lyrics, topLeft: topLeft).x, 278)
    }

    func testWindowSaveWritesSidecarWhenTheDirectoryIsWritable() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("llamp-lyrics-off-\(ProcessInfo.processInfo.processIdentifier)")
        let music = root.appendingPathComponent("music")
        try FileManager.default.createDirectory(at: music, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let track = music.appendingPathComponent("song.wav")
        try writeGrantedWav(track)
        let before = try Data(contentsOf: track)
        let db = root.appendingPathComponent("library.sqlite")
        XCTAssertEqual(db.path.withCString { llamp_library_open($0) }, LLAMP_OK)
        XCTAssertEqual(music.path.withCString { llamp_library_grant($0) }, 1)
        try "[00:01.00]Hi\n".write(to: music.appendingPathComponent("song.lrc"), atomically: true, encoding: .utf8)

        let window = LyricsWindow()
        window.load(path: track.path)
        window.stepOffset(4)
        XCTAssertEqual(llamp_lyrics_offset_ms(), 200)
        XCTAssertTrue(window.saveOffset())
        XCTAssertEqual(llamp_lyrics_sidecar_written(), 1)
        XCTAssertNil(window.chrome.sidecarWarning)
        let text = try String(contentsOf: music.appendingPathComponent("song.lrc"), encoding: .utf8)
        XCTAssertTrue(text.contains("[offset:+200]"), text)
        XCTAssertEqual(try Data(contentsOf: track), before)
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("song.wav").path))
    }

    func testWindowSaveShowsSidecarWasNotWrittenWhenTheDirectoryIsNotWritable() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("llamp-lyrics-ro-\(ProcessInfo.processInfo.processIdentifier)")
        let music = root.appendingPathComponent("music")
        try FileManager.default.createDirectory(at: music, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: music.path)
            try? FileManager.default.removeItem(at: root)
        }
        let track = music.appendingPathComponent("song.wav")
        try writeGrantedWav(track)
        let before = try Data(contentsOf: track)
        let db = root.appendingPathComponent("library.sqlite")
        XCTAssertEqual(db.path.withCString { llamp_library_open($0) }, LLAMP_OK)
        XCTAssertEqual(music.path.withCString { llamp_library_grant($0) }, 1)
        try "[00:01.00]Hi\n".write(to: music.appendingPathComponent("song.lrc"), atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o555], ofItemAtPath: music.path)

        let window = LyricsWindow()
        window.load(path: track.path)
        window.stepOffset(4)
        XCTAssertTrue(window.saveOffset())
        XCTAssertEqual(llamp_lyrics_sidecar_written(), 0)
        XCTAssertEqual(window.chrome.sidecarWarning, "The sidecar was not written.")
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: music.path)
        let text = try String(contentsOf: music.appendingPathComponent("song.lrc"), encoding: .utf8)
        XCTAssertFalse(text.contains("[offset:+200]"), text)
        XCTAssertEqual(try Data(contentsOf: track), before)
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("song.wav").path))
    }

    private func place(_ window: NSWindow, skinX: CGFloat, skinY: CGFloat, topLeft: NSPoint) {
        let height = window.frame.height
        window.setFrameOrigin(NSPoint(x: topLeft.x + skinX, y: topLeft.y - skinY - height))
    }

    private func skin(_ window: NSWindow, topLeft: NSPoint) -> (x: Int, y: Int) {
        (
            Int((window.frame.minX - topLeft.x).rounded()),
            Int((topLeft.y - window.frame.maxY).rounded())
        )
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
}
