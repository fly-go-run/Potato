import XCTest

final class CodeHighlightingUITests: XCTestCase {
    private func check(_ language: String, dark: Bool = false) {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--syntax-preview", "--syntax-\(language)", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if dark { app.launchArguments.append("--syntax-dark") }
        app.launch()
        XCTAssertTrue(app.buttons["copy-code"].firstMatch.waitForExistence(timeout: 10))
        XCTAssertTrue(app.scrollViews["markdown-code-scroll"].firstMatch.exists)
        XCTAssertFalse(app.buttons["open-sandbox"].exists)
        XCTAssertFalse(app.buttons["run-sandbox"].exists)
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "syntax-\(language)-\(dark ? "dark" : "light")"; shot.lifetime = .keepAlways; add(shot)
        app.buttons["copy-code"].firstMatch.tap()
        let input = app.textViews["composer-input"]
        input.tap(); input.press(forDuration: 1.2)
        let paste = app.menuItems["粘贴"].exists ? app.menuItems["粘贴"] : app.menuItems["Paste"]
        if paste.waitForExistence(timeout: 3) {
            paste.tap()
            XCTAssertTrue((input.value as? String ?? "").contains(language == "python" ? "def top_words" : "# 项目说明"))
        } else { XCTFail("Copied code should be available to paste") }
    }
    func testPythonLightAndCopyWithoutExecution() { check("python") }
    func testPythonDarkAndCopyWithoutExecution() { check("python", dark: true) }
    func testMarkdownSourceAndCopy() { check("markdown") }
}
