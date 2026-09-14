import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class VisWindowTests: XCTestCase {
    func testChromeOwnsTheBorderPixels() {
        let hole = llamp_vis_client_rect(275, 116, 0)
        XCTAssertEqual(hole.x, 8)
        XCTAssertEqual(hole.y, 14)
        XCTAssertEqual(hole.w, 259)
        XCTAssertEqual(hole.h, 94)
        let full = llamp_vis_client_rect(800, 600, 1)
        XCTAssertEqual(full.x, 0)
        XCTAssertEqual(full.y, 0)
        XCTAssertEqual(full.w, 800)
        XCTAssertEqual(full.h, 600)
    }

    func testFiveWindowsSnapAsOneGroup() {
        let main = MainWindow()
        let eq = EqWindow()
        let playlist = PlaylistWindow()
        let browser = BrowserWindow()
        let vis = VisWindow()
        let dock = WindowDock.shared
        dock.detachExtras()
        llamp_group_reset()
        dock.attach(main: main, equalizer: eq)
        dock.attach(playlist: playlist)
        dock.attach(browser: browser)
        dock.attach(visualizer: vis)

        let topLeft = NSPoint(x: main.frame.minX, y: main.frame.maxY)
        place(eq, skinX: 8, skinY: 126, topLeft: topLeft)
        place(playlist, skinX: 283, skinY: 8, topLeft: topLeft)
        place(browser, skinX: 291, skinY: 140, topLeft: topLeft)
        place(vis, skinX: 8, skinY: 256, topLeft: topLeft)

        dock.applyGroupDrag(which: 1, dx: -8, dy: -10)
        dock.applyGroupDrag(which: 2, dx: -8, dy: -8)
        dock.applyGroupDrag(which: 3, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 4, dx: -8, dy: -24)
        dock.applyGroupDrag(which: 0, dx: 3, dy: 4)

        XCTAssertEqual(skin(main, topLeft: topLeft).x, 3)
        XCTAssertEqual(skin(vis, topLeft: topLeft).x, 3)
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
}
