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

    private func remoteVoice(large: Bool = false, long: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--remote-preview", "--remote-draft-preview", "--voice-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if large { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"] }
        if long { app.launchArguments.append("--voice-long-preview") }
        app.launch()
        let device = app.buttons["remote-device-00000000-0000-4000-8000-000000000001"]
        XCTAssertTrue(device.waitForExistence(timeout: 10)); device.tap()
        app.buttons["remote-new-task"].tap()
        XCTAssertTrue(app.textViews["remote-prompt"].waitForExistence(timeout: 5))
        return app
    }
    private func listenRemote(_ app: XCUIApplication) {
        app.buttons["remote-voice-input"].tap()
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value CONTAINS %@", words), object: app.textViews["voice-transcript"])], timeout: 8), .completed)
        XCTAssertFalse(app.buttons["remote-model-settings"].exists)
        XCTAssertFalse(app.staticTexts["让电脑帮你做点什么"].exists)
    }
    func testRemoteVoiceCancelAndEdit() {
        let app = remoteVoice(), input = app.textViews["remote-prompt"]
        input.tap(); input.typeText("Original: ")
        listenRemote(app); capture(app, "remote-voice-shared-panel")
        app.buttons["cancel-voice"].tap()
        XCTAssertEqual(input.value as? String, "Original: ")
        listenRemote(app); app.textViews["voice-transcript"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5))
        XCTAssertEqual(input.value as? String, "Original: " + words)
        input.typeText(" revised")
        XCTAssertTrue((input.value as? String ?? "").hasSuffix(" revised"))
        capture(app, "remote-voice-edit")
    }
    func testRemoteVoiceBackgroundAndLargeText() {
        let app = remoteVoice(large: true)
        listenRemote(app)
        XCTAssertTrue(app.buttons["cancel-voice"].isHittable); XCTAssertTrue(app.buttons["send-voice"].isHittable)
        capture(app, "remote-voice-large-text")
        XCUIDevice.shared.press(.home); app.activate()
        XCTAssertTrue(app.textViews["remote-prompt"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.textViews["remote-prompt"].value as? String, words)
        XCTAssertTrue(app.staticTexts["voice-notice"].exists)
    }
    func testRemoteVoiceExpandsToEditor() {
        let app = remoteVoice(long: true)
        listenRemote(app)
        let expand = app.buttons["expand-voice-input"]
        XCTAssertTrue(expand.waitForExistence(timeout: 15)); expand.tap()
        let editor = app.textViews["expanded-input"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5)); XCTAssertTrue((editor.value as? String ?? "").hasPrefix(words))
        let finalText = editor.value as? String
        app.buttons["collapse-input"].tap()
        XCTAssertTrue(app.textViews["remote-prompt"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.textViews["remote-prompt"].value as? String, finalText)
    }
    func testRemoteVoiceSendsFinalTextToComputer() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else { throw XCTSkip("Requires the loopback draft fixture") }
        let app = remoteVoice(); listenRemote(app)
        app.buttons["send-voice"].tap()
        XCTAssertTrue(app.staticTexts["DRAFT_FIXTURE_OK: " + words].waitForExistence(timeout: 10))
        XCTAssertFalse(app.buttons["send-voice"].exists)
        XCTAssertEqual(app.textViews["remote-prompt"].value as? String, "")
        capture(app, "remote-voice-sent")
    }
    func testRemoteHomeVoiceEntryUsesSharedPanel() {
        let app = remoteVoice()
        app.navigationBars.buttons.firstMatch.tap()
        XCTAssertTrue(app.buttons["remote-voice"].waitForExistence(timeout: 5)); app.buttons["remote-voice"].tap()
        let transcript = app.textViews["voice-transcript"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value CONTAINS %@", words), object: transcript)], timeout: 8), .completed)
        XCTAssertTrue(app.buttons["cancel-voice"].isHittable); XCTAssertTrue(app.buttons["send-voice"].isHittable)
        capture(app, "remote-home-voice-entry")
        app.buttons["cancel-voice"].tap()
    }

    func testLocalComposerWhitespaceFocusAndButtonExclusion() {
        let app = launch()
        app.buttons["new-chat"].tap()
        checkComposerWhitespace(app, inputID: "composer-input", modelID: "connection-settings",
                                voiceID: "voice-input", sendID: "send-message")
        app.buttons["voice-input"].tap()
        XCTAssertTrue(app.buttons["cancel-voice"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        app.buttons["cancel-voice"].tap()
        app.buttons["add-attachment"].tap()
        XCTAssertTrue(app.buttons["选择文件"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.keyboards.firstMatch.exists)
    }

    func testRemoteComposerWhitespaceFocusAndButtonExclusion() {
        let app = remoteVoice()
        checkComposerWhitespace(app, inputID: "remote-prompt", modelID: "remote-model-settings",
                                voiceID: "remote-voice-input", sendID: "remote-send")
        app.buttons["remote-voice-input"].tap()
        XCTAssertTrue(app.buttons["cancel-voice"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.keyboards.firstMatch.exists)
    }

    private func checkComposerWhitespace(_ app: XCUIApplication, inputID: String, modelID: String,
                                        voiceID: String, sendID: String) {
        let input = app.textViews[inputID]
        let keyboard = app.keyboards.firstMatch
        func tap(_ point: CGPoint) {
            app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: point.x, dy: point.y)).tap()
        }
        func dismiss() {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.02, dy: 0.25)).tap()
            XCTAssertTrue(keyboard.waitForNonExistence(timeout: 3))
        }
        // A disabled send control must not fall through to the focus background.
        XCTAssertFalse(app.buttons[sendID].isEnabled)
        app.buttons[sendID].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertFalse(keyboard.exists)
        // Leading inset, space between editor and toolbar, toolbar spacer, bottom inset.
        for region in 0..<4 {
            let editor = input.frame, model = app.buttons[modelID].frame, voice = app.buttons[voiceID].frame
            let point: CGPoint
            switch region {
            case 0: point = CGPoint(x: editor.minX - 3, y: editor.midY)
            case 1: point = CGPoint(x: editor.midX, y: editor.maxY + 3)
            case 2: point = CGPoint(x: (model.maxX + voice.minX) / 2, y: voice.midY)
            default: point = CGPoint(x: editor.midX, y: voice.maxY + 4)
            }
            tap(point)
            XCTAssertTrue(keyboard.waitForExistence(timeout: 3), "Whitespace region \(region) must focus \(inputID)")
            dismiss()
        }
        input.tap(); input.typeText("Draft stays intact")
        dismiss()
        let model = app.buttons[modelID].frame, voice = app.buttons[voiceID].frame
        tap(CGPoint(x: (model.maxX + voice.minX) / 2, y: voice.midY))
        XCTAssertTrue(keyboard.waitForExistence(timeout: 3))
        XCTAssertEqual(input.value as? String, "Draft stays intact")
        capture(app, inputID + "-whitespace-keyboard")
        dismiss()
    }

}
