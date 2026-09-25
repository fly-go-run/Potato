import XCTest

final class ReasoningUITests: XCTestCase {
    private func launch(_ mode: String = "complete", reset: Bool = true, reduceMotion: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reasoning-preview", "--reasoning-case=" + mode, "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if reduceMotion { app.launchArguments.append("--reduce-motion-preview") }
        app.launch(); XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 10)); return app
    }
    private func waitLabel(_ element: XCUIElement, _ part: String, timeout: TimeInterval = 20) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout))
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", part), object: element)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed)
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot()); attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    private func openReasoning(_ app: XCUIApplication) {
        app.buttons["activity-summary"].tap()
        XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 5))
    }
    func testLongStreamingReplyFollowsMeasuredLayoutThroughSplitCodeFences() {
        let app = launch("streaming"); app.buttons["send-message"].tap()
        let anchor = app.staticTexts["稳定正文锚点"]
        XCTAssertTrue(anchor.waitForExistence(timeout: 8))
        XCTAssertTrue(anchor.isHittable)
        let code = app.scrollViews["markdown-code-scroll"]
        XCTAssertTrue(code.waitForExistence(timeout: 8))
        capture(app, "streaming-split-code-fence")
        XCTAssertTrue(app.buttons["stop-generation"].waitForNonExistence(timeout: 20))
        let tail = app.staticTexts["回复末尾：中文与家庭👨‍👩‍👧‍👦完整保留。"]
        XCTAssertTrue(tail.waitForExistence(timeout: 3))
        XCTAssertTrue(tail.isHittable)
        XCTAssertTrue(app.buttons["copy-code"].exists)
        XCTAssertFalse(app.buttons["scroll-latest"].exists)
        capture(app, "streaming-complete-bottom")
        app.scrollViews["conversation"].swipeDown()
        XCTAssertTrue(app.buttons["scroll-latest"].waitForExistence(timeout: 3))
        app.buttons["scroll-latest"].tap()
        XCTAssertTrue(tail.isHittable)
    }
    func testWaitingThinkingReplyAndRelaunch() {
        let app = launch(); app.buttons["send-message"].tap()
        XCTAssertTrue(app.descendants(matching: .any)["generating"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.staticTexts["正在准备回复…"].exists)
        capture(app, "quiet-reply-waiting")
        // The dot stays at the tail while the thought row names the work.
        waitLabel(app.buttons["activity-summary"], "正在思考"); XCTAssertTrue(app.descendants(matching: .any)["generating"].exists)
        openReasoning(app); capture(app, "reasoning-active-detail")
        waitLabel(app.staticTexts["activity-reasoning"], "十分位的 8 大于 1")
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        XCTAssertTrue(app.buttons["stop-generation"].waitForNonExistence(timeout: 15)); capture(app, "reasoning-complete")
        app.terminate(); let restored = launch(reset: false)
        waitLabel(restored.buttons["activity-summary"], "思考过程"); openReasoning(restored)
        XCTAssertTrue(restored.staticTexts["activity-reasoning"].label.contains("十分位的 8 大于 1")); capture(restored, "reasoning-restored")
    }
    func testStoppingReasoningKeepsContentAndClockWithReducedMotion() {
        let app = launch("hold", reduceMotion: true); app.buttons["send-message"].tap()
        waitLabel(app.buttons["activity-summary"], "正在思考"); openReasoning(app); capture(app, "reasoning-reduced-motion-active")
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap(); app.buttons["stop-generation"].tap()
        XCTAssertTrue(app.staticTexts["已停止生成"].waitForExistence(timeout: 20)); waitLabel(app.buttons["activity-summary"], "思考过程"); openReasoning(app)
        XCTAssertTrue(app.staticTexts["已停止"].waitForExistence(timeout: 5)); capture(app, "reasoning-stopped")
        app.terminate(); let restored = launch("hold", reset: false, reduceMotion: true)
        XCTAssertTrue(restored.staticTexts["已停止生成"].waitForExistence(timeout: 10)); waitLabel(restored.buttons["activity-summary"], "思考过程"); openReasoning(restored)
        XCTAssertTrue(restored.staticTexts["activity-reasoning"].label.contains("先把小数位对齐"))
    }
    func testInterruptedReasoningIsRetainedWithoutRunningAnimation() {
        let app = launch("interrupted"); app.buttons["send-message"].tap()
        waitLabel(app.buttons["activity-summary"], "正在思考"); waitLabel(app.buttons["activity-summary"], "思考过程")
        XCTAssertFalse(app.buttons["stop-generation"].exists); openReasoning(app)
        XCTAssertTrue(app.staticTexts["未成功"].waitForExistence(timeout: 5)); capture(app, "reasoning-interrupted")
    }
    func testSearchPausesReasoningThenThinkingResumes() {
        let app = launch("search"); app.buttons["send-message"].tap()
        waitLabel(app.buttons["activity-summary"], "正在思考")
        waitLabel(app.buttons["activity-summary"], "搜索：", timeout: 15); app.buttons["activity-summary"].tap()
        waitLabel(app.buttons["activity-step-reasoning"], "已完成", timeout: 2)
        XCTAssertTrue(app.buttons["activity-step-search:fixture-search"].exists); capture(app, "reasoning-paused-for-search")
        waitLabel(app.buttons["activity-step-reasoning"], "进行中", timeout: 15); capture(app, "reasoning-resumed-after-search")
        app.buttons["activity-close"].tap()
        XCTAssertTrue(app.buttons["stop-generation"].waitForNonExistence(timeout: 20)); waitLabel(app.buttons["activity-summary"], "搜索网页")
    }
}
