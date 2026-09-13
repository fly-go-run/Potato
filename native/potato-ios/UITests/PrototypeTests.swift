import XCTest
final class PrototypeTests: XCTestCase {
    func launch(reset: Bool = true, largeText: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--slow-stream", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if largeText { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"] }
        app.launch(); return app
    }
    func capture(_ app: XCUIApplication, _ name: String) {
        let image = XCTAttachment(screenshot: app.screenshot()); image.name = name; image.lifetime = .keepAlways; add(image)
    }
    func testDraftEditCopyShareAndPersistence() {
        let app = launch()
        XCTAssertTrue(app.buttons["expand-document"].waitForExistence(timeout: 10)); capture(app, "01-working-draft")
        app.buttons["expand-document"].tap()
        let item = app.buttons["去附近的公园走走，晒晒太阳"]
        XCTAssertTrue(item.waitForExistence(timeout: 3)); item.tap(); XCTAssertEqual(item.value as? String, "已完成")
        app.buttons["copy-document"].tap(); XCTAssertTrue(app.buttons["copy-document"].label.contains("已复制"))
        app.buttons["edit-document"].tap()
        XCTAssertTrue(app.textViews["text-editor"].waitForExistence(timeout: 3)); capture(app, "02-document-editor")
        app.buttons["save-edit"].tap(); app.buttons["share-document"].tap()
        XCTAssertTrue(app.otherElements["ActivityListView"].waitForExistence(timeout: 5) || app.buttons["拷贝"].exists); capture(app, "03-share")
        app.terminate(); let restored = launch(reset: false)
        XCTAssertTrue(restored.buttons["expand-document"].waitForExistence(timeout: 10)); restored.buttons["edit-document"].tap()
        XCTAssertTrue(restored.textViews["text-editor"].waitForExistence(timeout: 3))
        XCTAssertTrue((restored.textViews["text-editor"].value as? String ?? "").contains("[x]"))
    }
    func testNewChatKeyboardStreamingStopAndHistory() {
        let app = launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap(); capture(app, "04-new-chat")
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 3)); input.tap(); input.typeText("Plan a calm weekend"); capture(app, "05-keyboard")
        XCTAssertTrue(app.buttons["send-message"].isHittable); app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["stop-generation"].waitForExistence(timeout: 3)); app.buttons["stop-generation"].tap()
        XCTAssertTrue(app.staticTexts["已停止生成"].waitForExistence(timeout: 3)); capture(app, "06-stopped")
        app.buttons["retry-message"].tap(); XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 15)); XCTAssertFalse(app.keyboards.firstMatch.exists)
        XCTAssertTrue(app.buttons["previous-reply"].waitForExistence(timeout: 3))
        app.buttons["previous-reply"].tap()
        XCTAssertTrue(app.staticTexts["已停止生成"].waitForExistence(timeout: 3)); capture(app, "19-reply-version")
        app.buttons["next-reply"].tap()
        XCTAssertFalse(app.staticTexts["已停止生成"].exists)
        app.buttons["history"].tap(); app.buttons["搜索会话"].tap(); XCTAssertTrue(app.textFields["sidebar-search"].waitForExistence(timeout: 3)); capture(app, "07-history")
        app.textFields["sidebar-search"].tap(); app.textFields["sidebar-search"].typeText("calm")
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Plan a calm weekend")).firstMatch.waitForExistence(timeout: 3)); capture(app, "08-search")
    }
    func testLargeTextLayout() {
        let app = launch(largeText: true)
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10)); app.buttons["close-document"].tap()
        XCTAssertTrue(app.buttons["open-document"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.buttons["history"].isHittable)
        XCTAssertTrue(app.buttons["connection-settings"].isHittable)
        capture(app, "11-accessibility-text")
    }
    func testSettingsValidationAndLargeText() {
        let app = launch()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10)); app.buttons["close-document"].tap(); capture(app, "09-accessibility-text")
        app.buttons["connection-settings"].tap(); XCTAssertTrue(app.switches["demo-mode"].waitForExistence(timeout: 3))
        let toggle = app.switches["demo-mode"]
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertEqual(toggle.value as? String, "0")
        capture(app, "10-settings")
        app.buttons["save-settings"].tap()
        XCTAssertTrue(app.alerts["无法保存"].waitForExistence(timeout: 3)); capture(app, "10-settings-validation")
    }
    func testAttachmentAndVoiceEntryPoints() {
        let app = launch()
        XCTAssertTrue(app.buttons["add-attachment"].waitForExistence(timeout: 10))
        app.buttons["add-attachment"].tap()
        XCTAssertTrue(app.buttons["照片图库"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.buttons["选择文件"].exists)
        capture(app, "12-attachment-menu")
        app.buttons["照片图库"].tap()
        capture(app, "13-photo-picker")
        let cancel = app.buttons["取消"].firstMatch
        if cancel.waitForExistence(timeout: 3) { cancel.tap() }
        else if app.buttons["Cancel"].waitForExistence(timeout: 3) { app.buttons["Cancel"].tap() }
        XCTAssertTrue(app.buttons["voice-input"].waitForExistence(timeout: 3))
        app.buttons["voice-input"].tap()
        XCTAssertTrue(app.staticTexts["voice-notice"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["开始录音"].exists)
        XCTAssertFalse(app.buttons["放入输入框"].exists)
        capture(app, "14-voice-ready")
    }

    func testPhotoImportPreviewRemoval() {
        let app = launch()
        XCTAssertTrue(app.buttons["add-attachment"].waitForExistence(timeout: 10))
        app.buttons["add-attachment"].tap(); app.buttons["照片图库"].tap()
        let photo = app.images.matching(identifier: "PXGGridLayout-Info").firstMatch
        XCTAssertTrue(photo.waitForExistence(timeout: 5))
        photo.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        
        let done = app.buttons["Add"]
        XCTAssertTrue(done.waitForExistence(timeout: 3)); done.tap()
        let preview = app.buttons["预览 照片.jpg"]
        XCTAssertTrue(preview.waitForExistence(timeout: 8)); capture(app, "15-photo-imported")
        preview.tap(); XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 5)); capture(app, "16-photo-preview")
        app.buttons["close-preview"].tap()
        let remove = app.buttons["移除 照片.jpg"]
        XCTAssertTrue(remove.waitForExistence(timeout: 3)); remove.tap()
        XCTAssertFalse(preview.exists)
    }

    func testReplyActionsAndMultiPhotoGallery() {
        let app = launch()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 10)); app.buttons["close-document"].tap()
        app.buttons["copy-reply"].tap()
        XCTAssertTrue(app.buttons["copy-reply"].label.contains("已复制"))
        app.buttons["reply-more"].tap(); capture(app, "25-reply-menu")
        app.buttons["选择文字"].tap()
        XCTAssertTrue(app.textViews["selectable-reply"].waitForExistence(timeout: 3)); capture(app, "26-select-text")
        app.buttons["close-text-selection"].tap()
        app.buttons["new-chat"].tap()
        app.buttons["add-attachment"].tap(); app.buttons["照片图库"].tap()
        let photos = app.images.matching(identifier: "PXGGridLayout-Info")
        XCTAssertTrue(photos.firstMatch.waitForExistence(timeout: 5)); XCTAssertGreaterThanOrEqual(photos.count, 4)
        for i in 0..<4 { photos.element(boundBy: i).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap() }
        app.buttons["Add"].tap()
        let pending = app.buttons.matching(NSPredicate(format: "label == %@", "预览 照片.jpg"))
        let imported = XCTNSPredicateExpectation(predicate: NSPredicate(format: "count == 4"), object: pending)
        XCTAssertEqual(XCTWaiter.wait(for: [imported], timeout: 15), .completed)
        capture(app, "27-four-pending-images")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["stop-generation"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.otherElements["streaming-status"].waitForExistence(timeout: 3) || app.staticTexts["正在回复"].exists)
        capture(app, "28-streaming-images")
        app.buttons["stop-generation"].tap()
        XCTAssertTrue(app.buttons["message-image-3"].waitForExistence(timeout: 3))
        app.buttons["message-image-1"].tap()
        XCTAssertTrue(app.buttons["next-image"].waitForExistence(timeout: 5)); capture(app, "29-image-gallery")
        XCTAssertEqual(app.staticTexts["image-position"].label, "2 / 4")
        app.buttons["next-image"].tap()
        let third = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "3 / 4"), object: app.staticTexts["image-position"])
        XCTAssertEqual(XCTWaiter.wait(for: [third], timeout: 3), .completed)
        app.buttons["previous-image"].tap()
        app.buttons["close-preview"].tap()
        app.terminate(); let restored = launch(reset: false)
        XCTAssertTrue(restored.buttons["message-image-3"].waitForExistence(timeout: 10)); capture(restored, "30-multi-image-restored")
    }

    func testDraftVersionRestore() {
        let app = launch()
        XCTAssertTrue(app.buttons["expand-document"].waitForExistence(timeout: 10)); app.buttons["expand-document"].tap()
        let item = app.buttons["去附近的公园走走，晒晒太阳"]
        XCTAssertTrue(item.waitForExistence(timeout: 3)); item.tap()
        let checked = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", "已完成"), object: item)
        XCTAssertEqual(XCTWaiter.wait(for: [checked], timeout: 3), .completed)
        app.buttons["draft-history"].tap()
        let version = app.buttons.matching(identifier: "draft-version").firstMatch
        XCTAssertTrue(version.waitForExistence(timeout: 3)); capture(app, "17-draft-history")
        version.tap()
        XCTAssertTrue(app.buttons["restore-draft-version"].waitForExistence(timeout: 3)); capture(app, "18-draft-version-preview")
        app.buttons["restore-draft-version"].tap()
        XCTAssertTrue(item.waitForExistence(timeout: 3)); XCTAssertEqual(item.value as? String, "未完成")
        app.buttons["draft-history"].tap()
        XCTAssertTrue(app.buttons.matching(identifier: "draft-version").firstMatch.waitForExistence(timeout: 3))
        XCTAssertEqual(app.buttons.matching(identifier: "draft-version").count, 2)
    }

    func testConnectionSetupCanBeReviewedWithoutSendingOrSaving() {
        let app = launch()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10)); app.buttons["connection-settings"].tap()
        let probe = app.buttons["test-connection"]
        XCTAssertTrue(probe.waitForExistence(timeout: 3)); XCTAssertFalse(probe.isEnabled)
        let endpoint = app.textFields["endpoint"]
        endpoint.tap(); endpoint.typeText("https://fixture.invalid/v1/chat/completions\n")
        let model = app.textFields["model-name"]
        model.typeText("fixture-model")
        app.buttons["dismiss-settings-keyboard"].tap()
        if !probe.isHittable { app.swipeUp() }
        XCTAssertTrue(probe.isEnabled)
        XCTAssertTrue(probe.isHittable)
        capture(app, "20-connection-setup")
        app.buttons["取消"].tap()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 3)); app.buttons["connection-settings"].tap()
        XCTAssertEqual(app.textFields["endpoint"].value as? String, "完整接口地址（HTTPS）")
        XCTAssertFalse(app.buttons["test-connection"].isEnabled)
    }

}
