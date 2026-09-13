import XCTest
final class CodeExecutionUITests: XCTestCase {
    func testAutomaticExecutionAndFilesSurviveRelaunch() {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--code-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]; app.launch()
        XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 10)); app.buttons["send-message"].tap()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "code-execution-running").firstMatch.waitForExistence(timeout: 8))
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "code-execution-complete").firstMatch.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "report.md")).firstMatch.waitForExistence(timeout: 5))
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "automatic-code-result"; shot.lifetime = .keepAlways; add(shot)
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" }; app.launch()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "code-execution-complete").firstMatch.waitForExistence(timeout: 10))
        let file = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "report.md")).firstMatch
        XCTAssertTrue(file.exists); file.tap()
        XCTAssertTrue(app.buttons["完成"].waitForExistence(timeout: 8) || app.buttons["Done"].exists)
    }
    func testStopCancelsCodeExecution() {
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "--reset", "--code-preview", "--code-hold", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]; app.launch()
        XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 10)); app.buttons["send-message"].tap()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "code-execution-running").firstMatch.waitForExistence(timeout: 8))
        app.buttons["stop-generation"].tap()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "code-execution-stopped").firstMatch.waitForExistence(timeout: 8))
        XCTAssertFalse(app.descendants(matching: .any).matching(identifier: "code-execution-complete").firstMatch.exists)
    }
}
