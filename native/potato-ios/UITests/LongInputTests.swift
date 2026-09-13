import XCTest

final class LongInputTests: XCTestCase {
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    func testLongInputAudit() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--voice-preview", "--voice-long-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 10)); input.tap()
        let introduction = "请帮我整理这段需求。"
        input.typeText(introduction)
        let shortHeight = input.frame.height
        capture(app, "50-short-input")
        let paragraph = "这是一份用于测试长文本输入的合成需求。希望保留已有草稿和附件，能够回到前文修改，输入过程中始终看见当前光标，点击发送前可以检查完整内容。\n"
        let text = String((introduction + String(repeating: paragraph, count: 20)).prefix(1000))
        input.typeText(String(text.dropFirst(introduction.count).prefix(500 - introduction.count)))
        XCTAssertGreaterThan(input.frame.height, shortHeight)
        XCTAssertEqual(input.value as? String, String(text.prefix(500)))
        let mediumHeight = input.frame.height
        XCTAssertTrue(app.buttons["send-message"].isHittable); capture(app, "51-input-500")
        input.typeText(String(text.dropFirst(500)))
        XCTAssertEqual(input.value as? String, text)
        let metrics = XCTAttachment(string: "shortHeight=\(shortHeight), height500=\(mediumHeight), height1000=\(input.frame.height)")
        metrics.name = "long-input-heights"; metrics.lifetime = .keepAlways; add(metrics)
        XCTAssertTrue(app.buttons["send-message"].isHittable); capture(app, "52-input-1000")
        XCTAssertTrue(app.buttons["expand-input"].isHittable)
        app.buttons["expand-input"].tap()
        let expanded = app.textViews["expanded-input"]
        XCTAssertTrue(expanded.waitForExistence(timeout: 5)); XCTAssertEqual(expanded.value as? String, text)
        XCTAssertGreaterThan(expanded.frame.height, mediumHeight)
        expanded.typeText("补充内容")
        capture(app, "54-expanded-input")
        app.buttons["collapse-input"].tap()
        XCTAssertTrue(input.waitForExistence(timeout: 5)); XCTAssertEqual(input.value as? String, text + "补充内容")
        app.buttons["voice-input"].tap()
        XCTAssertTrue(app.textViews["voice-transcript"].waitForExistence(timeout: 5))
        let finalWords = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value CONTAINS %@", "先把项目方案整理好，下午再和团队讨论一下。"), object: app.textViews["voice-transcript"])
        XCTAssertEqual(XCTWaiter.wait(for: [finalWords], timeout: 8), .completed)
        capture(app, "53-voice-long-draft")
        app.textViews["voice-transcript"].swipeDown()
        XCTAssertTrue(app.buttons["follow-voice"].waitForExistence(timeout: 3)); capture(app, "55-voice-review")
        app.buttons["follow-voice"].tap()
        XCTAssertFalse(app.buttons["follow-voice"].exists); capture(app, "56-voice-latest")
        app.buttons["cancel-voice"].tap()
        XCTAssertEqual(input.value as? String, text + "补充内容")
        app.terminate(); app.launchArguments.removeAll { $0 == "--reset" || $0 == "--voice-preview" }; app.launch()
        XCTAssertTrue(input.waitForExistence(timeout: 5)); XCTAssertEqual(input.value as? String, text + "补充内容")
        app.buttons["expand-input"].tap()
        XCTAssertTrue(app.buttons["send-expanded-input"].waitForExistence(timeout: 5)); app.buttons["send-expanded-input"].tap()
        XCTAssertTrue(input.waitForExistence(timeout: 5)); XCTAssertEqual(input.value as? String, "")
        capture(app, "57-expanded-sent")
    }
    func testInputShrinksAfterDeletingLines() {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--reset", "--voice-preview", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        app.launch()
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 10)); input.tap()
        let originalHeight = input.frame.height
        let lines = String(repeating: "一二三四五六七八九十\n", count: 8)
        input.typeText(lines); XCTAssertGreaterThan(input.frame.height, originalHeight * 2)
        input.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: lines.count))
        XCTAssertEqual(input.value as? String, ""); XCTAssertLessThanOrEqual(input.frame.height, originalHeight + 1)
        capture(app, "58-input-shrunk")
    }
}
