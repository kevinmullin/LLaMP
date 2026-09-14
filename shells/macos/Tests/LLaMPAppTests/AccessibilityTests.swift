import AppKit
import llamp_ffi
import XCTest
@testable import LLaMPApp

@MainActor
final class AccessibilityTests: XCTestCase {
    func testHitRegionsAreAccessibilityElementsWithRolesAndActions() throws {
        let view = try chrome()
        let children = view.accessibilityChildren() ?? []
        XCTAssertFalse(children.isEmpty)
        for child in children {
            let element = try XCTUnwrap(child as? NSAccessibilityElement)
            let label = try XCTUnwrap(element.accessibilityLabel())
            XCTAssertFalse(label.isEmpty)
            XCTAssertNotNil(element.accessibilityRole())
            let frame = element.accessibilityFrameInParentSpace()
            XCTAssertGreaterThan(frame.width, 0)
            XCTAssertGreaterThan(frame.height, 0)
            if label == "Play" || label == "Pause" || label == "Seek" {
                XCTAssertFalse(element.accessibilityActionNames().isEmpty, label)
            }
        }
        let labels = children.compactMap { ($0 as? NSAccessibilityElement)?.accessibilityLabel() }
        XCTAssertTrue(labels.contains("Play"))
        XCTAssertTrue(labels.contains("Pause"))
        XCTAssertTrue(labels.contains("Seek"))
        XCTAssertTrue(labels.contains("Volume"))
    }

    func testSpaceTogglesPlayAndArrowsSeek() throws {
        let view = try chrome()
        llamp_session_configure(44_100, 44_100 * 10, 2, "tone")
        view.keyDown(with: try XCTUnwrap(key(" ", code: 49)))
        XCTAssertEqual(llamp_playback_poll().transport, 1)
        view.keyDown(with: try XCTUnwrap(key(" ", code: 49)))
        XCTAssertEqual(llamp_playback_poll().transport, 2)
        view.keyDown(with: try XCTUnwrap(key(nil, code: 124)))
        XCTAssertEqual(llamp_playback_poll().position_frames, 44_100)
        view.keyDown(with: try XCTUnwrap(key(nil, code: 123)))
        XCTAssertEqual(llamp_playback_poll().position_frames, 0)
    }

    func testShadeIs14SkinPixelsAndAlwaysOnTopStartsOff() throws {
        let window = MainWindow()
        window.loadSkin(try Fixture.wsz())
        XCTAssertEqual(window.level, .normal)
        window.toggleShade()
        XCTAssertEqual(window.skinHeight, 14)
        XCTAssertEqual(window.contentView?.frame.height, 14)
    }

    private func chrome() throws -> ChromeView {
        let view = ChromeView(frame: NSRect(x: 0, y: 0, width: 275, height: 116))
        view.loadSkin(try Fixture.wsz())
        return view
    }

    private func key(_ characters: String?, code: UInt16) -> NSEvent? {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: characters ?? "",
            charactersIgnoringModifiers: characters ?? "",
            isARepeat: false,
            keyCode: code
        )
    }
}
