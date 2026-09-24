import XCTest

// Opt-in scheme only. Uses the provisioned connection; photo and sandbox requests need explicit flags.
final class LiveConnectionTests: XCTestCase {
    func testAutomaticExaSearchAndSourcePersistence() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SEARCH"] == "1")
        let app = XCUIApplication(); app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Please look up the current official Exa Search API documentation. Briefly explain numResults and link to the official source. Do not answer from memory.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 120))
        XCTAssertTrue(app.buttons["search-sources"].waitForExistence(timeout: 10))
        let reply = XCTAttachment(screenshot: app.screenshot()); reply.name = "36-auto-search-reply"; reply.lifetime = .keepAlways; add(reply)
        let sources = app.buttons["search-sources"]
        for _ in 0..<5 { if sources.isHittable { break }; app.swipeUp() }
        sources.tap(); XCTAssertTrue(app.buttons["search-source-link"].firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[cd] %@", "exa.ai")).firstMatch.exists)
        let panel = XCTAttachment(screenshot: app.screenshot()); panel.name = "37-auto-search-sources"; panel.lifetime = .keepAlways; add(panel)
        app.buttons["close-search-sources"].tap(); app.terminate(); app.launch()
        XCTAssertTrue(app.buttons["search-sources"].waitForExistence(timeout: 10))
    }

    private func startLiveDictation(_ app: XCUIApplication) {
        app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN", "--live-voice-fixture"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        app.buttons["voice-input"].tap()
        XCTAssertFalse(app.buttons["start-voice"].exists)
        let transcript = app.textViews["voice-transcript"]
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "value CONTAINS %@", "工作计划"), object: transcript)], timeout: 30), .completed)
    }
    func testDoubaoInlineEditKeepsUnsentDraft() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SPEECH"] == "1")
        let app = XCUIApplication(); startLiveDictation(app)
        let recording = XCTAttachment(screenshot: app.screenshot()); recording.name = "45-live-inline-recording"; recording.lifetime = .keepAlways; add(recording)
        app.textViews["voice-transcript"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 15))
        XCTAssertTrue((app.textViews["composer-input"].value as? String ?? "").contains("工作计划"))
        XCTAssertFalse(app.buttons["retry-message"].exists)
        let draft = XCTAttachment(screenshot: app.screenshot()); draft.name = "46-live-inline-edit"; draft.lifetime = .keepAlways; add(draft)
    }
    func testDoubaoInlineSendWaitsForFinal() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SPEECH"] == "1")
        let app = XCUIApplication(); startLiveDictation(app)
        app.buttons["send-voice"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 90))
        XCTAssertFalse(app.buttons["send-voice"].exists)
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "工作计划")).firstMatch.exists)
        XCTAssertEqual(app.textViews["composer-input"].value as? String ?? "", "")
        let result = XCTAttachment(screenshot: app.screenshot()); result.name = "47-live-inline-sent"; result.lifetime = .keepAlways; add(result)
    }

    func testCloudExecutionProducesFilesAndPersists() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SANDBOX"] == "1")
        let app = XCUIApplication(); app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Return only one fenced python block, no explanation. Use matplotlib Agg to save a small bar chart of [10,20,30] to /home/user/output/chart.png, close the figure. Use reportlab canvas to make /home/user/output/report.pdf containing the text Total: 60. Use python-docx to make /home/user/output/report.docx containing Total: 60. Use openpyxl to make /home/user/output/analysis.xlsx with rows Jan 10, Feb 20, Mar 30. The output folder already exists. Print POTATO_SANDBOX_OK and nothing else. Do not execute the code yourself.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 90))
        XCTAssertFalse(app.buttons["open-sandbox"].exists)
        input.tap(); input.typeText("Now execute that code in your Python sandbox and return the generated files and result. Use the run_python tool; do not just describe the expected result.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["activity-summary"].firstMatch.waitForExistence(timeout: 120))
        XCTAssertTrue(app.buttons["deliverable-card"].firstMatch.waitForExistence(timeout: 120))
        app.swipeUp()
        XCTAssertTrue(app.buttons["message-image-0"].exists)
        XCTAssertFalse(app.buttons["message-image-1"].exists)
        let pdf = app.buttons["预览 report.pdf"]
        XCTAssertTrue(pdf.waitForExistence(timeout: 10))
        for _ in 0..<6 { if pdf.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(app.buttons["预览 report.docx"].exists)
        XCTAssertTrue(app.buttons["预览 analysis.xlsx"].exists)
        let result = XCTAttachment(screenshot: app.screenshot()); result.name = "33-live-sandbox-files"; result.lifetime = .keepAlways; add(result)
        pdf.tap(); XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 10))
        let preview = XCTAttachment(screenshot: app.screenshot()); preview.name = "34-live-sandbox-pdf"; preview.lifetime = .keepAlways; add(preview)
        app.buttons["close-preview"].tap()
        app.terminate(); app.launch()
        XCTAssertTrue(app.buttons["预览 report.pdf"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["预览 report.docx"].exists)
        XCTAssertTrue(app.buttons["预览 analysis.xlsx"].exists)
        let restored = XCTAttachment(screenshot: app.screenshot()); restored.name = "35-live-sandbox-restored"; restored.lifetime = .keepAlways; add(restored)
    }

    func testFourSimulatorPhotosThroughWorker() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SAMPLE_PHOTO"] == "1")
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        app.buttons["add-attachment"].tap(); app.buttons["照片图库"].tap()
        let photos = app.images.matching(identifier: "PXGGridLayout-Info")
        XCTAssertTrue(photos.firstMatch.waitForExistence(timeout: 5)); XCTAssertGreaterThanOrEqual(photos.count, 4)
        for i in 0..<4 { photos.element(boundBy: i).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap() }
        app.buttons["Add"].tap()
        let pending = app.buttons.matching(NSPredicate(format: "label == %@", "预览 照片.jpg"))
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "count == 4"), object: pending)], timeout: 15), .completed)
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Briefly describe each of these four photos in English, numbered 1 to 4. One short sentence each.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 90))
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[cd] %@", "flower")).firstMatch.exists)
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[cd] %@", "waterfall")).firstMatch.exists)
        app.swipeDown()
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "31-live-four-images"; shot.lifetime = .keepAlways; add(shot)
    }

    func testPythonSnippetHasNoManualExecutionControls() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SANDBOX_SETUP"] == "1")
        let app = XCUIApplication(); app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch(); XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10)); app.buttons["new-chat"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Reply only with a fenced python code block containing print(1 + 2). Do not execute it.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.buttons["copy-code"].exists)
        XCTAssertFalse(app.buttons["open-sandbox"].exists)
        XCTAssertFalse(app.buttons["run-sandbox"].exists)
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "32-python-snippet"; shot.lifetime = .keepAlways; add(shot)
    }
    // Explicit opt-in on a disposable simulator whose first photo is Apple's flower sample.
    // Never enable this against a personal photo library.
    func testSimulatorSamplePhotoThroughConfiguredService() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["POTATO_LIVE_SAMPLE_PHOTO"] == "1")
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        XCTAssertTrue(app.buttons["new-chat"].waitForExistence(timeout: 10))
        app.buttons["new-chat"].tap()
        app.buttons["add-attachment"].tap(); app.buttons["照片图库"].tap()
        let photo = app.images.matching(identifier: "PXGGridLayout-Info").firstMatch
        XCTAssertTrue(photo.waitForExistence(timeout: 5))
        photo.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(app.buttons["Add"].waitForExistence(timeout: 3)); app.buttons["Add"].tap()
        let preview = app.buttons["预览 照片.jpg"]
        XCTAssertTrue(preview.waitForExistence(timeout: 8)); preview.tap()
        XCTAssertTrue(app.buttons["close-preview"].waitForExistence(timeout: 5))
        let source = XCTAttachment(screenshot: app.screenshot()); source.name = "23-live-photo-source"; source.lifetime = .keepAlways; add(source)
        app.buttons["close-preview"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Describe the attached photo in one short English sentence.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 90))
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[cd] %@", "flower")).firstMatch.exists)
        let reply = XCTAttachment(screenshot: app.screenshot()); reply.name = "24-live-photo-reply"; reply.lifetime = .keepAlways; add(reply)
    }

    func testConfiguredModelStreamsAndPersistsAnswer() {
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        XCTAssertTrue(app.buttons["connection-settings"].waitForExistence(timeout: 10))
        app.buttons["connection-settings"].tap()
        let connection = app.buttons["local-open-connection"]
        for _ in 0..<8 { if connection.exists && connection.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(connection.isHittable); connection.tap()
        XCTAssertEqual(app.switches["demo-mode"].value as? String, "0")
        let probe = app.buttons["test-connection"]
        XCTAssertTrue(probe.waitForExistence(timeout: 3))
        for _ in 0..<3 { if probe.isHittable { break }; app.swipeUp() }
        XCTAssertTrue(probe.isEnabled); probe.tap()
        let result = app.descendants(matching: .any).matching(identifier: "connection-result").firstMatch
        XCTAssertTrue(result.waitForExistence(timeout: 45))
        let success = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label CONTAINS %@", "已收到模型回复"), object: result)
        XCTAssertEqual(XCTWaiter.wait(for: [success], timeout: 3), .completed)
        let settings = XCTAttachment(screenshot: app.screenshot()); settings.name = "21-live-connection"; settings.lifetime = .keepAlways; add(settings)
        app.buttons["取消"].tap()
        app.buttons["new-chat"].tap()
        let input = app.descendants(matching: .any).matching(identifier: "composer-input").firstMatch
        input.tap(); input.typeText("Reply with exactly POTATO_LIVE_OK and nothing else.")
        app.buttons["send-message"].tap()
        XCTAssertTrue(app.buttons["retry-message"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.staticTexts["POTATO_LIVE_OK"].exists)
        let reply = XCTAttachment(screenshot: app.screenshot()); reply.name = "22-live-reply"; reply.lifetime = .keepAlways; add(reply)
        app.terminate(); app.launch()
        XCTAssertTrue(app.staticTexts["POTATO_LIVE_OK"].waitForExistence(timeout: 10))
    }
}
