import XCTest

final class LibraryUITests: XCTestCase {
    private func launch(fixtures: Bool = true, reset: Bool = true, large: Bool = false) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--ui-testing", "--library-open", "-AppleLanguages", "(zh-Hans)", "-AppleLocale", "zh_CN"]
        if reset { app.launchArguments.append("--reset") }
        if fixtures { app.launchArguments.append("--library-fixtures") }
        if large { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL", "--reduce-transparency-preview"] }
        app.launch(); XCTAssertTrue(app.buttons["library-add"].waitForExistence(timeout: 10)); return app
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = name; shot.lifetime = .keepAlways; add(shot)
    }
    func testGroupedLibrarySearchPreviewReuseAndRestart() {
        let app = launch()
        XCTAssertTrue(app.buttons["library-file-推理优化报告.pdf"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["library-file-Potato 图标.png"].exists)
        capture(app, "library-01-grouped")
        app.buttons["library-category-图片"].tap()
        XCTAssertFalse(app.buttons["library-file-推理优化报告.pdf"].exists)
        app.buttons["library-file-Potato 图标.png"].tap()
        XCTAssertTrue(app.staticTexts["library-detail-title"].waitForExistence(timeout: 5)); capture(app, "library-02-image")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["library-category-文档"].tap()
        app.buttons["library-file-推理优化报告.pdf"].tap()
        XCTAssertTrue(app.buttons["library-use"].waitForExistence(timeout: 5)); capture(app, "library-09-pdf")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let search = app.textFields["library-search"]; search.tap(); search.typeText("独立留存")
        XCTAssertTrue(app.buttons["library-file-项目说明.txt"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["library-file-推理优化报告.pdf"].exists)
        capture(app, "library-03-search")
        app.buttons["library-file-项目说明.txt"].tap()
        XCTAssertTrue(app.buttons["library-use"].waitForExistence(timeout: 5)); capture(app, "library-04-document")
        app.buttons["library-use"].tap()
        XCTAssertTrue(app.textViews["composer-input"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.textViews["composer-input"].value as? String, "保留的草稿")
        XCTAssertTrue(app.buttons["预览 项目说明.txt"].exists)
        capture(app, "library-05-composer")
        app.terminate()
        let reopened = launch(reset: false)
        XCTAssertTrue(reopened.buttons["library-file-项目说明.txt"].exists)
    }
    func testCreateTextRenameDeleteRestoreAndHistoryAccess() {
        let app = launch(fixtures: false)
        XCTAssertTrue(app.staticTexts["还没有资料"].waitForExistence(timeout: 5)); capture(app, "library-08-empty")
        app.buttons["library-add"].tap(); app.buttons["粘贴文本"].tap()
        XCTAssertTrue(app.textFields["library-text-name"].waitForExistence(timeout: 5))
        app.textFields["library-text-name"].tap(); app.textFields["library-text-name"].typeText("测试笔记")
        app.textViews["library-text-content"].tap(); app.textViews["library-text-content"].typeText("这是一份可复用的资料")
        app.buttons["library-text-save"].tap()
        XCTAssertTrue(app.buttons["library-detail-more"].waitForExistence(timeout: 5))
        app.buttons["library-detail-more"].tap(); app.buttons["重命名"].tap()
        let name = app.alerts.textFields.firstMatch; XCTAssertTrue(name.waitForExistence(timeout: 5)); name.tap()
        name.press(forDuration: 1.2)
        if app.menuItems["全选"].exists { app.menuItems["全选"].tap(); name.typeText("改名笔记.md") }
        else { name.typeText("改名") }
        app.alerts.buttons["保存"].tap()
        app.buttons["library-detail-more"].tap(); app.buttons["移到最近删除"].tap()
        XCTAssertTrue(app.buttons["library-more"].waitForExistence(timeout: 5))
        app.buttons["library-more"].tap(); app.buttons["最近删除"].tap()
        let item = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "library-file-")).firstMatch
        XCTAssertTrue(item.waitForExistence(timeout: 5)); item.tap()
        app.buttons["恢复资料"].tap()
        app.buttons["library-sidebar"].tap()
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "library-file-")).firstMatch.waitForExistence(timeout: 5))
        app.buttons["library-sidebar"].tap()
        app.buttons["sidebar-history-manage"].tap()
        XCTAssertTrue(app.navigationBars["对话"].waitForExistence(timeout: 5))
    }
    func testLargeTypeAndReducedTransparencyRemainNavigable() {
        let app = launch(large: true)
        capture(app, "library-06-large-type")
        app.buttons["library-category-图片"].tap()
        let item = app.buttons["library-file-Potato 图标.png"]
        XCTAssertTrue(item.waitForExistence(timeout: 5)); item.tap()
        XCTAssertTrue(app.buttons["library-use"].waitForExistence(timeout: 5)); capture(app, "library-07-large-detail")
    }
    func testImportMenusAndShareSheet() {
        let app = launch()
        app.buttons["library-add"].tap(); app.buttons["从文件导入"].tap()
        let cancel = app.buttons["取消"].firstMatch
        let englishCancel = app.buttons["Cancel"].firstMatch
        if cancel.waitForExistence(timeout: 5) { cancel.tap() }
        else { XCTAssertTrue(englishCancel.waitForExistence(timeout: 5)); englishCancel.tap() }
        app.buttons["library-add"].tap(); app.buttons["从照片选择"].tap()
        if cancel.waitForExistence(timeout: 5) { cancel.tap() }
        else { XCTAssertTrue(englishCancel.waitForExistence(timeout: 5)); englishCancel.tap() }
        app.buttons["library-file-项目说明.txt"].tap()
        XCTAssertTrue(app.buttons["library-detail-more"].waitForExistence(timeout: 5))
        app.buttons["library-detail-more"].tap(); app.buttons["分享"].tap()
        XCTAssertTrue(app.otherElements["ActivityListView"].waitForExistence(timeout: 5))
        capture(app, "library-10-share")
    }
    func testReplyCanBeSavedAndLibraryCanOpenFromComposer() {
        let app = launch(fixtures: false)
        app.buttons["library-sidebar"].tap(); app.buttons["周末计划"].tap()
        XCTAssertTrue(app.buttons["close-document"].waitForExistence(timeout: 5)); app.buttons["close-document"].tap()
        app.buttons["reply-more"].tap()
        XCTAssertTrue(app.buttons["save-reply-library"].waitForExistence(timeout: 5)); app.buttons["save-reply-library"].tap()
        XCTAssertTrue(app.alerts["已保存到资料库"].waitForExistence(timeout: 5)); app.alerts.buttons["好"].tap()
        app.buttons["add-attachment"].tap(); app.buttons["从资料库选择"].tap()
        XCTAssertTrue(app.buttons["library-add"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "library-file-")).firstMatch.waitForExistence(timeout: 5))
    }
    func testMultiSelectionDeleteAndRestore() {
        let app = launch()
        app.buttons["library-more"].tap(); app.buttons["选择资料"].tap()
        app.buttons["library-file-项目说明.txt"].tap(); app.buttons["library-file-产品设计笔记.md"].tap()
        XCTAssertTrue(app.staticTexts["已选择 2 项"].exists)
        app.buttons["删除所选资料"].tap()
        XCTAssertFalse(app.buttons["library-file-项目说明.txt"].exists)
        app.buttons["library-selection-done"].tap()
        app.buttons["library-more"].tap(); app.buttons["最近删除"].tap()
        XCTAssertTrue(app.buttons["library-file-项目说明.txt"].exists)
    }
}
