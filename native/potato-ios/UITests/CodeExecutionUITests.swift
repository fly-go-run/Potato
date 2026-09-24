import XCTest
final class CodeExecutionUITests: XCTestCase {
    private func launch(hold: Bool = false) -> XCUIApplication {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--code-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if hold { app.launchArguments.append("--code-hold") }
        app.launch(); XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 10)); app.buttons["send-message"].tap(); return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = name; shot.lifetime = .keepAlways; add(shot)
    }
    func testAutomaticExecutionAndFilesSurviveRelaunch() {
        let app = launch()
        XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 8))
        app.buttons["activity-summary"].tap()
        let step = app.buttons["activity-step-code:python-fixture"]
        XCTAssertTrue(step.waitForExistence(timeout: 5)); step.tap()
        XCTAssertTrue(app.staticTexts["print(17 * 19)"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["323"].waitForExistence(timeout: 15), "Open details must receive the live result")
        capture(app, "activity-live-code-detail")
        app.buttons["activity-back"].tap(); XCTAssertTrue(step.waitForExistence(timeout: 5))
        capture(app, "activity-complete-summary")
        app.buttons["activity-close"].tap()
        XCTAssertTrue(app.buttons["deliverable-card"].waitForExistence(timeout: 5)); capture(app, "activity-chat-delivery")
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" }; app.launch()
        XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 10))
        app.buttons["activity-summary"].tap(); app.buttons["activity-step-code:python-fixture"].tap()
        XCTAssertTrue(app.staticTexts["323"].waitForExistence(timeout: 5)); app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        app.buttons["deliverable-card"].tap(); XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 8))
    }
    func testStopCancelsCodeExecution() {
        let app = launch(hold: true)
        XCTAssertTrue(app.buttons["activity-summary"].waitForExistence(timeout: 8))
        app.buttons["stop-generation"].tap()
        let expected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", "已停止"), object: app.buttons["activity-summary"])
        XCTAssertEqual(XCTWaiter.wait(for: [expected], timeout: 8), .completed)
        app.buttons["activity-summary"].tap(); app.buttons["activity-step-code:python-fixture"].tap()
        XCTAssertTrue(app.staticTexts["已停止"].waitForExistence(timeout: 5)); capture(app, "activity-stopped")
    }
}
