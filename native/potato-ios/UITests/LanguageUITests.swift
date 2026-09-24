import XCTest

final class LanguageUITests: XCTestCase {
    func testLanguagePreviewSaveCancelRelaunchAndSystem() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        func openSettings() {
            XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10))
            app.buttons["connection-settings"].tap()
            XCTAssertTrue(app.buttons["language-picker"].waitForExistence(timeout: 4))
        }
        func choose(_ title: String) {
            app.buttons["language-picker"].tap()
            let option = app.buttons.matching(NSPredicate(format: "label == %@", title)).firstMatch
            XCTAssertTrue(option.waitForExistence(timeout: 3)); option.tap()
        }
        XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10))
        app.buttons["new-chat"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Keep my unsent draft")
        openSettings()
        choose("English")
        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 4))
        XCTAssertTrue(app.staticTexts["General"].exists)
        capture(app, "language-english")
        app.buttons["save-settings"].tap()
        XCTAssertTrue(app.buttons["connection-settings"].label.hasPrefix("Model and reasoning"))
        XCTAssertEqual(input.value as? String, "Keep my unsent draft")
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" }; app.launch()
        openSettings()
        XCTAssertTrue(app.navigationBars["Settings"].exists)
        choose("简体中文")
        XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 4))
        capture(app, "language-chinese")
        // Language applies immediately; closing keeps it and the draft.
        app.buttons["save-settings"].tap()
        XCTAssertTrue(app.buttons["connection-settings"].label.hasPrefix("模型与思考"))
        XCTAssertEqual(input.value as? String, "Keep my unsent draft")
        openSettings(); choose("跟随系统")
        XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 4))
        capture(app, "language-follow-system")
        app.buttons["save-settings"].tap()
        app.terminate()
        app.launchArguments = ["--ui-testing", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch(); openSettings()
        XCTAssertTrue(app.navigationBars["Settings"].exists)
        choose("简体中文"); app.buttons["save-settings"].tap()
        app.terminate(); app.launch(); openSettings()
        XCTAssertTrue(app.navigationBars["设置"].exists)
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let image = XCTAttachment(screenshot: app.screenshot()); image.name = name; image.lifetime = .keepAlways; add(image)
    }
}
