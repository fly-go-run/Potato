import XCTest

final class RemoteUITests: XCTestCase {
    private func launch(preview: Bool = true, reset: Bool = true) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if preview { app.launchArguments.append("--remote-preview") }
        app.launch(); return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot()); attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    private func launchDraftFixture(reset: Bool = true, legacy: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--remote-preview", "--remote-draft-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if legacy { app.launchArguments.append("--remote-legacy-draft-preview") }
        app.launch(); return app
    }
    private func openDraft(_ app: XCUIApplication, second: Bool = false) -> XCUIElement {
        let id = second ? "00000000-0000-4000-8000-000000000002" : "00000000-0000-4000-8000-000000000001"
        let device = app.buttons["remote-device-" + id]
        XCTAssertTrue(device.waitForExistence(timeout: 10)); device.tap()
        app.buttons["remote-new-task"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5)); return input
    }
    private func backToRemoteHome(_ app: XCUIApplication) {
        app.navigationBars.buttons.firstMatch.tap()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 5))
    }
    func testAccountDeviceDraftsStaySeparateAcrossNavigationAndRelaunch() {
        var app = launchDraftFixture()
        var input = openDraft(app); input.tap(); input.typeText("only-first-mac")
        backToRemoteHome(app)
        input = openDraft(app, second: true)
        XCTAssertFalse((input.value as? String ?? "").contains("only-first-mac"))
        input.tap(); input.typeText("only-second-mac")
        backToRemoteHome(app)
        input = openDraft(app)
        XCTAssertEqual(input.value as? String, "only-first-mac")
        capture(app, "draft-first-computer")
        app.terminate(); app = launchDraftFixture(reset: false)
        input = openDraft(app, second: true)
        XCTAssertEqual(input.value as? String, "only-second-mac")
        capture(app, "draft-second-computer-after-relaunch")
    }
    func testLegacyDraftRequiresExplicitTargetConfirmation() {
        let app = launchDraftFixture(legacy: true)
        let input = openDraft(app)
        XCTAssertFalse((input.value as? String ?? "").contains("旧版"))
        app.buttons["remote-legacy-draft"].tap()
        XCTAssertTrue(app.buttons["remote-confirm-legacy-draft"].waitForExistence(timeout: 3))
        capture(app, "draft-legacy-target-review")
        app.buttons["取消"].tap()
        XCTAssertFalse((input.value as? String ?? "").contains("旧版"))
        app.buttons["remote-legacy-draft"].tap()
        app.buttons["remote-confirm-legacy-draft"].tap()
        XCTAssertTrue(app.buttons["重试确认发送结果"].waitForExistence(timeout: 5))
        XCTAssertEqual(input.value as? String, "旧版草稿：先核对目标电脑")
        XCTAssertFalse(app.buttons["remote-send"].isEnabled)
        capture(app, "draft-legacy-restored-without-dispatch")
        backToRemoteHome(app)
        let second = openDraft(app, second: true)
        XCTAssertFalse((second.value as? String ?? "").contains("旧版"))
        XCTAssertFalse(app.buttons["重试确认发送结果"].exists)
        XCTAssertFalse(app.buttons["remote-legacy-draft"].exists)
    }
    func testAcknowledgedRemoteDraftReopensInItsConversation() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else { throw XCTSkip("Requires scripts/remote-draft-fixture.py on loopback") }
        var app = launchDraftFixture()
        var input = openDraft(app); input.tap(); input.typeText("fixture-first")
        app.buttons["remote-send"].tap()
        XCTAssertTrue(app.staticTexts["DRAFT_FIXTURE_OK: fixture-first"].waitForExistence(timeout: 10))
        input.tap(); input.typeText("followup-after-ack")
        backToRemoteHome(app)
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "调整侧栏和项目导航")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5)); row.tap()
        input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        XCTAssertEqual(input.value as? String, "followup-after-ack")
        capture(app, "draft-followup-reopened-from-history")
        app.terminate(); app = launchDraftFixture(reset: false)
        input = openDraft(app)
        XCTAssertFalse((input.value as? String ?? "").contains("followup-after-ack"))
    }
    func testTimedOutSendRetainsOriginalPayloadThroughRelaunchAndRetry() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else { throw XCTSkip("Requires scripts/remote-draft-fixture.py on loopback") }
        var app = launchDraftFixture()
        var input = openDraft(app); input.tap(); input.typeText("fixture-timeout")
        app.buttons["remote-send"].tap()
        XCTAssertTrue(app.buttons["重试确认发送结果"].waitForExistence(timeout: 10))
        app.terminate(); app = launchDraftFixture(reset: false)
        input = openDraft(app)
        XCTAssertTrue(app.buttons["重试确认发送结果"].waitForExistence(timeout: 5))
        input.tap(); input.typeText("-edited-but-not-sent")
        let editedDraft = try XCTUnwrap(input.value as? String)
        XCTAssertTrue(editedDraft.contains("fixture-timeout"))
        XCTAssertTrue(editedDraft.contains("-edited-but-not-sent"))
        app.buttons["重试确认发送结果"].tap()
        XCTAssertTrue(app.staticTexts["DRAFT_FIXTURE_OK: fixture-timeout"].waitForExistence(timeout: 10))
        XCTAssertEqual(input.value as? String, editedDraft, "Receipt confirmation must preserve exactly the edited draft, wherever the insertion caret was.")
        XCTAssertFalse(app.buttons["重试确认发送结果"].exists)
        capture(app, "draft-timeout-recovered-with-original-payload")
    }
    func testRecoveredReceiptShowsSavedInputWithoutClaimingCompletion() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else { throw XCTSkip("Requires scripts/remote-draft-fixture.py on loopback") }
        var app = launchDraftFixture()
        var input = openDraft(app); input.tap(); input.typeText("fixture-recovered")
        app.buttons["remote-send"].tap()
        XCTAssertTrue(app.buttons["重试确认发送结果"].waitForExistence(timeout: 10))
        app.terminate(); app = launchDraftFixture(reset: false); input = openDraft(app)
        app.buttons["重试确认发送结果"].tap()
        XCTAssertTrue(app.staticTexts["remote-recovered-send"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["remote-recovered-send"].label.contains("不代表执行已完成"))
        XCTAssertFalse(app.buttons["重试确认发送结果"].exists)
        XCTAssertFalse((input.value as? String ?? "").contains("fixture-recovered"))
        capture(app, "draft-saved-input-recovered")
    }
    func testUnconfirmedInstructionCanBeArchivedAfterExplicitReviewWithoutResending() throws {
        guard ProcessInfo.processInfo.environment["POTATO_IOS_DRAFT_UI"] == "1" else { throw XCTSkip("Requires scripts/remote-draft-fixture.py on loopback") }
        var app = launchDraftFixture()
        var input = openDraft(app); input.tap(); input.typeText("fixture-uncertain")
        app.buttons["remote-send"].tap()
        XCTAssertTrue(app.buttons["remote-review-pending"].waitForExistence(timeout: 10))
        app.buttons["remote-review-pending"].tap()
        let finish = app.buttons["remote-archive-unconfirmed"]
        XCTAssertTrue(finish.waitForExistence(timeout: 5)); XCTAssertFalse(finish.isEnabled)
        let operation = (app.staticTexts["remote-unconfirmed-text"].value as? String)
        app.buttons["取消"].tap()
        XCTAssertTrue(app.buttons["重试确认发送结果"].exists)
        app.buttons["remote-review-pending"].tap()
        let understood = app.switches["remote-understand-unconfirmed"]
        for _ in 0..<5 { if understood.isHittable { break }; app.swipeUp() }
        // SwiftUI exposes the whole labelled row as the switch's AX frame.
        // Tap the visible switch at the trailing edge, not the label's center.
        understood.coordinate(withNormalizedOffset: CGVector(dx: 0.94, dy: 0.5)).tap()
        let enabled = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true"), object: finish)
        XCTAssertEqual(XCTWaiter.wait(for: [enabled], timeout: 3), .completed)
        for _ in 0..<5 { if finish.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(finish.isEnabled); capture(app, "draft-unconfirmed-review"); finish.tap()
        XCTAssertTrue(app.buttons["remote-unconfirmed-records"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["重试确认发送结果"].exists)
        XCTAssertFalse((input.value as? String ?? "").contains("fixture-uncertain"))
        app.terminate(); app = launchDraftFixture(reset: false); input = openDraft(app)
        app.buttons["remote-unconfirmed-records"].tap()
        XCTAssertEqual((app.staticTexts["remote-unconfirmed-text"].value as? String), operation)
        XCTAssertEqual(app.staticTexts["remote-unconfirmed-text"].label, "fixture-uncertain")
        capture(app, "draft-unconfirmed-record-after-relaunch")
        app.buttons["关闭"].tap(); backToRemoteHome(app)
        _ = openDraft(app, second: true)
        XCTAssertFalse(app.buttons["remote-unconfirmed-records"].exists)
    }
    func testSidebarOpensWithHorizontalDrag() throws {
        let app = launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.025, dy: 0.50))
        start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.78, dy: 0.50)))
        let close = app.buttons["close-sidebar"]
        let opened = close.waitForExistence(timeout: 3) && close.isHittable
        capture(app, "audit-horizontal-open")
        XCTAssertTrue(opened, "A horizontal drag from the root page must open the sidebar.")
    }
    func testSidebarClosesWithHorizontalDrag() throws {
        let app = launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        app.buttons["remote-sidebar"].tap()
        let close = app.buttons["close-sidebar"]
        XCTAssertTrue(close.waitForExistence(timeout: 3))
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.64, dy: 0.55))
        start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.05, dy: 0.55)))
        let closed = close.waitForNonExistence(timeout: 3)
        capture(app, "audit-horizontal-close")
        XCTAssertTrue(closed, "A horizontal closing drag must dismiss the sidebar, not require a tap.")
    }
    func testVerticalScrollDoesNotOpenSidebar() throws {
        let app = launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.50, dy: 0.70))
        start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.51, dy: 0.30)))
        capture(app, "audit-vertical-scroll-control")
        XCTAssertFalse(app.buttons["close-sidebar"].exists)
        XCTAssertTrue(app.buttons["remote-sidebar"].isHittable)
    }
    func testRemoteNavigationSearchAndDrawer() {
        let app = launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        capture(app, "remote-home")
        app.buttons["remote-sidebar"].tap()
        XCTAssertTrue(app.buttons["sidebar-remote"].waitForExistence(timeout: 3))
        capture(app, "remote-sidebar")
        app.buttons["sidebar-remote"].tap()
        XCTAssertTrue(app.textFields["remote-search"].waitForExistence(timeout: 3))
        app.textFields["remote-search"].tap(); app.textFields["remote-search"].typeText("Potato")
        XCTAssertTrue(app.buttons["在Potato新建任务"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["在dynamo新建任务"].exists)
        capture(app, "remote-search")
        app.buttons["清除搜索"].tap()
        XCTAssertTrue(app.buttons["在dynamo新建任务"].waitForExistence(timeout: 3))
    }
    func testEmptyRemotePairingValidationAndLocalChatReturn() {
        let app = launch(preview: false)
        XCTAssertTrue(app.buttons["history"].waitForExistence(timeout: 10)); app.buttons["history"].tap()
        app.buttons["sidebar-remote"].tap()
        XCTAssertTrue(app.buttons["remote-pair-empty"].waitForExistence(timeout: 3))
        capture(app, "remote-empty")
        app.buttons["remote-pair-empty"].tap()
        let input = app.secureTextFields["remote-pairing-code"]
        XCTAssertTrue(input.waitForExistence(timeout: 3)); input.tap(); input.typeText("invalid-code")
        app.buttons["remote-pair-submit"].tap()
        XCTAssertTrue(app.staticTexts["请粘贴电脑生成的完整配对码。"].waitForExistence(timeout: 3))
        app.buttons["取消"].tap(); app.buttons["remote-sidebar"].tap(); app.buttons["sidebar-new-chat"].tap()
        XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 3))
    }

    func testNativeRemoteRoundTrip() throws {
        let account = ProcessInfo.processInfo.environment["POTATO_REMOTE_ACCOUNT"] == "1"
        let pairing = ProcessInfo.processInfo.environment["POTATO_REMOTE_PAIRING_CODE"]
        guard account || pairing != nil else { throw XCTSkip("Opt-in native bridge fixture") }
        let reuse = ProcessInfo.processInfo.environment["POTATO_REMOTE_ACCOUNT_REUSE"] == "1"
        let app = launch(preview: false, reset: !reuse)
        XCTAssertTrue(app.buttons["history"].waitForExistence(timeout: 10)); app.buttons["history"].tap(); app.buttons["sidebar-remote"].tap()
        if account && !reuse {
            app.buttons["remote-sign-in"].tap()
            app.buttons["继续登录"].tap()
            // A human/browser verifies the displayed code using the real IdP.
            app.activate()
            XCTAssertTrue(app.buttons["我已登录，刷新"].waitForExistence(timeout: 15))
            let deadline = Date().addingTimeInterval(240)
            while app.buttons["我已登录，刷新"].exists && Date() < deadline {
                app.buttons["我已登录，刷新"].tap()
                RunLoop.current.run(until: Date().addingTimeInterval(2))
            }
        } else if !account {
            app.buttons["remote-pair-empty"].tap()
            let code = app.secureTextFields["remote-pairing-code"]; XCTAssertTrue(code.waitForExistence(timeout: 3)); code.tap(); code.typeText(pairing!); app.buttons["remote-pair-submit"].tap()
        }
        let newTask = app.buttons["remote-new-task"]
        XCTAssertTrue(newTask.waitForExistence(timeout: 15)); expectation(for: NSPredicate(format: "enabled == true"), evaluatedWith: newTask); waitForExpectations(timeout: 15)
        capture(app, "remote-real-connected"); newTask.tap()
        func send(_ text: String) {
            let input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
            XCTAssertTrue(input.waitForExistence(timeout: 5)); input.tap(); input.typeText(text); app.buttons["remote-send"].tap()
        }
        func reveal(_ element: XCUIElement, timeout: TimeInterval = 15) {
            if element.waitForExistence(timeout: timeout) && element.isHittable { return }
            for _ in 0..<5 { app.scrollViews["remote-conversation"].swipeUp(); if element.exists && element.isHittable { return } }
        }
        send("remote hello")
        let initial = app.staticTexts["POTATO_REMOTE_OK"]; XCTAssertTrue(initial.waitForExistence(timeout: 20)); capture(app, "remote-real-response")
        send("remote continue")
        let continued = app.staticTexts["POTATO_CONTINUED_OK"]; reveal(continued); XCTAssertTrue(continued.exists)
        send("remote question")
        let option = app.buttons["全部"].firstMatch; reveal(option); XCTAssertTrue(option.exists); option.tap(); app.buttons["提交回答"].tap()
        let answered = app.staticTexts["POTATO_QUESTION_OK"]; reveal(answered); XCTAssertTrue(answered.exists)
        send("remote approval")
        let allow = app.buttons["允许这一次"]; reveal(allow); XCTAssertTrue(allow.exists); capture(app, "remote-real-approval"); allow.tap()
        let approved = app.staticTexts["POTATO_APPROVAL_OK"]; reveal(approved); XCTAssertTrue(approved.exists); capture(app, "remote-real-approved")
        send("remote wait")
        let stop = app.buttons["停止任务"]; XCTAssertTrue(stop.waitForExistence(timeout: 10)); stop.tap()
        let confirm = app.buttons.matching(NSPredicate(format: "label == %@", "停止任务")).allElementsBoundByIndex.last!; confirm.tap()
        expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: app.buttons["停止任务"]); waitForExpectations(timeout: 15)
        XCTAssertTrue(app.staticTexts["本轮任务已停止"].waitForExistence(timeout: 15))
        capture(app, "remote-real-stopped")
    }

    func testLogoutLiveAccountFixture() throws {
        guard ProcessInfo.processInfo.environment["POTATO_REMOTE_ACCOUNT_CLEANUP"] == "1" else { throw XCTSkip("Opt-in live fixture cleanup") }
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        XCTAssertTrue(app.buttons["history"].waitForExistence(timeout: 10))
        app.buttons["history"].tap(); app.buttons["sidebar-remote"].tap()
        // Run after the disposable host logs out: account discovery must remove it.
        XCTAssertTrue(app.staticTexts["在电脑 Potato 登录同一账号，再开启远程访问。"].waitForExistence(timeout: 20))
        app.buttons["remote-options"].tap(); app.buttons["管理电脑"].tap()
        let logout = app.buttons["退出这台 iPhone 的登录"]
        XCTAssertTrue(logout.waitForExistence(timeout: 10)); logout.tap()
        expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: logout)
        waitForExpectations(timeout: 15)
        capture(app, "remote-live-account-logged-out")
    }
    func testRemoteReplyPresentation() throws {
        guard ProcessInfo.processInfo.environment["POTATO_REMOTE_STYLE_AUDIT"] == "1" else { throw XCTSkip("Opt-in local reply fixture") }
        for (title, marker, name) in [
            ("调整侧栏和项目导航", "已完成远程连接和账号关联。", "01-remote-reply-text"),
            ("检查 iPhone 远程控制的实现", "检查完成，配置正常。", "02-remote-reply-process"),
            ("查看今天的运行日志", "示例代码：", "03-remote-reply-markdown")
        ] {
            let app = launch()
            let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", title)).firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 10)); row.tap()
            XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", marker)).firstMatch.waitForExistence(timeout: 10))
            XCTAssertFalse(app.scrollViews["remote-conversation"].staticTexts["你"].exists)
            XCTAssertFalse(app.scrollViews["remote-conversation"].staticTexts["Potato"].exists)
            capture(app, name)
            if name == "02-remote-reply-process" {
                let reasoning = app.staticTexts["我先查看项目目录，再读取配置文件，最后整理检查结果。"]
                XCTAssertFalse(reasoning.exists)
                app.buttons["remote-process-toggle"].tap()
                let reasoningStep = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS %@", "activity-step-", "思考")).firstMatch
                XCTAssertTrue(reasoningStep.waitForExistence(timeout: 3)); reasoningStep.tap()
                XCTAssertTrue(app.staticTexts["activity-reasoning"].waitForExistence(timeout: 3))
                app.buttons["activity-back"].tap()
                let tool = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS %@", "activity-step-", "读取文件")).firstMatch
                tool.tap(); capture(app, "04-remote-reply-tool-detail")
                app.buttons["activity-back"].tap(); app.buttons["activity-close"].tap()
                XCTAssertFalse(reasoning.exists)
            }
            if name == "01-remote-reply-text" {
                let bubble = app.staticTexts["remote-user-u"]
                XCTAssertTrue(bubble.exists)
                XCTAssertGreaterThan(bubble.frame.midX, app.frame.midX)
                app.buttons["remote-copy-reply"].tap()
                XCTAssertEqual(app.buttons["remote-copy-reply"].label, "已复制回复")
                app.buttons["remote-reply-more"].tap()
                app.buttons["remote-select-reply"].tap()
                let selection = app.textViews["selectable-reply"]
                XCTAssertTrue(selection.waitForExistence(timeout: 3))
                XCTAssertTrue((selection.value as? String ?? "").contains(marker))
                capture(app, "05-remote-reply-selection")
                app.buttons["close-text-selection"].tap()
                app.buttons["remote-reply-more"].tap()
                app.buttons["remote-share-reply"].tap()
                XCTAssertTrue(app.otherElements["ActivityListView"].waitForExistence(timeout: 5) || app.buttons["拷贝"].exists)
                capture(app, "06-remote-reply-share")
            }
        }
    }

}

