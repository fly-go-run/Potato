import XCTest

final class ReasoningUITests: XCTestCase {
    private func launch(_ mode: String = "complete", reset: Bool = true, reduceMotion: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reasoning-preview", "--reasoning-case=" + mode, "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if reduceMotion { app.launchArguments.append("--reduce-motion-preview") }
        app.launch(); XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 10))
        return app
    }
    private func waitLabel(_ element: XCUIElement, _ part: String, timeout: TimeInterval = 20) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout))
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", part), object: element)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed)
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot()); attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    func testWaitingThinkingReplyAndRelaunch() {
        let app = launch(); app.buttons["send-message"].tap()
        XCTAssertTrue(app.staticTexts["正在准备回复…"].waitForExistence(timeout: 3))
        let reasoning = app.buttons["reasoning-toggle"]
        waitLabel(reasoning, "正在思考")
        XCTAssertFalse(app.staticTexts["正在准备回复…"].exists)
        reasoning.tap(); XCTAssertTrue(app.staticTexts["reasoning-content"].exists)
        XCTAssertTrue(app.buttons["scroll-latest"].exists, "Expanding the process must pause automatic scrolling.")
        capture(app, "reasoning-active-expanded")
        waitLabel(reasoning, "已思考")
        XCTAssertTrue(app.staticTexts["正在回复"].exists)
        capture(app, "reasoning-finished-body-streaming")
        XCTAssertTrue(app.buttons["stop-generation"].waitForNonExistence(timeout: 15))
        XCTAssertFalse(app.staticTexts["正在回复"].exists)
        let completedLabel = reasoning.label
        capture(app, "reasoning-complete")
        app.terminate()
        let restored = launch(reset: false)
        waitLabel(restored.buttons["reasoning-toggle"], "已思考")
        XCTAssertFalse(restored.staticTexts["reasoning-content"].exists)
        restored.buttons["reasoning-toggle"].tap()
        XCTAssertEqual(restored.buttons["reasoning-toggle"].label, completedLabel)
        XCTAssertTrue((restored.staticTexts["reasoning-content"].label).contains("十分位的 8 大于 1"))
        capture(restored, "reasoning-restored")
    }
    func testStoppingReasoningKeepsContentAndClockWithReducedMotion() {
        let app = launch("hold", reduceMotion: true); app.buttons["send-message"].tap()
        let reasoning = app.buttons["reasoning-toggle"]
        waitLabel(reasoning, "正在思考"); reasoning.tap()
        capture(app, "reasoning-reduced-motion-active")
        app.buttons["stop-generation"].tap()
        waitLabel(reasoning, "思考已停止")
        XCTAssertTrue(app.staticTexts["reasoning-content"].exists)
        XCTAssertFalse(app.buttons["stop-generation"].exists)
        let stoppedLabel = reasoning.label
        capture(app, "reasoning-stopped")
        app.terminate()
        let restored = launch("hold", reset: false, reduceMotion: true)
        restored.buttons["reasoning-toggle"].tap()
        XCTAssertEqual(restored.buttons["reasoning-toggle"].label, stoppedLabel)
    }
    func testInterruptedReasoningIsRetainedWithoutRunningAnimation() {
        let app = launch("interrupted"); app.buttons["send-message"].tap()
        let reasoning = app.buttons["reasoning-toggle"]
        waitLabel(reasoning, "正在思考")
        waitLabel(reasoning, "思考已中断")
        XCTAssertFalse(app.buttons["stop-generation"].exists)
        reasoning.tap(); XCTAssertTrue(app.staticTexts["reasoning-content"].exists)
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "连接中断")).firstMatch.exists)
        capture(app, "reasoning-interrupted")
    }
    func testSearchPausesReasoningThenThinkingResumes() {
        let app = launch("search"); app.buttons["send-message"].tap()
        let reasoning = app.buttons["reasoning-toggle"]
        waitLabel(reasoning, "正在思考")
        XCTAssertTrue(app.staticTexts["正在搜索网页"].waitForExistence(timeout: 15))
        waitLabel(reasoning, "已思考", timeout: 2)
        capture(app, "reasoning-paused-for-search")
        waitLabel(reasoning, "正在思考", timeout: 15)
        XCTAssertFalse(app.staticTexts["正在搜索网页"].exists)
        capture(app, "reasoning-resumed-after-search")
        waitLabel(reasoning, "已思考")
        XCTAssertTrue(app.buttons["stop-generation"].waitForNonExistence(timeout: 15))
        capture(app, "reasoning-search-complete")
    }
}
