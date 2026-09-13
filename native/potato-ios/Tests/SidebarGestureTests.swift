import XCTest
@testable import PotatoMobile

final class SidebarGestureTests: XCTestCase {
    func testHorizontalOpeningTracksFingerAndCommitsPastThreshold() {
        var drag = SidebarDrag(open: false, width: 300, allowed: true)
        drag.update(CGSize(width: 50, height: 6))
        XCTAssertTrue(drag.active); XCTAssertEqual(drag.offset, 50)
        XCTAssertFalse(drag.destination(predicted: 50))
        drag.update(CGSize(width: 140, height: 8))
        XCTAssertEqual(drag.offset, 140); XCTAssertTrue(drag.destination(predicted: 140))
    }
    func testClosingTracksFingerAndShortDragReturnsOpen() {
        var drag = SidebarDrag(open: true, width: 300, allowed: true)
        drag.update(CGSize(width: -40, height: 2))
        XCTAssertEqual(drag.offset, 260); XCTAssertTrue(drag.destination(predicted: -40))
        drag.update(CGSize(width: -150, height: 2))
        XCTAssertEqual(drag.offset, 150); XCTAssertFalse(drag.destination(predicted: -150))
    }
    func testVerticalOrAmbiguousStartCannotTurnIntoDrawerGesture() {
        for start in [CGSize(width: 2, height: 12), CGSize(width: 11, height: 10)] {
            var drag = SidebarDrag(open: false, width: 300, allowed: true)
            drag.update(start); drag.update(CGSize(width: 200, height: 20))
            XCTAssertFalse(drag.active); XCTAssertFalse(drag.destination(predicted: 250))
        }
    }
    func testWrongInitialDirectionAndExcludedRegionsStayRejected() {
        var wrong = SidebarDrag(open: false, width: 300, allowed: true)
        wrong.update(CGSize(width: -20, height: 0)); wrong.update(CGSize(width: 200, height: 0))
        XCTAssertFalse(wrong.active)
        var excluded = SidebarDrag(open: true, width: 300, allowed: false)
        excluded.update(CGSize(width: -300, height: 0))
        XCTAssertFalse(excluded.active); XCTAssertTrue(excluded.destination(predicted: -300))
    }
    func testVelocityCanFinishDeliberateFlickButIsBounded() {
        var tiny = SidebarDrag(open: false, width: 300, allowed: true)
        tiny.update(CGSize(width: 10, height: 0))
        XCTAssertFalse(tiny.destination(predicted: 10_000))
        var flick = SidebarDrag(open: false, width: 300, allowed: true)
        flick.update(CGSize(width: 70, height: 0))
        XCTAssertTrue(flick.destination(predicted: 200))
    }
    func testReverseAndOvershootRemainInsideDrawerBounds() {
        var drag = SidebarDrag(open: false, width: 300, allowed: true)
        drag.update(CGSize(width: 1000, height: 0)); XCTAssertEqual(drag.offset, 300)
        drag.update(CGSize(width: -100, height: 0)); XCTAssertEqual(drag.offset, 0)
        XCTAssertFalse(drag.destination(predicted: -100))
        let interrupted = SidebarDrag(open: true, width: 300, allowed: true)
        XCTAssertTrue(interrupted.destination(predicted: 0))
    }
}
