import XCTest

final class RemoteModelUITests: XCTestCase {
    override func setUpWithError() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else {
            throw XCTSkip("Requires scripts/remote-draft-fixture.py --models on loopback; enable POTATO_IOS_DRAFT_UI")
        }
    }
    private func launch(reset: Bool = true, extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--remote-preview", "--remote-draft-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"] + extra
        if reset { app.launchArguments.append("--reset") }
        app.launch(); return app
    }
    private func open(_ app: XCUIApplication) -> XCUIElement {
        let device = app.buttons["remote-device-00000000-0000-4000-8000-000000000001"]
        XCTAssertTrue(device.waitForExistence(timeout: 8)); device.tap()
        app.buttons["remote-new-task"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5)); return input
    }
    private func picker(_ app: XCUIApplication) {
        app.buttons["remote-model-settings"].tap()
        XCTAssertTrue(app.buttons["remote-model-one"].waitForExistence(timeout: 5))
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let value = XCTAttachment(screenshot: app.screenshot()); value.name = name; value.lifetime = .keepAlways; add(value)
    }
    @discardableResult private func reveal(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        if id.hasPrefix("remote-effort-") {
            let thinking = app.buttons["remote-thinking-open"]
            if thinking.exists {
                for _ in 0..<8 { if thinking.isHittable { break }; app.swipeUp() }
                thinking.tap()
            }
        }
        let value = app.buttons[id]
        for _ in 0..<8 { if value.exists && value.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(value.isHittable); return value
    }
    private func reply(_ app: XCUIApplication, containing value: String) -> XCUIElement {
        app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", value)).firstMatch
    }
    func testChoiceAndEffortPersistThenReachSendPayload() {
        var app = launch(); let initialInput = open(app)
        XCTAssertTrue(app.staticTexts["让电脑帮你做点什么"].exists)
        XCTAssertLessThan(initialInput.frame.maxY, app.buttons["remote-model-settings"].frame.minY)
        capture(app, "remote-new-conversation-composer")
        picker(app)
        app.buttons["remote-model-one"].tap(); reveal(app, "remote-effort-low").tap()
        XCTAssertEqual(app.buttons["remote-effort-low"].value as? String, "已选择")
        capture(app, "remote-model-low-selected"); app.buttons["remote-model-done"].tap()
        app.terminate(); app = launch(reset: false)
        let input = open(app); picker(app)
        reveal(app, "remote-effort-low")
        XCTAssertEqual(app.buttons["remote-effort-low"].value as? String, "已选择")
        app.buttons["remote-model-done"].tap()
        input.tap(); input.typeText("fixture-model"); app.buttons["remote-send"].tap()
        XCTAssertTrue(reply(app, containing: "\"reasoning_effort\": \"low\"").waitForExistence(timeout: 10))
        capture(app, "remote-model-selected-wire-echo")
    }
    func testUnknownCapabilitiesOfferOnlyServiceDefault() {
        let app = launch(); let input = open(app); picker(app)
        app.buttons["remote-model-unknown"].tap()
        reveal(app, "remote-effort-default")
        XCTAssertTrue(app.buttons["remote-effort-default"].exists)
        XCTAssertFalse(app.buttons["remote-effort-high"].exists); XCTAssertFalse(app.buttons["remote-effort-low"].exists)
        XCTAssertEqual(app.buttons["remote-effort-default"].value as? String, "已选择")
        capture(app, "remote-model-unknown-capabilities")
        app.buttons["remote-model-done"].tap()
        input.tap(); input.typeText("fixture-model"); app.buttons["remote-send"].tap()
        XCTAssertTrue(reply(app, containing: "\"reasoning_effort\": null").waitForExistence(timeout: 10))
    }
    func testLostReceiptRetryKeepsChoiceAndOperationAcrossRelaunch() {
        var app = launch(); let input = open(app); picker(app)
        app.buttons["remote-model-one"].tap(); reveal(app, "remote-effort-low").tap(); app.buttons["remote-model-done"].tap()
        input.tap(); input.typeText("fixture-model-timeout"); app.buttons["remote-send"].tap()
        XCTAssertTrue(app.buttons["remote-retry-pending"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.buttons["remote-model-settings"].isEnabled)
        capture(app, "remote-model-pending-immutable")
        app.terminate(); app = launch(reset: false); _ = open(app)
        XCTAssertTrue(app.buttons["remote-retry-pending"].waitForExistence(timeout: 5))
        app.buttons["remote-retry-pending"].tap()
        XCTAssertTrue(reply(app, containing: "\"reasoning_effort\": \"low\"").waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["remote-retry-pending"].exists)
    }
    func testRunningFollowupKeepsTaskConfigurationAndRunIdentity() {
        let app = launch(); let input = open(app)
        input.tap(); input.typeText("fixture-running"); app.buttons["remote-send"].tap()
        let button = app.buttons["remote-model-settings"]
        let label = NSPredicate(format: "label CONTAINS %@", "沿用当前任务配置")
        expectation(for: label, evaluatedWith: button); waitForExpectations(timeout: 10)
        XCTAssertFalse(button.isEnabled); capture(app, "remote-running-model-locked")
        input.tap(); input.typeText("fixture-followup"); app.buttons["remote-send"].tap()
        XCTAssertTrue(reply(app, containing: "DRAFT_FIXTURE_OK: fixture-followup").waitForExistence(timeout: 10))
    }
    func testLargeTextCanScrollToEffortSelectAndDismiss() {
        let app = launch(extra: ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"])
        _ = open(app); app.buttons["remote-model-settings"].tap()
        let one = app.buttons["remote-model-one"]
        for _ in 0..<8 { if one.exists && one.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(one.isHittable); one.tap()
        reveal(app, "remote-effort-high")
        let high = app.buttons["remote-effort-high"]
        for _ in 0..<8 { if high.exists && high.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(high.isHittable); high.tap()
        XCTAssertEqual(high.value as? String, "已选择")
        capture(app, "remote-model-large-text-effort")
        XCTAssertTrue(app.buttons["remote-model-done"].isHittable); app.buttons["remote-model-done"].tap()
        XCTAssertTrue(app.buttons["remote-model-settings"].isHittable)
        capture(app, "remote-model-large-text-composer")
        let input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
        input.tap(); input.typeText("fixture-model")
        XCTAssertEqual(input.value as? String, "fixture-model")
        XCTAssertTrue(app.buttons["remote-send"].isHittable)
        capture(app, "remote-model-large-text-keyboard")
        app.buttons["remote-send"].tap()
        XCTAssertTrue(reply(app, containing: "\"reasoning_effort\": \"high\"").waitForExistence(timeout: 10))
    }
}
