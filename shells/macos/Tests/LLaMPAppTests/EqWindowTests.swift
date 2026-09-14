import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class EqWindowTests: XCTestCase {
    func testWindowIs275By116AndShadeIs14() {
        let window = EqWindow()
        XCTAssertEqual(window.frame.width, 275)
        XCTAssertEqual(window.frame.height, 116)
        window.toggleShade()
        XCTAssertEqual(window.frame.height, CGFloat(llamp_shade_height()))
        XCTAssertEqual(llamp_shade_height(), 14)
    }

    func testConstrainKeepsSkinSizeOffTheDock() {
        let window = EqWindow()
        let clipped = NSRect(x: 0, y: 0, width: 275, height: 40)
        let constrained = window.constrainFrameRect(clipped, to: NSScreen.main)
        XCTAssertEqual(constrained.width, 275)
        XCTAssertEqual(constrained.height, 116)
        if let vis = NSScreen.main?.visibleFrame {
            XCTAssertGreaterThanOrEqual(constrained.minY, vis.minY)
            XCTAssertLessThanOrEqual(constrained.maxY, vis.maxY)
        }
    }

    func testAutoLabelIsTheFilenamePresetAction() throws {
        let window = EqWindow()
        let children = window.chrome.accessibilityChildren() ?? []
        let labels = children.compactMap { ($0 as? NSAccessibilityElement)?.accessibilityLabel() }
        XCTAssertTrue(labels.contains("Auto-load preset for this track"), labels.joined(separator: ", "))
        XCTAssertFalse(labels.contains { $0.localizedCaseInsensitiveContains("preamp law") })
        let element = try XCTUnwrap(children.compactMap { $0 as? NSAccessibilityElement }.first {
            $0.accessibilityLabel() == "Auto-load preset for this track"
        })
        XCTAssertEqual(element.accessibilityHelp(), "Auto-load preset for this track")
    }

    func testBandSweepIsAudibleThroughSliders() {
        let window = EqWindow()
        window.chrome.reload()
        llamp_eq_press(1)
        for band in 0..<10 {
            for other in 0..<10 {
                window.chrome.dragBand(other, millidb: other == band ? 12_000 : 0)
            }
            XCTAssertGreaterThan(
                llamp_eq_center_db(UInt32(band)),
                6,
                "window slider sweep did not make band \(band) audible"
            )
        }
    }
}
