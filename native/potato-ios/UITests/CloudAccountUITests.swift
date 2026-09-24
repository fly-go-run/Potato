import XCTest

final class CloudAccountUITests: XCTestCase {
    func testCloudLoginEntryAndDismissPreserveManualSettings() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10))
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["cloud-model-login"].waitForExistence(timeout: 5))
        let settings = XCTAttachment(screenshot: app.screenshot()); settings.name = "cloud-settings-entry"; settings.lifetime = .keepAlways; add(settings)
        app.buttons["cloud-model-login"].tap()
        XCTAssertTrue(app.buttons["cloud-sign-in"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.textFields["endpoint"].isHittable)
        let login = XCTAttachment(screenshot: app.screenshot()); login.name = "cloud-account-login"; login.lifetime = .keepAlways; add(login)
        app.navigationBars["Potato 账号"].buttons["完成"].tap()
        XCTAssertTrue(app.buttons["cloud-model-login"].waitForExistence(timeout: 5))
        app.buttons["save-settings"].tap()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 5))
    }
}
