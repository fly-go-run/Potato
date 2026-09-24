import XCTest

final class GlassChromeTests: XCTestCase {
    private func launch(extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--sidebar-long-chat-preview",
                               "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"] + extra
        app.launch()
        XCTAssertTrue(app.buttons["history"].waitForExistence(timeout: 10))
        return app
    }

    private func capture(_ app: XCUIApplication, _ name: String) {
        let image = XCTAttachment(screenshot: app.screenshot())
        image.name = name; image.lifetime = .keepAlways; add(image)
    }

    func testReadingSidebarAndLatestMessageRemainAccessible() {
        let app = launch(extra: ["--glass-reading-preview"])
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.35)).press(forDuration: 0.05,
            thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.7)))
        let latest = app.buttons["scroll-latest"]
        XCTAssertTrue(latest.waitForExistence(timeout: 3)); XCTAssertTrue(latest.isHittable)
        XCTAssertLessThan(latest.frame.maxY, app.textViews["composer-input"].frame.minY)
        capture(app, "glass-01-reading-under-controls")
        app.buttons["history"].tap()
        let close = app.buttons["close-sidebar"]
        XCTAssertTrue(close.waitForExistence(timeout: 3))
        XCTAssertLessThanOrEqual(close.frame.minY, app.frame.minY + 1)
        XCTAssertGreaterThanOrEqual(close.frame.maxY, app.frame.maxY - 1)
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "selected == true")).firstMatch.exists)
        capture(app, "glass-02-continuous-sidebar")
        close.tap()
        XCTAssertTrue(close.waitForNonExistence(timeout: 3))
        capture(app, "glass-08-after-closing-sidebar")
        latest.tap()
        XCTAssertTrue(latest.waitForNonExistence(timeout: 3))
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "不用把空出来的时间补满")).firstMatch.isHittable)
        let actions = app.buttons["retry-message"]
        XCTAssertTrue(actions.isHittable)
        XCTAssertLessThan(actions.frame.maxY, app.textViews["composer-input"].frame.minY)
        capture(app, "glass-03-latest-unobscured")
    }

    func testControlsRespondAfterClosingSidebar() {
        let app = launch()
        app.buttons["history"].tap()
        let close = app.buttons["close-sidebar"]
        XCTAssertTrue(close.waitForExistence(timeout: 3)); close.tap()
        XCTAssertTrue(close.waitForNonExistence(timeout: 3))
        app.buttons["new-chat"].tap()
        XCTAssertTrue(app.staticTexts["今天，想聊什么？"].waitForExistence(timeout: 3))
        let input = app.textViews["composer-input"]
        input.tap(); input.typeText("Still interactive")
        XCTAssertEqual(input.value as? String, "Still interactive")
    }

    func testKeyboardAndNewChatKeepControlsReachable() {
        let app = launch()
        app.buttons["new-chat"].tap()
        capture(app, "glass-04-welcome")
        let input = app.textViews["composer-input"]
        input.tap(); input.typeText("A draft above the keyboard")
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 3))
        let send = app.buttons["send-message"]
        XCTAssertTrue(send.isHittable)
        XCTAssertLessThanOrEqual(send.frame.maxY, app.keyboards.firstMatch.frame.minY)
        capture(app, "glass-05-keyboard")
        app.buttons["history"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 3))
        app.buttons["close-sidebar"].tap()
        XCTAssertEqual(input.value as? String, "A draft above the keyboard")
    }

    func testLargeTextKeepsComposerAndDrawerUsable() {
        let app = launch(extra: ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"])
        app.buttons["new-chat"].tap()
        XCTAssertTrue(app.buttons["connection-settings"].isHittable)
        XCTAssertTrue(app.buttons["voice-input"].isHittable)
        capture(app, "glass-06-large-text")
        app.buttons["history"].tap()
        XCTAssertTrue(app.buttons["sidebar-new-chat"].isHittable)
        XCTAssertLessThanOrEqual(app.buttons["sidebar-new-chat"].frame.height, 64)
        XCTAssertTrue(app.buttons["sidebar-settings"].isHittable)
        capture(app, "glass-07-large-text-sidebar")
    }

    func testReducedTransparencyKeepsControlsReadable() {
        let app = launch(extra: ["--reduce-transparency-preview", "--reduce-motion-preview"])
        app.swipeDown()
        XCTAssertTrue(app.buttons["history"].isHittable)
        XCTAssertTrue(app.buttons["voice-input"].isHittable)
        capture(app, "glass-09-reduced-transparency")
        app.buttons["history"].tap()
        XCTAssertTrue(app.buttons["close-sidebar"].waitForExistence(timeout: 3))
        app.buttons["close-sidebar"].tap()
        app.buttons["new-chat"].tap()
        XCTAssertTrue(app.staticTexts["今天，想聊什么？"].waitForExistence(timeout: 3))
    }
}
