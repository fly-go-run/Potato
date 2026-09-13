import XCTest

// Opt-in: browser login is completed by the operator, then only synthetic text
// is sent. All workspace/defaults data uses the dedicated UI test namespace.
final class CloudLiveTests: XCTestCase {
    func testCloudSignInModelsReplyAndRelaunch() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_CLOUD"] == "1")
        let app = XCUIApplication()
        let reuse = ProcessInfo.processInfo.environment["POTATO_LIVE_CLOUD_REUSE"] == "1"
        app.launchArguments = ["--ui-testing", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if !reuse { app.launchArguments.append("--reset") }
        app.launch()
        if !reuse {
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10)); app.buttons["connection-settings"].tap()
        app.buttons["cloud-model-login"].tap()
        XCTAssertTrue(app.buttons["cloud-sign-in"].waitForExistence(timeout: 5)); app.buttons["cloud-sign-in"].tap()
        // Open URL hands off to Safari. Bring Potato back to poll while the
        // operator completes the same verification URL in an authenticated browser.
        sleep(3); app.activate()
        }
        let connected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS[c] %@", "deepseek"), object: app.buttons["connection-settings"])
        XCTAssertEqual(XCTWaiter.wait(for: [connected], timeout: 240), .completed, "Complete browser login, then let the cloud catalog load")
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["local-model-deepseek/deepseek-v4-pro"].waitForExistence(timeout: 15))
        capture(app, "cloud-live-models")
        app.buttons["local-model-done"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Reply with exactly: POTATO_CLOUD_OK")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 120))
        XCTAssertTrue(app.staticTexts["POTATO_CLOUD_OK"].waitForExistence(timeout: 10))
        capture(app, "cloud-live-reply")
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" }; app.launch()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10)); app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["local-model-deepseek/deepseek-v4-pro"].waitForExistence(timeout: 10))
        capture(app, "cloud-live-restored-models")
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let value = XCTAttachment(screenshot: app.screenshot()); value.name = name; value.lifetime = .keepAlways; add(value)
    }
}

extension CloudLiveTests {
    func testSub2apiThroughCloudAndLogout() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_CLOUD_REUSE"] == "1")
        let app = XCUIApplication(); app.launchArguments = ["--ui-testing", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        app.buttons["connection-settings"].tap(); app.buttons["local-more-models"].tap()
        let search = app.textFields["local-model-search"]; XCTAssertTrue(search.waitForExistence(timeout: 5)); search.tap(); search.typeText("gpt-5.6")
        app.buttons["local-model-sub2api/gpt-5.6"].tap(); capture(app, "cloud-live-sub2api-selected")
        app.buttons["local-model-done"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Reply with exactly: POTATO_SUB2API_OK")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 120))
        XCTAssertTrue(app.staticTexts["POTATO_SUB2API_OK"].waitForExistence(timeout: 10)); capture(app, "cloud-live-sub2api-reply")
        app.buttons["more"].tap()
        try XCTUnwrap(app.buttons.matching(NSPredicate(format: "label == %@", "设置")).allElementsBoundByIndex.first(where: { $0.isHittable })).tap()
        app.buttons["cloud-model-login"].tap()
        XCTAssertTrue(app.buttons["退出 Cloudflare"].waitForExistence(timeout: 5)); app.buttons["退出 Cloudflare"].tap()
        XCTAssertTrue(app.buttons["cloud-sign-in"].waitForExistence(timeout: 15)); capture(app, "cloud-live-logged-out")
    }
}