final class RemoteDirectoryUITests: XCTestCase {
    private func launch(reset: Bool, flags: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--remote-preview", "--remote-directory-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"] + flags
        if reset { app.launchArguments.append("--reset") }
        app.launch()
        XCTAssertTrue(app.buttons["remote-sidebar"].waitForExistence(timeout: 10))
        return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    private func waitOnline(_ app: XCUIApplication) {
        let chip = app.buttons["remote-device-directory-mac"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", "在线"), object: chip)], timeout: 15), .completed)
        XCTAssertTrue(app.buttons["remote-chat-directory-mac-cached-chat-1"].waitForExistence(timeout: 5))
    }
    func testColdLoadNeverShowsSetupOrFalseOffline() {
        let app = launch(reset: true, flags: ["--remote-directory-slow"])
        XCTAssertTrue(app.descendants(matching: .any)["remote-directory-loading"].waitForExistence(timeout: 3))
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "离线")).firstMatch.exists)
        XCTAssertFalse(app.buttons["remote-new-task"].isEnabled)
        capture(app, "01-first-load")
        waitOnline(app)
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        capture(app, "02-connected-list")
    }
    func testRelaunchShowsCachedListBeforeGreenStatusWithoutLayoutJump() {
        var app = launch(reset: true); waitOnline(app); app.terminate()
        app = launch(reset: false, flags: ["--remote-directory-slow"])
        let row = app.buttons["remote-chat-directory-mac-cached-chat-1"], chip = app.buttons["remote-device-directory-mac"]
        XCTAssertTrue(row.waitForExistence(timeout: 3))
        XCTAssertNotEqual(chip.value as? String, "在线")
        XCTAssertNotEqual(chip.value as? String, "离线")
        XCTAssertFalse(app.staticTexts["remote-directory-status"].exists)
        XCTAssertFalse(app.staticTexts["正在更新…"].exists)
        XCTAssertFalse(app.buttons["remote-new-task"].isEnabled)
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        let origin = row.frame.minY
        capture(app, "03-relaunch-cached-connecting")
        waitOnline(app)
        XCTAssertEqual(row.frame.minY, origin, accuracy: 1)
        XCTAssertTrue(app.buttons["remote-new-task"].isEnabled)
        capture(app, "04-relaunch-connected")
        app.textFields["remote-search"].tap(); app.textFields["remote-search"].typeText("桌面")
        XCTAssertTrue(row.exists); XCTAssertFalse(app.buttons["remote-chat-directory-mac-cached-chat-2"].exists)
    }
    func testOfflineAndNetworkFailureKeepCachedSessions() {
        var app = launch(reset: true); waitOnline(app); app.terminate()
        app = launch(reset: false, flags: ["--remote-directory-offline"])
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", "离线"), object: app.buttons["remote-device-directory-mac"])], timeout: 8), .completed)
        XCTAssertTrue(app.buttons["remote-chat-directory-mac-cached-chat-1"].exists)
        XCTAssertFalse(app.buttons["remote-new-task"].isEnabled)
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        capture(app, "05-offline-keeps-list")
        app.terminate(); app = launch(reset: false, flags: ["--remote-directory-failure"])
        XCTAssertTrue(app.buttons["remote-chat-directory-mac-cached-chat-1"].waitForExistence(timeout: 3))
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", "连接未确认"), object: app.buttons["remote-device-directory-mac"])], timeout: 8), .completed)
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        capture(app, "06-network-error-keeps-list")
    }
    func testSetupAppearsOnlyAfterConfirmedEmptyDirectory() {
        let app = launch(reset: true, flags: ["--remote-directory-slow", "--remote-directory-empty"])
        XCTAssertFalse(app.buttons["remote-pair-empty"].exists)
        XCTAssertTrue(app.buttons["remote-pair-empty"].waitForExistence(timeout: 12))
        XCTAssertFalse(app.descendants(matching: .any)["remote-directory-loading"].exists)
        capture(app, "07-confirmed-empty")
    }
}
