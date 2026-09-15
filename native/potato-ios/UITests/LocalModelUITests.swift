import XCTest

final class LocalModelUITests: XCTestCase {
    private func launch(reset: Bool = true, large: Bool = false, curated: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--local-model-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if large { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"] }
        if curated { app.launchArguments.append("--curated-cloud-preview") }
        app.launch(); XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10)); return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let value = XCTAttachment(screenshot: app.screenshot()); value.name = name; value.lifetime = .keepAlways; add(value)
    }
    private func reveal(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        if id.hasPrefix("local-effort-") || id.hasPrefix("local-thinking-") {
            let thinking = app.buttons["local-thinking-open"]
            if thinking.exists {
                for _ in 0..<8 { if thinking.isHittable { break }; app.swipeUp() }
                thinking.tap()
            }
        }
        if id == "local-open-connection" {
            if app.buttons["local-model-back"].exists { app.buttons["local-model-back"].tap() }
            app.buttons["local-more-models"].tap()
        }
        let item = app.buttons[id]
        for _ in 0..<10 {
            if item.exists && item.isHittable { return item }
            app.swipeUp()
        }
        XCTAssertTrue(item.isHittable, "Cannot reach \(id)"); return item
    }
    private func openPicker(_ app: XCUIApplication) {
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["local-model-deep"].waitForExistence(timeout: 5))
    }
    private func dismissPicker(_ app: XCUIApplication) {
        if app.buttons["local-model-done"].exists { app.buttons["local-model-done"].tap(); return }
        let title = app.navigationBars.firstMatch
        title.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98)))
        let closed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: title)
        XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 3), .completed)
    }
    func testCachedPickerReopensWithoutLoadingAndDismissesByDragging() {
        let app = launch(curated: true)
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["local-model-sub2api/gpt-5.6"].waitForExistence(timeout: 5))
        dismissPicker(app)
        for _ in 0..<3 {
            app.buttons["connection-settings"].tap()
            XCTAssertTrue(app.buttons["local-model-sub2api/gpt-5.6"].waitForExistence(timeout: 5))
            XCTAssertFalse(app.buttons["local-model-done"].exists)
            XCTAssertFalse(app.otherElements["local-model-loading"].exists)
            capture(app, "cached-model-picker")
            dismissPicker(app)
        }
        XCTAssertEqual(input(app).value as? String, "保留云端草稿")
    }
    private func chooseDeep(_ app: XCUIApplication) {
        openPicker(app); reveal(app, "local-model-deep").tap(); reveal(app, "local-effort-high").tap()
        XCTAssertEqual(app.buttons["local-effort-high"].value as? String, "已选择")
    }
    private func input(_ app: XCUIApplication) -> XCUIElement { app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch }
    private func send(_ app: XCUIApplication, _ text: String = "check model") {
        input(app).tap(); input(app).typeText(text)
        XCTAssertTrue(app.buttons["send-message"].isHittable); app.buttons["send-message"].tap()
    }
    private func reply(_ app: XCUIApplication, _ text: String) {
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch.waitForExistence(timeout: 8))
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 5))
    }
    func testCuratedCloudModelsAndExactThinkingOptions() {
        let app = launch(curated: true)
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.buttons["local-model-deepseek/deepseek-flash"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["local-model-sub2api/gpt-5.6"].exists)
        XCTAssertFalse(app.buttons["local-model-deepseek/deepseek-v4.1-flash-expires-on-0910"].exists)
        XCTAssertFalse(app.buttons["local-model-deepseek/deepseek-v4-pro"].exists)
        capture(app, "curated-cloud-models")
        app.buttons["local-thinking-open"].tap()
        XCTAssertTrue(app.buttons["local-thinking-disabled"].exists)
        XCTAssertTrue(app.buttons["local-effort-low"].exists)
        XCTAssertTrue(app.buttons["local-effort-high"].exists)
        XCTAssertTrue(app.buttons["local-effort-max"].exists)
        XCTAssertFalse(app.buttons["local-effort-medium"].exists)
        XCTAssertFalse(app.buttons["local-thinking-enabled"].exists)
        capture(app, "curated-deepseek-thinking")
        app.buttons["local-model-back"].tap()
        app.buttons["local-model-sub2api/gpt-5.6"].tap()
        app.buttons["local-thinking-open"].tap()
        for effort in ["none", "low", "medium", "high", "xhigh", "max"] { XCTAssertTrue(app.buttons["local-effort-\(effort)"].exists) }
        XCTAssertFalse(app.buttons["local-thinking-disabled"].exists)
        capture(app, "curated-gpt-thinking")
        app.buttons["local-effort-none"].tap()
        app.buttons["local-model-back"].tap(); app.buttons["local-more-models"].tap()
        XCTAssertFalse(app.textFields["local-manual-model"].exists)
        dismissPicker(app)
        XCTAssertEqual(input(app).value as? String, "保留云端草稿")
        app.buttons["send-message"].tap(); reply(app, "MODEL=sub2api/gpt-5.6;THINKING=default;EFFORT=none")
    }
    func testSimpleHomeAndLayeredModelPicker() {
        let app = launch()
        XCTAssertTrue(app.staticTexts["今天，想聊什么？"].exists)
        XCTAssertFalse(app.buttons["把一个想法变成计划"].exists)
        capture(app, "simple-home")
        input(app).tap(); input(app).typeText("keep draft")
        capture(app, "simple-home-keyboard")
        openPicker(app); capture(app, "simple-model-sheet")
        XCTAssertFalse(app.textFields["local-model-search"].exists)
        XCTAssertFalse(app.buttons["local-effort-high"].exists)
        app.buttons["local-model-deep"].tap()
        reveal(app, "local-effort-high").tap(); capture(app, "simple-thinking-sheet")
        app.buttons["local-model-back"].tap()
        app.buttons["local-model-deep"].tap()
        reveal(app, "local-effort-high")
        XCTAssertEqual(app.buttons["local-effort-high"].value as? String, "已选择")
        app.buttons["local-model-back"].tap()
        app.buttons["local-more-models"].tap()
        XCTAssertTrue(app.textFields["local-model-search"].waitForExistence(timeout: 5))
        app.textFields["local-model-search"].tap(); app.textFields["local-model-search"].typeText("missing")
        XCTAssertTrue(app.staticTexts["没有找到模型"].exists)
        app.buttons["local-model-back"].tap()
        XCTAssertTrue(app.buttons["local-model-deep"].exists)
        dismissPicker(app)
        XCTAssertEqual(input(app).value as? String, "keep draft")
    }
    func testSelectionPersistsAndRegenerationKeepsOldVersion() {
        var app = launch(); chooseDeep(app); capture(app, "local-model-high-selected")
        dismissPicker(app); app.terminate(); app = launch(reset: false)
        openPicker(app); XCTAssertEqual(reveal(app, "local-effort-high").value as? String, "已选择")
        dismissPicker(app); send(app)
        reply(app, "MODEL=deep;THINKING=enabled;EFFORT=high"); capture(app, "local-model-high-request")
        openPicker(app); reveal(app, "local-model-quick").tap(); dismissPicker(app)
        app.buttons["retry-message"].tap(); reply(app, "MODEL=deep;THINKING=enabled;EFFORT=high")
        app.buttons["reply-more"].tap(); app.buttons["reply-change-model"].tap()
        XCTAssertTrue(app.buttons["local-model-quick"].waitForExistence(timeout: 5))
        app.buttons["local-model-quick"].tap(); app.buttons["reply-regenerate-confirm"].tap()
        reply(app, "MODEL=quick;THINKING=default;EFFORT=default")
        app.buttons["previous-reply"].tap(); reply(app, "MODEL=deep;THINKING=enabled;EFFORT=high")
        capture(app, "local-model-old-reply-version")
    }
    func testCompactReplyActionsAndCancelledModelChange() {
        let app = launch(); send(app); reply(app, "MODEL=quick")
        XCTAssertFalse(app.buttons["read-reply"].exists)
        app.buttons["copy-reply"].tap(); XCTAssertEqual(app.buttons["copy-reply"].label, "已复制回复")
        capture(app, "compact-reply-actions")
        app.buttons["reply-more"].tap()
        XCTAssertTrue(app.buttons["reply-change-model"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["read-reply"].exists); XCTAssertTrue(app.buttons["share-reply"].exists)
        capture(app, "compact-reply-more")
        app.buttons["select-reply-text"].tap()
        XCTAssertTrue(app.textViews["selectable-reply"].waitForExistence(timeout: 5))
        app.buttons["close-text-selection"].tap()
        input(app).tap(); input(app).typeText("keep this draft")
        app.buttons["reply-more"].tap(); app.buttons["reply-change-model"].tap()
        XCTAssertTrue(app.buttons["local-model-deep"].waitForExistence(timeout: 5))
        app.buttons["local-model-deep"].tap(); reveal(app, "local-effort-high").tap()
        app.buttons["local-model-back"].tap(); capture(app, "compact-reply-change-model")
        dismissPicker(app)
        XCTAssertEqual(input(app).value as? String, "keep this draft")
        XCTAssertFalse(app.buttons["previous-reply"].exists)
        openPicker(app); XCTAssertEqual(app.buttons["local-model-quick"].value as? String, "已选择")
        dismissPicker(app)
    }
    func testLargeTextReplyActionsAndModelRegeneration() {
        let app = launch(large: true); send(app); reply(app, "MODEL=quick")
        reveal(app, "reply-more").tap()
        XCTAssertTrue(app.buttons["reply-change-model"].waitForExistence(timeout: 5))
        capture(app, "compact-reply-more-large-text")
        reveal(app, "reply-change-model").tap()
        XCTAssertTrue(app.buttons["local-model-deep"].waitForExistence(timeout: 5))
        reveal(app, "local-model-deep").tap()
        reveal(app, "reply-regenerate-confirm")
        capture(app, "compact-reply-model-large-text")
        app.buttons["reply-regenerate-confirm"].tap()
        reply(app, "MODEL=deep")
    }
    func testDisableThinkingClearsEffort() {
        let app = launch(); chooseDeep(app)
        // The disabled option is above the selected effort row on compact screens.
        for _ in 0..<5 { if app.buttons["local-thinking-disabled"].isHittable { break }; app.swipeDown() }
        app.buttons["local-thinking-disabled"].tap(); capture(app, "local-model-disabled")
        dismissPicker(app); send(app)
        reply(app, "MODEL=deep;THINKING=disabled;EFFORT=default")
    }
    func testServiceChangeRetainsDraftUntilModelIsReselected() {
        let app = launch(); chooseDeep(app); dismissPicker(app)
        let draft = "keep this unsent draft"
        input(app).tap(); input(app).typeText(draft)
        openPicker(app); reveal(app, "local-open-connection").tap()
        let endpoint = app.textFields["endpoint"]
        XCTAssertTrue(endpoint.waitForExistence(timeout: 5)); endpoint.tap()
        let old = endpoint.value as? String ?? ""
        endpoint.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count) + "https://other-model-preview.invalid/v1/chat/completions")
        app.buttons["save-settings"].tap(); XCTAssertTrue(app.buttons["send-message"].waitForExistence(timeout: 5)); app.buttons["send-message"].tap()
        XCTAssertTrue(app.alerts["Potato"].waitForExistence(timeout: 5)); capture(app, "local-model-service-changed-draft")
        app.alerts["Potato"].buttons["知道了"].tap(); XCTAssertEqual(input(app).value as? String, draft)
        openPicker(app); XCTAssertTrue(app.staticTexts["local-model-service-changed"].exists)
        reveal(app, "local-model-quick").tap(); dismissPicker(app); app.buttons["send-message"].tap()
        reply(app, "MODEL=quick;THINKING=default;EFFORT=default")
    }
    func testLargeTextPickerAndKeyboardRemainUsable() {
        let app = launch(large: true); chooseDeep(app); capture(app, "local-model-accessibility-high")
        XCTAssertTrue(app.buttons["local-model-done"].isHittable); dismissPicker(app)
        input(app).tap(); input(app).typeText("large text model check"); capture(app, "local-model-accessibility-keyboard")
        XCTAssertTrue(app.buttons["send-message"].isHittable); XCTAssertTrue(app.buttons["connection-settings"].isHittable)
        app.buttons["send-message"].tap(); reply(app, "MODEL=deep;THINKING=enabled;EFFORT=high")
    }
}
