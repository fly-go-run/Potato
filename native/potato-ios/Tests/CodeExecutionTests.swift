import XCTest
@testable import PotatoMobile

@MainActor final class CodeExecutionTests: XCTestCase {
    func testExecutionEventSupportsLargeArtifactsAndRejectsInvalidCompletion() throws {
        let execution = SandboxExecution(status: "complete", stdout: "323", stderr: "", text: "", artifacts: [SandboxArtifact(name: "large.txt", mime: "text/plain", base64: Data(repeating: 65, count: 850_000).base64EncodedString())])
        let run = CodeExecutionRun(id: "run-1", state: "complete", code: "print(17*19)", result: execution)
        let raw = try JSONSerialization.data(withJSONObject: ["potato_execution": JSONSerialization.jsonObject(with: JSONEncoder().encode(run))])
        XCTAssertGreaterThan(raw.count, 1_000_000)
        var decoder = SSEDecoder(); XCTAssertNil(try decoder.consume("data: " + String(decoding: raw, as: UTF8.self)))
        guard case .execution(let decoded) = try decoder.consume("") else { return XCTFail("Missing code event") }
        XCTAssertEqual(decoded.result?.artifacts.first?.base64, execution.artifacts.first?.base64)
        XCTAssertThrowsError(try CodeExecutionRun(id: "bad", state: "complete", code: "").validate())
        XCTAssertThrowsError(try CodeExecutionRun(id: "", state: "running", code: "").validate())
    }
    func testToolFilesOnlyGoToCloudEndpointAndMissingBudgetIsReported() throws {
        let disk = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let input = try disk.importArtifact(SandboxArtifact(name: "values.csv", mime: "text/csv", base64: Data("value\n17\n19".utf8).base64EncodedString()))
        let message = ChatMessage(role: "user", text: "计算总和", attachments: [input])
        var settings = ConnectionSettings(); settings.demo = false; settings.model = "fixture"; settings.endpoint = "https://fixture.invalid/v1/chat/completions"
        settings.cloudAccount = RemoteAccountProfile(owner: "fixture", email: "fixture@example.test", relay: URL(string: "https://fixture.invalid")!, scope: "cloud")
        let request = try ChatService.request(settings: settings, token: "fixture", messages: [message], storage: disk)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: request.httpBody!) as? [String: Any])
        let sandbox = try XCTUnwrap(json["sandbox"] as? [String: Any]); let files = try XCTUnwrap(sandbox["files"] as? [[String: String]])
        XCTAssertEqual(files.count, 1); XCTAssertEqual(files[0]["name"], "input-1.csv")
        XCTAssertEqual(Data(base64Encoded: files[0]["base64"]!), Data("value\n17\n19".utf8))
        let limited = try SandboxService.automaticInput(messages: [message], storage: disk, bodyBytes: 4 * 1_024 * 1_024)
        XCTAssertTrue((limited["files"] as? [[String: String]])!.isEmpty)
        XCTAssertTrue((limited["file_notes"] as? [String])!.joined().contains("未提供"))
        settings.cloudAccount = nil
        let custom = try ChatService.request(settings: settings, token: "fixture", messages: [message], storage: disk)
        XCTAssertNil((try JSONSerialization.jsonObject(with: custom.httpBody!) as? [String: Any])?["sandbox"])
    }
    func testAutomaticExecutionPersistsArtifactsAndReplyVersionsWithoutDuplication() throws {
        let disk = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let persistedStore = WorkspaceStore(storage: disk)
        persistedStore.update { $0.messages.append(ChatMessage(role: "assistant", text: "", state: .streaming)) }
        let messageID = persistedStore.selected.messages.last!.id, chatID = persistedStore.selectedID
        let run = CodeExecutionRun(id: "code-1", state: "running", code: "print(323)")
        try persistedStore.recordCodeExecution(run, messageID: messageID, conversationID: chatID)
        let result = SandboxExecution(status: "complete", stdout: "323", stderr: "", text: "", artifacts: [SandboxArtifact(name: "report.md", mime: "text/markdown", base64: Data("# 323".utf8).base64EncodedString())])
        let done = CodeExecutionRun(id: run.id, state: "complete", code: run.code, result: result)
        try persistedStore.recordCodeExecution(done, messageID: messageID, conversationID: chatID)
        try persistedStore.recordCodeExecution(done, messageID: messageID, conversationID: chatID)
        XCTAssertEqual(persistedStore.selected.messages.last?.attachments.count, 1)
        let saved = try XCTUnwrap(persistedStore.selected.messages.last?.codeRuns?.first)
        XCTAssertEqual(saved.result?.artifacts, [SandboxArtifact(name: "report.md", mime: "text/markdown", base64: "")])
        XCTAssertNoThrow(try saved.validate())
        persistedStore.update { $0.messages[$0.messages.count - 1].finishReply(.complete) }
        persistedStore.retry(); persistedStore.stop(); persistedStore.chooseReplyVersion(persistedStore.selected.messages.last!.id, offset: -1); persistedStore.persist()
        let restored = WorkspaceStore(storage: disk), message = restored.selected.messages.last!
        XCTAssertEqual(message.displayCodeRuns.first?.result?.stdout, "323")
        let file = try XCTUnwrap(message.displayAttachments.first)
        XCTAssertEqual(try Data(contentsOf: disk.url(for: file)), Data("# 323".utf8))
    }
    func testStopAndRestartNeverKeepAnExecutionSpinnerRunning() {
        for restart in [false, true] {
            var message = ChatMessage(role: "assistant", text: "", state: .streaming)
            message.codeRuns = [CodeExecutionRun(id: "code-1", state: "running", code: "while True: pass")]
            if restart { message.recoverInterruptedReply() } else { message.finishReply(.stopped) }
            XCTAssertEqual(message.codeRuns?.first?.state, "stopped")
            XCTAssertNil(message.codeRuns?.first?.result)
        }
    }
}
