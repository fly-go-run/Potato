import XCTest
final class ActivityPresentationUITests: XCTestCase {
    private func capture(_ app: XCUIApplication, _ name: String) { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = name; shot.lifetime = .keepAlways; add(shot) }
    func testPresentationFailurePreviewAndDelivery() {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--activity-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]; app.launch()
        XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 10))
        let thought = app.buttons["activity-summary-reasoning"]
        XCTAssertTrue(thought.exists); XCTAssertTrue(thought.label.contains("先整理推理优化"))
        thought.tap(); XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 5))
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        capture(app, "ppt-01-chat")
        app.buttons["activity-summary"].tap()
        XCTAssertTrue(app.buttons["activity-step-code:draft"].waitForExistence(timeout: 5)); capture(app, "ppt-02-timeline")
        app.buttons["activity-step-code:draft"].tap()
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "unsupported shape type")).firstMatch.waitForExistence(timeout: 5)); capture(app, "ppt-03-failed-input-output")
        app.buttons["activity-back"].tap(); app.buttons["activity-step-code:preview"].tap()
        let image = app.buttons["activity-image"]
        XCTAssertTrue(image.waitForExistence(timeout: 5)); capture(app, "ppt-04-medium-image-output")
        XCTAssertFalse(app.buttons["activity-done"].exists)
        // The larger detent still allows inspecting the entire image and execution log.
        app.swipeUp(); capture(app, "ppt-04-image-output")
        app.buttons["原始输入代码"].tap()
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "render_slide(")).firstMatch.waitForExistence(timeout: 5))
        app.buttons["原始输入代码"].tap()
        app.buttons["执行输出"].tap()
        XCTAssertTrue(app.staticTexts["已渲染第 1 页"].waitForExistence(timeout: 5))
        app.buttons["执行输出"].tap()
        image.tap(); XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 5)); capture(app, "ppt-05-full-image")
        app.buttons["close-preview"].tap(); app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        let file = app.buttons["deliverable-card"]; XCTAssertTrue(file.waitForExistence(timeout: 5)); file.tap()
        XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 8))
        XCTAssertTrue(app.activityIndicators.firstMatch.waitForNonExistence(timeout: 20))
        RunLoop.current.run(until: Date().addingTimeInterval(2))
        capture(app, "ppt-06-file-preview")
    }
    func testLargeTextSheetCanNavigateAndDismiss() {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--activity-preview", "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL", "-AppleLanguages", "(zh-Hans)"]; app.launch()
        XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 10)); app.buttons["activity-summary"].tap()
        XCTAssertTrue(app.buttons["activity-step-reasoning"].waitForExistence(timeout: 5)); app.swipeUp(); capture(app, "ppt-large-type-timeline")
        app.buttons["activity-step-reasoning"].tap(); XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 5))
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap(); XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 5))
    }
}
