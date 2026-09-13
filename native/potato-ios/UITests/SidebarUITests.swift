import XCTest

final class SidebarUITests: XCTestCase {
    private func launch(remote: Bool = true, extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"] + extra
        if remote { app.launchArguments.append("--remote-preview") }
        app.launch()
        XCTAssertTrue(app.buttons[remote ? "remote-sidebar" : "history"].waitForExistence(timeout: 10))
        return app
    }
    private func drag(_ app: XCUIApplication, from: CGVector, to: CGVector, hold: TimeInterval = 0) {
        app.coordinate(withNormalizedOffset: from).press(forDuration: 0.05,
            thenDragTo: app.coordinate(withNormalizedOffset: to), withVelocity: .slow, thenHoldForDuration: hold)
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let image = XCTAttachment(screenshot: app.screenshot()); image.name = name; image.lifetime = .keepAlways; add(image)
    }
    func testFullSurfaceDragOpensAndBackdropCloses() {
        let app = launch()
        drag(app, from: CGVector(dx: 0.22, dy: 0.56), to: CGVector(dx: 0.88, dy: 0.56))
        let close = app.buttons["close-sidebar"]
        XCTAssertTrue(close.waitForExistence(timeout: 3)); XCTAssertTrue(close.isHittable)
        XCTAssertTrue(app.buttons["sidebar-new-chat"].isHittable)
        capture(app, "sidebar-full-surface-open")
        close.tap()
        XCTAssertTrue(close.waitForNonExistence(timeout: 3))
        XCTAssertTrue(app.buttons["remote-sidebar"].isHittable)
    }
    func testShortSlowDragsReturnToTheirStartingState() {
        let app = launch()
        drag(app, from: CGVector(dx: 0.04, dy: 0.55), to: CGVector(dx: 0.12, dy: 0.55), hold: 0.5)
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        app.buttons["remote-sidebar"].tap()
        drag(app, from: CGVector(dx: 0.55, dy: 0.55), to: CGVector(dx: 0.47, dy: 0.55), hold: 0.5)
        XCTAssertTrue(app.buttons["close-sidebar"].isHittable)
        capture(app, "sidebar-short-close-cancelled")
    }
    func testVerticalScrollActuallyMovesContentWithoutOpeningDrawer() {
        let app = launch(extra: ["--sidebar-long-list-preview"])
        let row = app.buttons["在dynamo新建任务"]
        let before = row.frame.minY
        drag(app, from: CGVector(dx: 0.55, dy: 0.73), to: CGVector(dx: 0.56, dy: 0.39))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        XCTAssertTrue(!row.isHittable || abs(row.frame.minY - before) > 40, "Vertical scrolling must still move the list.")
        capture(app, "sidebar-vertical-scroll-preserved")
    }
    func testKeyboardDismissalPreservesLocalDraft() {
        let app = launch(remote: false)
        app.buttons["new-chat"].tap()
        let input = app.textViews["composer-input"]
        input.tap(); input.typeText("Keep this unsent draft")
        XCTAssertTrue(app.keyboards.firstMatch.exists)
        drag(app, from: CGVector(dx: 0.15, dy: 0.30), to: CGVector(dx: 0.87, dy: 0.30))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 3))
        drag(app, from: CGVector(dx: 0.6, dy: 0.5), to: CGVector(dx: 0.05, dy: 0.5))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForNonExistence(timeout: 3))
        XCTAssertEqual(input.value as? String, "Keep this unsent draft")
        capture(app, "sidebar-local-draft-preserved")
    }
    func testSidebarSearchKeyboardDismissesOnClose() {
        let app = launch()
        app.buttons["remote-sidebar"].tap(); app.buttons["搜索会话"].tap()
        let search = app.textFields["sidebar-search"]
        XCTAssertTrue(search.waitForExistence(timeout: 3)); search.tap(); search.typeText("not-found")
        drag(app, from: CGVector(dx: 0.6, dy: 0.40), to: CGVector(dx: 0.04, dy: 0.40))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForNonExistence(timeout: 3))
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 3))
        app.buttons["remote-sidebar"].tap()
        XCTAssertEqual(search.value as? String, "not-found")
    }
    func testDevicePickerAndComposerKeepTheirHorizontalGestures() {
        var app = launch()
        let picker = app.scrollViews["remote-device-picker"]
        XCTAssertTrue(picker.exists)
        picker.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.5)).press(forDuration: 0.05,
            thenDragTo: picker.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.5)))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        app.terminate(); app = launch(remote: false)
        app.buttons["new-chat"].tap()
        let input = app.textViews["composer-input"]
        input.tap(); input.typeText("Do not open a drawer while editing")
        input.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.5)).press(forDuration: 0.05,
            thenDragTo: input.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        XCTAssertEqual(input.value as? String, "Do not open a drawer while editing")
    }
    func testPresentedDeviceSheetDoesNotMoveWorkspace() {
        let app = launch()
        app.buttons["remote-options"].tap(); app.buttons["管理电脑"].tap()
        XCTAssertTrue(app.navigationBars["管理电脑"].waitForExistence(timeout: 3))
        drag(app, from: CGVector(dx: 0.25, dy: 0.45), to: CGVector(dx: 0.85, dy: 0.45))
        XCTAssertTrue(app.navigationBars["管理电脑"].exists)
        app.buttons["完成"].tap()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
    }
    func testCodeAndTableCanPanWithoutOpeningDrawer() {
        let app = launch(remote: false, extra: ["--sidebar-code-preview"])
        for id in ["markdown-code-scroll", "markdown-table-scroll"] {
            let scroll = app.scrollViews[id].firstMatch
            XCTAssertTrue(scroll.waitForExistence(timeout: 3)); XCTAssertTrue(scroll.isHittable)
            let text = scroll.staticTexts.firstMatch
            let before = text.frame.minX
            scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.5)).press(forDuration: 0.05,
                thenDragTo: scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.5)))
            XCTAssertFalse(app.buttons["close-sidebar"].exists)
            XCTAssertTrue(text.frame.minX < before - 30 || !text.isHittable, "The horizontal content should move.")
            scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.5)).press(forDuration: 0.05,
                thenDragTo: scroll.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.5)))
            XCTAssertFalse(app.buttons["close-sidebar"].exists)
            XCTAssertFalse(app.buttons["scroll-latest"].exists, "Horizontal content must not disable following the conversation.")
            capture(app, "sidebar-excluded-" + id)
        }
    }
    func testConversationScrollStillPausesAndResumesFollowing() {
        let app = launch(remote: false, extra: ["--sidebar-long-chat-preview"])
        let latest = app.buttons["scroll-latest"]
        XCTAssertFalse(latest.exists)
        drag(app, from: CGVector(dx: 0.55, dy: 0.35), to: CGVector(dx: 0.55, dy: 0.72))
        XCTAssertTrue(latest.waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        capture(app, "conversation-following-paused")
        latest.tap()
        XCTAssertTrue(latest.waitForNonExistence(timeout: 3))
        XCTAssertTrue(app.staticTexts["历史消息 30"].isHittable)
    }
    func testNestedRemotePagesKeepNativeBackGesture() {
        let app = launch()
        let project = app.buttons["remote-project-00000000-0000-4000-8000-000000000001-/fixture/Potato"]
        XCTAssertTrue(project.exists); project.tap()
        app.buttons["在此项目新建任务"].tap()
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch.waitForExistence(timeout: 3))
        XCTAssertTrue(app.navigationBars["新远程任务"].buttons.firstMatch.isHittable)
        capture(app, "remote-task-native-back-entry")
        drag(app, from: CGVector(dx: 0.015, dy: 0.5), to: CGVector(dx: 0.88, dy: 0.5))
        XCTAssertTrue(app.buttons["在此项目新建任务"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        drag(app, from: CGVector(dx: 0.015, dy: 0.5), to: CGVector(dx: 0.88, dy: 0.5))
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        drag(app, from: CGVector(dx: 0.2, dy: 0.55), to: CGVector(dx: 0.85, dy: 0.55))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForExistence(timeout: 3))
        capture(app, "sidebar-restored-after-native-back")
    }
    func testReduceMotionAndLargeTextStillAllowOpenAndClose() {
        let app = launch(extra: ["--reduce-motion-preview", "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"])
        drag(app, from: CGVector(dx: 0.03, dy: 0.55), to: CGVector(dx: 0.85, dy: 0.55))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForExistence(timeout: 3))
        XCTAssertTrue(app.buttons["sidebar-new-chat"].isHittable)
        capture(app, "sidebar-large-text-reduce-motion")
        drag(app, from: CGVector(dx: 0.65, dy: 0.5), to: CGVector(dx: 0.04, dy: 0.5))
        XCTAssertTrue(app.buttons["close-sidebar"].waitForNonExistence(timeout: 3))
    }
}
