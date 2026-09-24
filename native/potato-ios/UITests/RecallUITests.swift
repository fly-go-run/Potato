import XCTest
final class RecallUITests: XCTestCase {
    private func openMemory(_ app: XCUIApplication) {
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10)); app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["settings-memory"].waitForExistence(timeout: 5)); app.buttons["settings-memory"].tap()
    }
    func testRecallSettingsPersistAndClearlyExplainCloudSync() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        openMemory(app)
        let toggle = app.switches["参考历史对话"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 5)); XCTAssertEqual(toggle.value as? String, "0")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "同步到你的账号")).firstMatch.exists)
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap(); XCTAssertEqual(toggle.value as? String, "1")
        let capture = XCTAttachment(screenshot: app.screenshot()); capture.name = "recall-settings"; capture.lifetime = .keepAlways; add(capture)
        app.terminate()
        app.launchArguments.removeAll { $0 == "--reset" }; app.launch()
        openMemory(app)
        XCTAssertTrue(toggle.waitForExistence(timeout: 5)); XCTAssertEqual(toggle.value as? String, "1")
    }
    func testHistorySourceOpensOriginalMessage() {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--recall-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        let history = app.buttons["检索到的历史 · 1 条"]
        XCTAssertTrue(history.waitForExistence(timeout: 10)); history.tap()
        XCTAssertTrue(app.buttons["查看原对话"].waitForExistence(timeout: 5))
        app.buttons["查看原对话"].tap()
        let original = app.staticTexts["我最后选择 X100，并已下单。"]
        XCTAssertTrue(original.waitForExistence(timeout: 5)); XCTAssertTrue(original.isHittable)
        let capture = XCTAttachment(screenshot: app.screenshot()); capture.name = "recall-source-jump"; capture.lifetime = .keepAlways; add(capture)
    }

}
