import XCTest

final class RemoteProcessUITests: XCTestCase {
    override func setUpWithError() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_PROCESS_UI"] == "1" else { throw XCTSkip("Requires scripts/remote-process-fixture.py on loopback") }
    }
    private func control(_ mode: String, delay: Double = 0, overviewDelay: Double = 0, runID: String = "fixture-run", stopProtocol: Int = 1) {
        var request = URLRequest(url: URL(string: "http://127.0.0.1:19014/fixture/control")!)
        request.httpMethod = "POST"; request.httpBody = try! JSONSerialization.data(withJSONObject: ["mode": mode, "delay": delay, "overview_delay": overviewDelay, "run_id": runID, "stop_protocol": stopProtocol]); request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let done = expectation(description: "Fixture state")
        URLSession.shared.dataTask(with: request) { _, response, error in XCTAssertNil(error); XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200); done.fulfill() }.resume()
        wait(for: [done], timeout: 5)
    }
    private func launch(large: Bool = false, reduceMotion: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--remote-preview", "--remote-process-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if large { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"] }
        if reduceMotion { app.launchArguments.append("--reduce-motion-preview") }
        app.launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "调整侧栏和项目导航")).firstMatch
        for _ in 0..<8 { if row.exists && row.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(row.isHittable); row.tap(); return app
    }
    private func status(_ app: XCUIApplication, _ value: String, timeout: Double = 10) {
        let item = app.staticTexts["remote-current-status"]
        if value == "正在思考" {
            // The process row names the thought; the dot below it stays without text of its own.
            let row = app.buttons.matching(NSPredicate(format: "identifier == %@ AND label BEGINSWITH %@", "remote-process-toggle", value)).firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: timeout))
            XCTAssertLessThanOrEqual(app.staticTexts.matching(NSPredicate(format: "label == %@", value)).count, 1, "正在思考 is shown twice")
            XCTAssertFalse(item.exists)
            return
        }
        if ["正在执行", "正在回复"].contains(value) {
            let indicator = app.descendants(matching: .any).matching(NSPredicate(format: "identifier IN %@ AND label == %@", ["remote-activity-spinner", "remote-static-activity"], value)).firstMatch
            XCTAssertTrue(indicator.waitForExistence(timeout: timeout))
            XCTAssertFalse(item.exists)
            return
        }
        if value == "本轮任务已完成" {
            // Completion restores the composer without inserting a redundant
            // receipt into the conversation. Wait for the running controls to go.
            XCTAssertTrue(app.buttons["remote-copy-reply"].waitForExistence(timeout: timeout))
            XCTAssertTrue(app.buttons["remote-stop"].waitForNonExistence(timeout: timeout))
            XCTAssertTrue(item.waitForNonExistence(timeout: timeout))
            XCTAssertTrue(app.buttons["remote-send"].exists)
            XCTAssertFalse(app.staticTexts["本轮任务已完成"].exists)
            return
        }
        XCTAssertTrue(item.waitForExistence(timeout: timeout))
        let expected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", value), object: item)
        XCTAssertEqual(XCTWaiter.wait(for: [expected], timeout: timeout), .completed, "Actual status: \(item.label)")
    }
    private func spinner(_ app: XCUIApplication) -> XCUIElement { app.descendants(matching: .any).matching(identifier: "remote-activity-spinner").firstMatch }
    private func input(_ app: XCUIApplication) -> XCUIElement { app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch }
    private func capture(_ app: XCUIApplication, _ name: String) { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = name; shot.lifetime = .keepAlways; add(shot) }
    func testConversationGroupsProcessAndSharesWholeReply() {
        control("conversation"); let app = launch(); status(app, "本轮任务已完成")
        XCTAssertEqual(app.buttons.matching(identifier: "remote-process-toggle").count, 1)
        XCTAssertEqual(app.buttons.matching(identifier: "remote-copy-reply").count, 1)
        XCTAssertFalse(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "The user asks")).firstMatch.exists)
        capture(app, "remote-conversation-unified")
        app.buttons["remote-reply-more"].tap(); app.buttons["remote-select-reply"].tap()
        let selection = app.textViews["selectable-reply"]
        XCTAssertTrue(selection.waitForExistence(timeout: 5))
        let text = selection.value as? String ?? ""
        XCTAssertTrue(text.contains("我来看看桌面上的内容。"))
        XCTAssertTrue(text.contains("我再确认一下名称。"))
        XCTAssertTrue(text.contains("你想先打开哪一个？"))
        app.buttons["close-text-selection"].tap()
        // Commentary stays between the groups it introduced; the first group holds the opening thought.
        let process = app.buttons["remote-process-toggle-0"]
        if !process.isHittable { app.scrollViews["remote-conversation"].swipeDown() }
        process.tap()
        // A group holding only a thought opens straight into it.
        XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["activity-reasoning"].label.contains("The user asks"))
        capture(app, "remote-conversation-process")
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        app.buttons["remote-conversation-options"].tap()
        XCTAssertTrue(app.buttons["置顶对话"].waitForExistence(timeout: 3))
    }

    func testConversationKeyboardDismissesWithoutLosingDraft() {
        control("conversation"); let app = launch(); status(app, "本轮任务已完成")
        input(app).tap(); input(app).typeText("keep this draft")
        XCTAssertTrue(app.keyboards.firstMatch.exists)
        // The scroll view extends under the composer/keyboard. Drag on its
        // visible reading surface, rather than its full accessibility frame.
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.44)))
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 5))
        XCTAssertEqual(input(app).value as? String, "keep this draft")
        input(app).tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 3))
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.02, dy: 0.25)).tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 5))
        XCTAssertEqual(input(app).value as? String, "keep this draft")
        capture(app, "remote-conversation-draft")
    }

    func testThinkingToolReplyCompletionAndDisclosure() {
        control("thinking"); let app = launch(); status(app, "正在思考")
        // The dot stays at the tail while the row names the thought.
        XCTAssertTrue(spinner(app).exists)
        // A lone thought opens straight into its text.
        app.buttons["remote-process-toggle"].tap()
        XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 5)); capture(app, "remote-thinking-expanded")
        app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
        control("tool"); status(app, "正在执行")
        // One live indicator: the tail dot. No system spinner on screen (the ScrollView's
        // offscreen pull-to-refresh spinner remains in the AX tree, so filter to visible ones).
        XCTAssertEqual(app.descendants(matching: .any).matching(identifier: "remote-activity-spinner").count, 1)
        XCTAssertEqual(app.activityIndicators.allElementsBoundByIndex.filter { !$0.frame.isEmpty && app.frame.contains($0.frame) && $0.isHittable }.count, 0)
        app.buttons["remote-process-toggle"].tap(); capture(app, "remote-tool-active"); app.buttons["activity-close"].tap()
        control("reply"); status(app, "正在回复")
        XCTAssertFalse(app.buttons["remote-copy-reply"].exists)
        capture(app, "remote-body-active")
        control("complete"); status(app, "本轮任务已完成")
        XCTAssertFalse(spinner(app).exists); XCTAssertFalse(app.buttons["remote-stop"].exists)
        XCTAssertTrue(app.buttons["remote-copy-reply"].exists); capture(app, "remote-completed")
        // XCTest terminates the app at suite teardown. Keep the final state on
        // screen briefly when recording, so Simulator's video encoder catches it.
        if ProcessInfo.processInfo.environment["POTATO_IOS_PROCESS_VIDEO"] == "1" { Thread.sleep(forTimeInterval: 4) }
    }
    func testDisconnectStopsAnimationKeepsDraftAndReconnects() {
        control("thinking"); let app = launch(); status(app, "正在思考")
        input(app).tap(); input(app).typeText("keep draft")
        // A running task must remain stoppable when the shared composer switches to send.
        XCTAssertTrue(app.buttons["remote-stop"].isHittable)
        XCTAssertTrue(app.buttons["remote-send"].isHittable)
        XCTAssertLessThan(input(app).frame.maxY, app.buttons["remote-send"].frame.minY)
        control("offline"); status(app, "连接中断，任务状态未确认")
        XCTAssertFalse(spinner(app).exists); XCTAssertFalse(app.buttons["remote-send"].isEnabled); XCTAssertFalse(app.buttons["remote-stop"].isEnabled)
        XCTAssertEqual(input(app).value as? String, "keep draft"); capture(app, "remote-offline-draft-keyboard")
        control("tool"); status(app, "正在执行")
        XCTAssertTrue(spinner(app).exists); XCTAssertTrue(app.buttons["remote-send"].isEnabled); XCTAssertEqual(input(app).value as? String, "keep draft")
        capture(app, "remote-reconnected")
    }
    func testSlowRequestsExpireSnapshotAndModelLookupDoesNotBlockChat() {
        control("thinking", overviewDelay: 12); let app = launch(); status(app, "正在思考", timeout: 5)
        // A slow poll keeps the last known state on screen; only acting waits for confirmation.
        control("thinking", delay: 15)
        let stop = app.buttons["remote-stop"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "isEnabled == false"), object: stop)], timeout: 14), .completed)
        status(app, "正在思考", timeout: 1); XCTAssertFalse(app.staticTexts["remote-last-status"].exists)
        status(app, "正在确认任务状态…", timeout: 25)
        XCTAssertFalse(spinner(app).exists); XCTAssertTrue(app.staticTexts["remote-last-status"].exists); capture(app, "remote-slow-request-stale")
        control("complete"); status(app, "本轮任务已完成", timeout: 30)
    }
    func testApprovalQuestionStopAndFailureUseRealStates() {
        control("approval"); let app = launch(); XCTAssertTrue(app.staticTexts["remote-approval-title"].waitForExistence(timeout: 10))
        XCTAssertFalse(spinner(app).exists); app.buttons["remote-approval-allow"].tap(); status(app, "本轮任务已完成")
        control("question"); status(app, "等待你的回答"); XCTAssertFalse(spinner(app).exists)
        app.buttons["方案 A"].tap(); app.buttons["提交回答"].tap(); status(app, "本轮任务已完成")
        control("thinking"); status(app, "正在思考"); app.buttons["remote-stop"].tap()
        app.sheets.buttons["停止任务"].tap(); status(app, "本轮任务已停止")
        XCTAssertFalse(spinner(app).exists); capture(app, "remote-stopped")
        control("failed"); status(app, "本轮任务失败"); XCTAssertFalse(spinner(app).exists)
        capture(app, "remote-failed")
    }
    func testApprovalCanBeDeferredReopenedAndRejected() {
        control("approval"); let app = launch()
        XCTAssertTrue(app.staticTexts["remote-approval-title"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.staticTexts["remote-approval-scope-note"].exists)
        XCTAssertEqual(app.buttons["remote-approval-allow"].label, "允许")
        capture(app, "remote-approval-sheet")
        app.buttons["稍后处理"].tap()
        status(app, "等待你的批准")
        app.buttons["remote-open-approval"].tap()
        XCTAssertTrue(app.buttons["remote-approval-deny"].waitForExistence(timeout: 5))
        app.buttons["remote-approval-deny"].tap()
        status(app, "本轮任务已完成")
    }
    func testApprovalPersistentDirectorySelection() {
        control("approval"); let app = launch()
        XCTAssertTrue(app.staticTexts["remote-approval-title"].waitForExistence(timeout: 10))
        app.buttons["remote-approval-scope-persistent_directory"].tap()
        XCTAssertTrue(app.staticTexts["/tmp/potato-fixture"].exists)
        XCTAssertTrue(app.staticTexts["remote-approval-scope-note"].exists)
        XCTAssertEqual(app.buttons["remote-approval-allow"].label, "允许")
        capture(app, "remote-approval-directory")
        app.buttons["remote-approval-allow"].tap()
        status(app, "本轮任务已完成")
    }
    func testApprovalDisconnectAndExpiryDisableDecisions() {
        control("approval"); let app = launch(large: true)
        let allow = app.buttons["remote-approval-allow"]
        XCTAssertTrue(allow.waitForExistence(timeout: 10))
        control("offline")
        let disabled = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == false"), object: allow)
        XCTAssertEqual(XCTWaiter.wait(for: [disabled], timeout: 10), .completed)
        control("complete")
        XCTAssertTrue(app.staticTexts["这项审批已处理或已过期。"].waitForExistence(timeout: 10))
        XCTAssertFalse(allow.isEnabled)
        capture(app, "remote-approval-expired-large")
    }
    func testBackgroundRequiresReconfirmationWithReducedMotion() {
        control("thinking"); let app = launch(reduceMotion: true); status(app, "正在思考")
        XCTAssertFalse(spinner(app).exists); XCTAssertTrue(app.images["remote-static-activity"].exists)
        input(app).tap(); input(app).typeText("background draft"); capture(app, "remote-reduced-motion")
        XCUIDevice.shared.press(.home)
        control("complete", delay: 12); app.activate()
        // A brief absence keeps the last known state on screen, but sending waits for a fresh poll.
        XCTAssertTrue(app.images["remote-static-activity"].waitForExistence(timeout: 5)); XCTAssertFalse(spinner(app).exists)
        XCTAssertFalse(app.buttons["remote-send"].isEnabled); XCTAssertEqual(input(app).value as? String, "background draft")
        capture(app, "remote-foreground-unconfirmed")
        control("complete"); status(app, "本轮任务已完成", timeout: 18)
    }
    func testStopConfirmationCannotCancelAReplacementRun() {
        control("thinking"); let app = launch(); status(app, "正在思考")
        app.buttons["remote-stop"].tap()
        XCTAssertTrue(app.sheets.buttons["停止任务"].waitForExistence(timeout: 3))
        control("tool", runID: "replacement-run")
        // Polling changes the underlying snapshot while the confirmation remains open.
        status(app, "正在执行")
        app.sheets.buttons["停止任务"].tap()
        XCTAssertTrue(app.staticTexts["原任务已结束或改变，未停止其他任务，请刷新后确认"].waitForExistence(timeout: 8))
        status(app, "正在执行"); XCTAssertTrue(spinner(app).exists)
        capture(app, "remote-stale-stop-rejected")
        app.buttons["remote-stop"].tap(); app.sheets.buttons["停止任务"].tap()
        status(app, "本轮任务已停止")
    }
    func testOldComputerRequiresUpdateBeforeStopping() {
        control("thinking", stopProtocol: 0); let app = launch(large: true); status(app, "正在思考")
        XCTAssertTrue(app.staticTexts["remote-stop-update-required"].exists)
        XCTAssertTrue(app.frame.contains(app.staticTexts["remote-stop-update-required"].frame))
        XCTAssertFalse(app.buttons["remote-stop"].exists)
        capture(app, "remote-old-computer-stop")
    }
    func testLargeTextOfflineStatusAndKeyboardAreReachable() {
        control("thinking"); let app = launch(large: true); status(app, "正在思考")
        input(app).tap(); input(app).typeText("draft")
        control("offline"); status(app, "连接中断，任务状态未确认")
        XCTAssertTrue(input(app).isHittable); XCTAssertTrue(app.buttons["remote-status-refresh"].isHittable)
        XCTAssertLessThanOrEqual(input(app).frame.maxY, app.keyboards.firstMatch.frame.minY)
        XCTAssertTrue(app.frame.contains(input(app).frame))
        XCTAssertFalse(spinner(app).exists); capture(app, "remote-large-offline-keyboard")
        control("complete"); status(app, "本轮任务已完成")
        XCTAssertTrue(app.buttons["remote-send"].isHittable); XCTAssertEqual(input(app).value as? String, "draft")
    }
}
