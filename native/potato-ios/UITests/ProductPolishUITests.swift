import XCTest

final class ProductPolishUITests: XCTestCase {
    private func launch(_ extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"] + extra
        app.launch()
        return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let image = XCTAttachment(screenshot: app.screenshot()); image.name = name; image.lifetime = .keepAlways; add(image)
    }

    /// A fresh install shows no sample conversation and asks for sign-in instead of sending a fake reply.
    func testFirstRunAsksToSignInAndKeepsTheDraft() {
        let app = launch(["--first-run-preview"])
        XCTAssertTrue(app.buttons["welcome-sign-in"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["close-document"].exists)
        XCTAssertTrue(app.buttons["connection-settings"].label.contains("登录后使用"))
        capture(app, "first-run-welcome")
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("第一条消息")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["cloud-sign-in"].waitForExistence(timeout: 5))
        capture(app, "first-run-sign-in")
        app.navigationBars["Potato 账号"].buttons["完成"].tap()
        XCTAssertEqual(input.value as? String, "第一条消息")
        XCTAssertFalse(app.otherElements["reply-failure"].exists)
    }

    /// Conversation actions live in the “more” menu; settings live in the sidebar.
    func testConversationMenuRenames() {
        let app = launch()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10)); app.buttons["close-document"].tap()
        app.buttons["more"].tap()
        XCTAssertTrue(app.buttons["重命名"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.buttons["移到最近删除"].exists)
        app.buttons["重命名"].tap()
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 3))
        field.clearAndType("周末安排")
        app.alerts.buttons["保存"].tap()
        app.buttons["history"].tap()
        XCTAssertTrue(app.buttons["周末安排"].waitForExistence(timeout: 3))
        capture(app, "sidebar-renamed")
    }

    func testUpgradedSampleRetryRequestsSignInWithoutReplacingReply() {
        let app = launch()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10))
        app.terminate()
        // Keep the old sample on disk, but launch with production sign-in behavior.
        app.launchArguments.removeAll { $0 == "--reset" }
        app.launchArguments.append("--first-run-preview")
        app.launch()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10))
        app.buttons["close-document"].tap()
        app.buttons["retry-message"].tap()
        XCTAssertTrue(app.buttons["cloud-sign-in"].waitForExistence(timeout: 5))
        app.navigationBars["Potato 账号"].buttons["完成"].tap()
        XCTAssertTrue(app.buttons["retry-message"].exists)
        XCTAssertFalse(app.buttons["previous-reply"].exists)
        XCTAssertFalse(app.buttons["stop-generation"].exists)
    }

    func testEmptyFailureCanChangeModelAndKeepDraftAndFailedVersion() {
        let app = launch(["--local-model-preview", "--unavailable-model-preview"])
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 10))
        input.tap(); input.typeText("测试失效模型")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["failure-change-model"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.buttons["copy-reply"].exists)
        capture(app, "failed-reply-model-recovery")
        input.tap(); input.typeText("保留这个草稿")
        app.buttons["failure-change-model"].tap()
        XCTAssertTrue(app.buttons["local-model-quick"].waitForExistence(timeout: 5))
        app.buttons["local-model-quick"].tap()
        app.buttons["reply-regenerate-confirm"].tap()
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "MODEL=quick")).firstMatch.waitForExistence(timeout: 8))
        XCTAssertEqual(input.value as? String, "保留这个草稿")
        XCTAssertTrue(app.buttons["previous-reply"].exists)
        capture(app, "failed-reply-model-recovered")
        app.buttons["previous-reply"].tap()
        XCTAssertTrue(app.otherElements["reply-failure"].waitForExistence(timeout: 3))
    }
}

private extension XCUIElement {
    func clearAndType(_ text: String) {
        tap()
        if let value = value as? String, !value.isEmpty { typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: value.count)) }
        typeText(text)
    }
}
