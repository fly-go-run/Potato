import XCTest

final class RemoteQueueUITests: XCTestCase {
    func testContinuousQueueEditingDeletionAndInterrupt() throws {
        var reset = URLRequest(url: URL(string: "http://127.0.0.1:19017/fixture/reset")!)
        reset.timeoutInterval = 2
        reset.httpMethod = "POST"; reset.httpBody = Data("{}".utf8)
        let resetDone = expectation(description: "Reset isolated fixture")
        var available = false
        URLSession.shared.dataTask(with: reset) { _, response, error in available = error == nil && (response as? HTTPURLResponse)?.statusCode == 200; resetDone.fulfill() }.resume()
        wait(for: [resetDone], timeout: 5)
        guard available else { throw XCTSkip("Requires remote-queue-fixture.py on loopback") }
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--remote-preview", "--remote-queue-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "调整侧栏和项目导航")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10)); row.tap()
        let input = app.descendants(matching: .any).matching(identifier: "remote-prompt").firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        let send = app.buttons["remote-send"]
        for text in ["First task", "Second task", "Third task", "Fourth task"] {
            input.tap(); input.typeText(text)
            XCTAssertTrue(send.waitForExistence(timeout: 3))
            let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true"), object: send)
            XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 5), .completed)
            send.tap()
            XCTAssertTrue(app.staticTexts[text].waitForExistence(timeout: 5))
        }
        let transcript = app.scrollViews["remote-conversation"]
        XCTAssertFalse(app.buttons["remote-queue-expand"].exists)
        XCTAssertFalse(app.buttons["remote-send-options"].exists)
        for text in ["First task", "Second task", "Third task", "Fourth task"] {
            XCTAssertTrue(transcript.staticTexts[text].exists)
        }
        transcript.swipeDown()
        capture(app, "queue-inline-messages")
        let firstMenu = app.buttons.matching(NSPredicate(format: "label == %@", "排队中，First task")).firstMatch
        if !firstMenu.isHittable { transcript.swipeDown() }
        firstMenu.tap(); app.buttons["编辑消息"].tap()
        let editor = app.textViews["remote-queue-editor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 3)); editor.tap(); editor.typeText(" updated")
        let edited = try XCTUnwrap(editor.value as? String)
        XCTAssertTrue(edited.contains("updated")); XCTAssertTrue(edited.contains("First task"))
        app.buttons["保存"].tap()
        XCTAssertTrue(app.staticTexts[edited].waitForExistence(timeout: 5))
        app.buttons.matching(NSPredicate(format: "label == %@", "排队中，Second task")).firstMatch.tap()
        app.buttons["删除"].tap()
        XCTAssertTrue(app.staticTexts["Second task"].waitForNonExistence(timeout: 5))
        input.tap(); input.typeText("Urgent correction")
        send.tap()
        let urgent = app.buttons.matching(NSPredicate(format: "label == %@", "排队中，Urgent correction")).firstMatch
        XCTAssertTrue(urgent.waitForExistence(timeout: 5))
        urgent.tap()
        XCTAssertTrue(app.buttons["打断并发送"].waitForExistence(timeout: 3))
        capture(app, "queue-inline-actions")
        app.buttons["打断并发送"].tap()
        let interrupting = app.buttons.matching(NSPredicate(format: "label == %@", "正在打断，随后发送…，Urgent correction")).firstMatch
        XCTAssertTrue(interrupting.waitForExistence(timeout: 5))
        XCTAssertTrue(transcript.staticTexts["Urgent correction"].exists)
        XCTAssertTrue(transcript.staticTexts["Third task"].exists)
        capture(app, "queue-inline-interrupting")

        var complete = URLRequest(url: URL(string: "http://127.0.0.1:19017/fixture/complete-next")!)
        complete.httpMethod = "POST"; complete.httpBody = Data("{}".utf8)
        let completed = expectation(description: "Queued message becomes history")
        URLSession.shared.dataTask(with: complete) { _, _, _ in completed.fulfill() }.resume()
        wait(for: [completed], timeout: 5)
        XCTAssertTrue(interrupting.waitForNonExistence(timeout: 10))
        XCTAssertEqual(transcript.staticTexts.matching(NSPredicate(format: "label == %@", "Urgent correction")).count, 1)
        XCTAssertTrue(transcript.staticTexts["Third task"].exists)

    }
    func testMultilineQueuedMessagesInConversation() throws {
        var seed = URLRequest(url: URL(string: "http://127.0.0.1:19017/fixture/preview")!)
        seed.timeoutInterval = 2; seed.httpMethod = "POST"; seed.httpBody = Data("{}".utf8)
        let seeded = expectation(description: "Seed multiline preview")
        var available = false
        URLSession.shared.dataTask(with: seed) { _, response, error in
            available = error == nil && (response as? HTTPURLResponse)?.statusCode == 200; seeded.fulfill()
        }.resume()
        wait(for: [seeded], timeout: 5)
        guard available else { throw XCTSkip("Requires remote-queue-fixture.py on loopback") }
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--remote-preview", "--remote-queue-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "调整侧栏和项目导航")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10)); row.tap()
        let transcript = app.scrollViews["remote-conversation"]
        XCTAssertTrue(transcript.staticTexts["最后再检查一下手机端。"].waitForExistence(timeout: 5))
        XCTAssertTrue(transcript.staticTexts["我认为这里应该有缓存。重新打开软件时，先显示上次的项目和对话，然后在后台更新。"].exists)
        capture(app, "queue-inline-chinese")
        app.buttons["remote-queue-item-preview-1"].tap()
        XCTAssertTrue(app.buttons["打断并发送"].waitForExistence(timeout: 3))
        capture(app, "queue-inline-chinese-actions")
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let item = XCTAttachment(screenshot: app.screenshot()); item.name = name; item.lifetime = .keepAlways; add(item)
    }
}
