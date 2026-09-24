import XCTest
import UIKit
@testable import PotatoMobile
import UniformTypeIdentifiers

@MainActor final class WorkspaceTests: XCTestCase {
    func storage() -> LocalStorage { LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)) }
    func testAppearanceMigrationAndPersistence() throws {
        let legacy = Data(#"{"demo":true,"endpoint":"","model":"","haptics":true,"systemPrompt":"test"}"#.utf8)
        let decoded = try JSONDecoder().decode(ConnectionSettings.self, from: legacy)
        XCTAssertEqual(decoded.appearanceMode, .automatic)
        XCTAssertEqual(decoded.appearanceMode.interfaceStyle, .unspecified)
        let disk = storage(), store = WorkspaceStore(storage: disk)
        for mode in AppAppearance.allCases {
            store.settings.appearanceMode = mode
            store.persist()
            XCTAssertEqual(WorkspaceStore(storage: disk).settings.appearanceMode, mode)
        }
    }

    func testAppearanceColorsMaintainReadableContrast() {
        func luminance(_ color: UIColor) -> Double {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            color.getRed(&r, green: &g, blue: &b, alpha: &a)
            func linear(_ value: CGFloat) -> Double {
                let v = Double(value)
                return v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        }
        for style in [UIUserInterfaceStyle.light, .dark] {
            let traits = UITraitCollection(userInterfaceStyle: style)
            for (foreground, background) in [(Palette.ink, Palette.canvas), (Palette.secondary, Palette.canvas),
                                             (Palette.ink, Palette.surface), (Palette.secondary, Palette.surface),
                                             (Palette.onInk, Palette.ink)] {
                let a = luminance(UIColor(foreground).resolvedColor(with: traits))
                let b = luminance(UIColor(background).resolvedColor(with: traits))
                XCTAssertGreaterThanOrEqual((max(a, b) + 0.05) / (min(a, b) + 0.05), 4.5)
            }
        }
    }

    func testPersistenceAndIsolation() throws {
        let disk = storage(), store = WorkspaceStore(storage: storage())
        store.update { $0.input = "未发送"; $0.draft?.sections[0].items[0].isDone = true }
        let old = store.selectedID
        store.newChat(); let new = store.selectedID
        store.update { $0.input = "第二个会话" }
        try disk.save(SavedWorkspace(conversations: store.conversations, selectedID: new))
        let loaded = WorkspaceStore(storage: disk)
        XCTAssertEqual(loaded.selected.input, "第二个会话")
        loaded.select(old)
        XCTAssertEqual(loaded.selected.input, "未发送")
        XCTAssertTrue(loaded.selected.draft!.sections[0].items[0].isDone)
    }
    func testDeleteRestore() {
        let store = WorkspaceStore(storage: storage()), id = UUID()
        _ = id
        let original = store.selected
        store.trash(original.id)
        XCTAssertFalse(store.visibleConversations.contains { $0.id == original.id })
        store.restore(original.id); store.select(original.id)
        XCTAssertEqual(store.selected.messages, original.messages)
    }
    func testStreamRecovery() throws {
        let disk = storage(); var chat = Conversation()
        chat.messages = [ChatMessage(role: "assistant", text: "部分内容", state: .streaming)]
        try disk.save(SavedWorkspace(conversations: [chat], selectedID: chat.id))
        let store = WorkspaceStore(storage: disk)
        XCTAssertEqual(store.selected.messages[0].state, .stopped)
        XCTAssertEqual(store.selected.messages[0].text, "部分内容")
    }
    func testCorruptStoreNotOverwritten() throws {
        let disk = storage(); try disk.prepare()
        let original = Data("invalid".utf8); try original.write(to: disk.stateURL)
        let store = WorkspaceStore(storage: disk)
        XCTAssertNotNil(store.error); store.newChat(); store.persist()
        XCTAssertEqual(try Data(contentsOf: disk.stateURL), original)
    }
    func testStopPreservesPartialReply() async throws {
        let store = WorkspaceStore(storage: storage())
        store.newChat(); store.update { $0.input = "你好" }; store.send()
        try await Task.sleep(for: .milliseconds(100)); XCTAssertTrue(store.isGenerating)
        store.stop(); let partial = store.selected.messages.last!.text
        XCTAssertEqual(store.selected.messages.last?.state, .stopped)
        try await Task.sleep(for: .milliseconds(100))
        XCTAssertEqual(store.selected.messages.last?.text, partial)
        XCTAssertNil(store.generatingID)
    }
    func testEditKeepsOriginalBranch() {
        let store = WorkspaceStore(storage: storage()), originalText = "修改后的问题"
        let original = store.selected
        store.editAndResend(original.messages[0], text: originalText)
        XCTAssertNotEqual(store.selectedID, original.id)
        XCTAssertEqual(store.conversations.first { $0.id == original.id }?.messages, original.messages)
        XCTAssertEqual(store.selected.messages.first?.text, originalText); store.stop()
    }
    func testAttachments() throws {
        let disk = storage()
        let a = try disk.importData(Data("中文资料".utf8), name: "资料.txt", type: .plainText)
        let b = try disk.importData(Data("第二份".utf8), name: "资料.txt", type: .plainText)
        XCTAssertNotEqual(a.filename, b.filename)
        XCTAssertEqual(try String(contentsOf: disk.url(for: a), encoding: .utf8), "中文资料")
        XCTAssertThrowsError(try disk.importData(Data(repeating: 0, count: 10 * 1_024 * 1_024 + 1), name: "large.txt", type: .plainText))
    }
    func testRequestAndURLValidation() throws {
        let disk = storage(), file = try storage().importData(Data("有效附件".utf8), name: "a.txt", type: .plainText)
        var settings = ConnectionSettings(); settings.endpoint = "https://example.com/v1/chat/completions"; settings.model = "test"
        let request = try ChatService.request(settings: settings, token: "test-token", messages: [ChatMessage(role: "user", text: "阅读", attachments: [file])], storage: disk)
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer test-token")
        let object = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        XCTAssertEqual(object["stream"] as? Bool, true)
        XCTAssertTrue(((object["messages"] as! [[String: Any]])[1]["content"] as! String).contains("有效附件"))
        for url in ["http://example.com", "https://user:pass@example.com", "https://example.com?token=secret"] {
            settings.endpoint = url
            XCTAssertThrowsError(try ChatService.request(settings: settings, token: "", messages: [], storage: disk))
        }
    }
    func testSSEUnicodeDoneAndError() throws {
        var parser = SSEDecoder()
        XCTAssertNil(try parser.consume(": heartbeat"))
        XCTAssertNil(try parser.consume("data: {\"choices\":[{\"delta\":{\"content\":\"你好🌱\"}}]}"))
        guard case .delta(let delta) = try parser.consume("") else { return XCTFail("missing text") }
        let text = delta.text
        XCTAssertEqual(text, "你好🌱")
        XCTAssertNil(try parser.consume("data: [DONE]"))
        guard case .done = try parser.consume("") else { return XCTFail("missing terminal") }
        XCTAssertThrowsError(try SSEDecoder.decode("{\"error\":{\"message\":\"private detail\"}}"))
        XCTAssertThrowsError(try SSEDecoder.decode("not json"))
    }
}

final class MarkdownTests: XCTestCase {
    func testStructuredMarkdownWithBlankCodeLines() {
        let input = "# 标题\n\n正文 **强调**\n\n```swift\nlet a = 1\n\nprint(a)\n```\n\n- 第一条\n\n| 项目 | 状态 |\n| --- | --- |\n| 计划 | 完成 |"
        let blocks = MarkdownBlock.parse(input)
        XCTAssertEqual(blocks[0], .heading(1, "标题"))
        XCTAssertTrue(blocks.contains(.code("swift", "let a = 1\n\nprint(a)")))
        XCTAssertTrue(blocks.contains(.list("•", "第一条")))
        XCTAssertTrue(blocks.contains(.table([["项目", "状态"], ["计划", "完成"]])))
    }
}

final class DraftEditingTests: XCTestCase {
    func testEditingKeepsMarkdownChecklistInteractiveAndSkipsCode() {
        var draft = WorkingDraft()
        draft.customMarkdown = "# 清单\n\n```\n- [ ] 代码示例\n```\n\n- [ ] 第一条\n- [x] 第二条"
        draft.toggleMarkdownCheckbox(0)
        XCTAssertTrue(draft.markdown.contains("- [x] 第一条"))
        XCTAssertTrue(draft.markdown.contains("- [ ] 代码示例"))
        draft.toggleMarkdownCheckbox(1)
        XCTAssertTrue(draft.markdown.contains("- [ ] 第二条"))
    }
}

final class DraftContextTests: XCTestCase {
    func testCurrentEditedDraftIsIncludedInModelRequest() throws {
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/v1/chat/completions"; settings.model = "test"
        var draft = WorkingDraft(); draft.customMarkdown = "# 我的修改\n已确认的细节"
        let request = try ChatService.request(settings: settings, token: "", messages: [ChatMessage(role: "user", text: "继续调整")], storage: LocalStorage(), draft: draft)
        let object = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        let messages = object["messages"] as! [[String: Any]]
        XCTAssertTrue((messages[1]["content"] as! String).contains("已确认的细节"))
        XCTAssertEqual(messages[2]["content"] as? String, "继续调整")
    }
}
