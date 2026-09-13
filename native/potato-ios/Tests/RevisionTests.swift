import XCTest
@testable import PotatoMobile
import UniformTypeIdentifiers

@MainActor final class RevisionTests: XCTestCase {
    func storage() -> LocalStorage { LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)) }
    func testDocumentRestoreKeepsCurrentContentRecoverable() throws {
        var draft = WorkingDraft()
        let original = draft.markdown
        draft.replaceContent("# 新文稿\n\n- [ ] 新清单")
        XCTAssertEqual(draft.versionHistory.count, 1)
        draft.toggleMarkdownCheckbox(0)
        XCTAssertEqual(draft.versionHistory.count, 2)
        let edited = draft.markdown
        draft.restore(draft.versionHistory[0])
        XCTAssertEqual(draft.markdown, original)
        XCTAssertEqual(draft.versionHistory.last?.markdown, edited)
        let decoded = try JSONDecoder().decode(WorkingDraft.self, from: JSONEncoder().encode(draft))
        XCTAssertEqual(decoded.versionHistory.count, 3)
        XCTAssertEqual(decoded.markdown, original)
    }
    func testOldPersistedDraftAndMessageDecodeWithoutRevisionFields() throws {
        var draftJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(WorkingDraft())) as! [String: Any]
        draftJSON.removeValue(forKey: "revisions")
        let draft = try JSONDecoder().decode(WorkingDraft.self, from: JSONSerialization.data(withJSONObject: draftJSON))
        XCTAssertTrue(draft.versionHistory.isEmpty)
        var messageJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(ChatMessage(role: "assistant", text: "旧消息"))) as! [String: Any]
        messageJSON.removeValue(forKey: "versions"); messageJSON.removeValue(forKey: "selectedVersionID")
        let message = try JSONDecoder().decode(ChatMessage.self, from: JSONSerialization.data(withJSONObject: messageJSON))
        XCTAssertEqual(message.displayText, "旧消息"); XCTAssertEqual(message.versionCount, 1)
    }
    func testRetryKeepsOriginalAnswerEvenWhenNewAttemptStops() async throws {
        let disk = storage(), store = WorkspaceStore(storage: storage())
        let original = store.selected.messages.last!.text
        store.retry()
        XCTAssertEqual(store.selected.messages.last?.versions?.first?.text, original)
        try await Task.sleep(for: .milliseconds(70)); store.stop()
        let messageID = store.selected.messages.last!.id
        store.chooseReplyVersion(messageID, offset: -1)
        XCTAssertEqual(store.selected.messages.last?.displayText, original)
        try disk.save(store.snapshot)
        let restored = WorkspaceStore(storage: disk)
        XCTAssertEqual(restored.selected.messages.last?.displayText, original)
        store.chooseReplyVersion(messageID, offset: 1)
        XCTAssertEqual(store.selected.messages.last?.displayState, .stopped)
    }
    func testReplacingDraftFromReplyPreservesOriginal() {
        let store = WorkspaceStore(storage: storage()), original = WorkingDraft().markdown
        store.saveReplyAsDraft(ChatMessage(role: "assistant", text: "# 修改稿\n新的内容"))
        XCTAssertEqual(store.selected.draft?.versionHistory.first?.markdown, original)
        XCTAssertEqual(store.selected.draft?.title, "修改稿")
    }
    func testSelectedReplyVersionIsSentInNextRequest() throws {
        var message = ChatMessage(role: "assistant", text: "最新回复")
        let version = ReplyVersion(text: "用户选回的回复", state: .complete, failure: nil)
        message.versions = [version]; message.selectedVersionID = version.id
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/v1/chat/completions"; settings.model = "test"
        let request = try ChatService.request(settings: settings, token: "", messages: [message], storage: storage())
        let body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        XCTAssertEqual((body["messages"] as! [[String: Any]])[1]["content"] as? String, "用户选回的回复")
    }
    func testOrphanCleanupPreservesDeletedConversationFilesAndUnknownFiles() throws {
        let disk = storage()
        let kept = try disk.importData(Data("属于最近删除".utf8), name: "kept.txt", type: .plainText)
        let pending = try disk.importData(Data("未发送".utf8), name: "pending.txt", type: .plainText)
        let orphan = try disk.importData(Data("已移除".utf8), name: "orphan.txt", type: .plainText)
        let unknown = disk.root.appendingPathComponent("Attachments/readme.txt")
        try Data("非应用生成的文件".utf8).write(to: unknown)
        var chat = Conversation(); chat.deletedAt = Date()
        chat.messages = [ChatMessage(role: "user", text: "", attachments: [kept])]; chat.pendingAttachments = [pending]
        let saved = SavedWorkspace(conversations: [chat], selectedID: chat.id)
        try disk.pruneUnreferencedAttachments(in: saved)
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: kept).path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: pending).path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: unknown.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: disk.url(for: orphan).path))
    }
}

@MainActor final class ReplyBranchTests: XCTestCase {
    func testSwitchingEarlierReplyCreatesBranchWithoutChangingLaterTurns() {
        let disk = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let store = WorkspaceStore(storage: disk)
        let alternative = ReplyVersion(text: "旧答案", state: .complete, failure: nil)
        store.update { chat in
            chat.messages[1].versions = [alternative]
            chat.messages.append(ChatMessage(role: "user", text: "基于当前回答继续"))
            chat.messages.append(ChatMessage(role: "assistant", text: "原来的后续内容"))
        }
        let original = store.selected
        store.chooseReplyVersion(original.messages[1].id, offset: -1)
        XCTAssertNotEqual(store.selectedID, original.id)
        XCTAssertEqual(store.selected.messages.count, 2)
        XCTAssertEqual(store.selected.messages[1].displayText, "旧答案")
        XCTAssertEqual(store.conversations.first { $0.id == original.id }?.messages, original.messages)
    }
}
