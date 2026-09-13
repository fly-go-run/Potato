import XCTest

final class VoiceInteractionTests: XCTestCase {
    private let words = "先把项目方案整理好，下午再和团队讨论一下。"
    private func launch(large: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--voice-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if large { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"] }
        app.launch(); XCTAssertTrue(app.buttons["voice-input"].waitForExistence(timeout: 10)); return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let value = XCTAttachment(screenshot: app.screenshot()); value.name = name; value.lifetime = .keepAlways; add(value)
    }
    private func listen(_ app: XCUIApplication) {
        app.buttons["voice-input"].tap()
        let text = app.textViews["voice-transcript"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value CONTAINS %@", words), object: text)], timeout: 8), .completed)
        XCTAssertFalse(app.buttons["start-voice"].exists); XCTAssertFalse(app.buttons["use-transcript"].exists)
    }
    func testDirectSpeechSend() {
        let app = launch(); listen(app); capture(app, "40-inline-recording")
        XCTAssertTrue(app.buttons["send-voice"].isHittable); app.buttons["send-voice"].tap()
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: app.buttons["send-voice"])], timeout: 8), .completed)
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 12)); XCTAssertFalse(app.buttons["send-voice"].exists)
        XCTAssertTrue(app.staticTexts[words].exists); capture(app, "41-inline-sent")
    }
    func testCancelAndEditKeepOriginalDraft() {
        let app = launch(), input = app.textViews["composer-input"]
        input.tap(); input.typeText("Original: "); listen(app)
        app.buttons["cancel-voice"].tap(); XCTAssertEqual(input.value as? String, "Original: ")
        listen(app); app.textViews["voice-transcript"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue((input.value as? String ?? "").contains("Original: " + words))
        input.typeText(" revised"); XCTAssertTrue((input.value as? String ?? "").hasSuffix(" revised")); capture(app, "42-inline-edit")
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" || $0 == "--voice-preview" }; app.launch(); XCTAssertTrue(app.textViews["composer-input"].waitForExistence(timeout: 10))
        XCTAssertTrue((app.textViews["composer-input"].value as? String ?? "").hasSuffix(" revised"))
    }
    func testBackgroundPreservesUnsentSpeech() {
        let app = launch(); listen(app)
        XCUIDevice.shared.press(.home); app.activate()
        XCTAssertTrue(app.textViews["composer-input"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.textViews["composer-input"].value as? String, words)
        XCTAssertTrue(app.staticTexts["voice-notice"].exists); capture(app, "43-inline-interrupted")
    }
    func testLargeTextVoiceControlsRemainReachable() {
        let app = launch(large: true); listen(app)
        XCTAssertTrue(app.buttons["send-voice"].isHittable); XCTAssertFalse(app.buttons["edit-voice"].exists); XCTAssertTrue(app.buttons["cancel-voice"].isHittable)
        capture(app, "44-inline-large-text"); app.buttons["cancel-voice"].tap()
    }
    func testReferenceCompactVoice() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--voice-preview", "--grok-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["voice-input"].waitForExistence(timeout: 10)); app.buttons["voice-input"].tap()
        let result = app.textViews["voice-transcript"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", "这是一个测试，你可以看一下，这是我语音转文字"), object: result)], timeout: 8), .completed)
        XCTAssertTrue(app.buttons["send-voice"].isHittable); capture(app, "59-reference-compact-voice")
        app.buttons["cancel-voice"].tap()
    }
    func testVoiceExpansionKeepsFinalText() {
        let app = launch(), input = app.textViews["composer-input"]
        let draft = String(repeating: "这段已有内容需要保留。", count: 25)
        input.tap(); input.typeText(draft); listen(app)
        XCTAssertTrue(app.buttons["expand-voice-input"].waitForExistence(timeout: 3)); app.buttons["expand-voice-input"].tap()
        let editor = app.textViews["expanded-input"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5)); XCTAssertEqual(editor.value as? String, draft + words)
        editor.typeText("修改完成"); capture(app, "60-voice-expanded-edit")
        app.buttons["collapse-input"].tap()
        XCTAssertTrue(input.waitForExistence(timeout: 5)); XCTAssertEqual(input.value as? String, draft + words + "修改完成")
    }
    func testWaveformGestures() {
        let app = launch(), input = app.textViews["composer-input"]
        input.tap(); input.typeText("原来的草稿"); listen(app)
        let center = app.buttons["cancel-voice"].coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0)).withOffset(CGVector(dx: 180, dy: 22))
        center.press(forDuration: 0.1, thenDragTo: center.withOffset(CGVector(dx: -140, dy: 0)))
        XCTAssertTrue(input.waitForExistence(timeout: 5)); XCTAssertEqual(input.value as? String, "原来的草稿")
        listen(app)
        let sendGesture = app.buttons["cancel-voice"].coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0)).withOffset(CGVector(dx: 180, dy: 22))
        sendGesture.press(forDuration: 0.1, thenDragTo: sendGesture.withOffset(CGVector(dx: 0, dy: -120)))
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 12))
        XCTAssertFalse(app.buttons["send-voice"].exists)
        XCTAssertTrue(app.staticTexts["原来的草稿" + words].exists)
        capture(app, "61-gesture-sent")
    }

}
