import XCTest

/// Seed the simulator photo library with at least 20 images and grant Photos access before running these tests.
final class AttachmentSheetUITests: XCTestCase {
    private func launch(large: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launchArguments += ["-UIPreferredContentSizeCategoryName", large ? "UICTContentSizeCategoryAccessibilityXXXL" : "UICTContentSizeCategoryL"]
        app.launch()
        XCTAssertTrue(app.buttons["add-attachment"].waitForExistence(timeout: 10))
        return app
    }
    private func openPanel(_ app: XCUIApplication) {
        app.buttons["add-attachment"].tap()
        XCTAssertTrue(app.buttons["attachment-close"].waitForExistence(timeout: 5))
        let permission = app.buttons["attachment-photo-permission"]
        if permission.exists {
            permission.tap()
            let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            let allow = springboard.buttons["Allow Full Access"]
            if allow.waitForExistence(timeout: 5) { allow.tap() }
            else {
                let chinese = springboard.buttons.matching(NSPredicate(format: "label CONTAINS %@", "完全访问")).firstMatch
                XCTAssertTrue(chinese.waitForExistence(timeout: 3), springboard.debugDescription)
                if chinese.exists { chinese.tap() }
            }
        }
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot()); attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    private func photos(_ app: XCUIApplication) -> XCUIElementQuery {
        app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "attachment-recent-"))
    }
    private func waitForValue(_ element: XCUIElement, _ value: String) {
        let predicate = NSPredicate(format: "value == %@", value)
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: predicate, object: element)], timeout: 15), .completed)
    }

    func testRecentTapAddsImmediatelyAndClosePreservesDraft() {
        let app = launch()
        let editor = app.textViews["composer-input"]
        editor.tap(); editor.typeText("帮我看看这张图")
        openPanel(app)
        guard photos(app).firstMatch.waitForExistence(timeout: 5) else { XCTFail(app.debugDescription); return }
        XCTAssertFalse(app.staticTexts["轻点照片即可添加，关闭后可继续编辑。"].exists)
        capture(app, "attachments-01-recent")
        let first = photos(app).element(boundBy: 0)
        first.tap(); waitForValue(first, "已添加")
        capture(app, "attachments-02-added")
        XCTAssertTrue(app.buttons["attachment-close"].exists)
        app.buttons["attachment-close"].tap()
        XCTAssertTrue(app.buttons["预览 照片.jpg"].waitForExistence(timeout: 5))
        XCTAssertEqual(editor.value as? String, "帮我看看这张图")
        capture(app, "attachments-03-draft")
        app.buttons["预览 照片.jpg"].tap()
        XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 5))
        app.buttons["close-preview"].tap()
        openPanel(app)
        waitForValue(photos(app).element(boundBy: 0), "已添加")
        photos(app).element(boundBy: 0).tap()
        app.buttons["attachment-close"].tap()
        XCTAssertFalse(app.buttons["预览 照片.jpg"].exists)
        XCTAssertEqual(editor.value as? String, "帮我看看这张图")
    }

    func testContinuousSelectionLimitAndRemoveToFreeSlot() {
        let app = launch()
        openPanel(app)
        guard photos(app).firstMatch.waitForExistence(timeout: 5) else { XCTFail(app.debugDescription); return }
        var lastAdded = ""
        for _ in 0..<20 {
            var next = photos(app).allElementsBoundByIndex.first { $0.isHittable && $0.isEnabled && $0.value as? String != "已添加" }
            for _ in 0..<12 where next == nil {
                // A full-speed swipe can skip thumbnails between viewports.
                let strip = app.scrollViews["attachment-recent-strip"]
                strip.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.5))
                    .press(forDuration: 0.1, thenDragTo: strip.coordinate(withNormalizedOffset: CGVector(dx: 0.35, dy: 0.5)))
                next = photos(app).allElementsBoundByIndex.first { $0.isHittable && $0.isEnabled && $0.value as? String != "已添加" }
            }
            guard let photo = next else { XCTFail("Seed at least 20 photos before running this test"); return }
            lastAdded = photo.identifier
            photo.tap(); waitForValue(photo, "已添加")
        }
        XCTAssertFalse(app.buttons["attachment-all-photos"].isEnabled)
        XCTAssertFalse(app.buttons["attachment-files"].isEnabled)
        capture(app, "attachments-04-limit")
        app.buttons[lastAdded].tap()
        XCTAssertTrue(app.buttons["attachment-all-photos"].isEnabled)
        app.buttons["attachment-close"].tap()
        XCTAssertTrue(app.buttons["attachment-close"].waitForNonExistence(timeout: 5))
        XCTAssertEqual(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "移除 ")).count, 19)
    }

    func testAllPhotosAndFilesCancelReturnToPanel() {
        let app = launch()
        openPanel(app)
        for identifier in ["attachment-all-photos", "attachment-files"] {
            app.buttons[identifier].tap()
            let cancel = app.buttons["取消"].firstMatch
            if cancel.waitForExistence(timeout: 3) { cancel.tap() }
            else { let english = app.buttons["Cancel"].firstMatch; XCTAssertTrue(english.waitForExistence(timeout: 3)); english.tap() }
            XCTAssertTrue(app.buttons["attachment-close"].waitForExistence(timeout: 5))
        }
        app.buttons["attachment-camera"].tap()
        let system = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let deny = system.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "不允许", "Don't Allow")).firstMatch
        if deny.waitForExistence(timeout: 3) { deny.tap() }
        // Newer simulators expose the camera UI; older ones report it unavailable.
        let cameraClose = app.buttons["DismissImagePickerButton"]
        if cameraClose.waitForExistence(timeout: 3) { cameraClose.tap() }
        else {
            let cameraNotice = app.staticTexts.matching(NSPredicate(format: "label == %@ OR label == %@", "这台设备暂时无法使用相机，请从照片中选择。", "请在系统设置中允许 Potato 使用相机。")).firstMatch
            XCTAssertTrue(cameraNotice.waitForExistence(timeout: 3))
        }
        XCTAssertTrue(app.buttons["attachment-close"].waitForExistence(timeout: 5))
        app.buttons["attachment-library"].tap()
        XCTAssertTrue(app.buttons["library-add"].waitForExistence(timeout: 5))
    }

    func testLargeTextPanelRemainsScrollable() {
        let app = launch(large: true)
        openPanel(app)
        XCTAssertTrue(app.buttons["attachment-close"].waitForExistence(timeout: 5))
        capture(app, "attachments-05-large-type")
        app.buttons["attachment-close"].tap()
        XCTAssertTrue(app.buttons["add-attachment"].waitForExistence(timeout: 5))
    }

    func testDarkPanelAndCloseWhileImporting() {
        let app = launch()
        app.buttons["connection-settings"].tap()
        XCTAssertTrue(app.segmentedControls["appearance-picker"].waitForExistence(timeout: 5))
        app.segmentedControls["appearance-picker"].buttons["暗色"].tap()
        app.buttons["save-settings"].tap()
        openPanel(app)
        guard photos(app).firstMatch.waitForExistence(timeout: 5) else { XCTFail(app.debugDescription); return }
        capture(app, "attachments-06-dark")
        photos(app).firstMatch.tap()
        app.buttons["attachment-close"].tap()
        XCTAssertTrue(app.buttons["预览 照片.jpg"].waitForExistence(timeout: 15))
        app.terminate()
        app.launchArguments.removeAll { $0 == "--reset" }
        app.launch()
        XCTAssertTrue(app.buttons["预览 照片.jpg"].waitForExistence(timeout: 10))
    }

    func testZDeniedPhotoAccessStillOffersSystemPicker() {
        let app = launch()
        app.resetAuthorizationStatus(for: .photos)
        app.launch()
        XCTAssertTrue(app.buttons["add-attachment"].waitForExistence(timeout: 10))
        app.buttons["add-attachment"].tap()
        XCTAssertTrue(app.buttons["attachment-photo-permission"].waitForExistence(timeout: 5))
        app.buttons["attachment-photo-permission"].tap()
        let system = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let deny = system.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "不允许", "Don't Allow")).firstMatch
        XCTAssertTrue(deny.waitForExistence(timeout: 5), system.debugDescription)
        deny.tap()
        XCTAssertTrue(app.buttons["attachment-photo-settings"].waitForExistence(timeout: 5))
        capture(app, "attachments-07-denied")
        app.buttons["attachment-all-photos"].tap()
        let cancel = app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "取消", "Cancel")).firstMatch
        XCTAssertTrue(cancel.waitForExistence(timeout: 5)); cancel.tap()
        XCTAssertTrue(app.buttons["attachment-close"].waitForExistence(timeout: 5))
    }
}
