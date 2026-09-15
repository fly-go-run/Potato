import XCTest
@testable import PotatoMobile

final class RemotePresentationTests: XCTestCase {
    func testApprovalDecodesReviewReasonAndOldDesktopPayload() throws {
        let legacy = try JSONDecoder().decode(RemoteApproval.self, from: Data(#"{"request_id":"old"}"#.utf8))
        XCTAssertNil(legacy.review_rationale)
        XCTAssertTrue(legacy.reviewExplanation.contains("本次请求"))
        let reviewed = try JSONDecoder().decode(RemoteApproval.self, from: Data(#"{"request_id":"new","review_rationale":"需要额外授权","review_outcome":"ask_user"}"#.utf8))
        XCTAssertEqual(reviewed.reviewExplanation, "需要额外授权")
        let failed = try JSONDecoder().decode(RemoteApproval.self, from: Data(#"{"request_id":"failed","review_failure":"timeout"}"#.utf8))
        XCTAssertTrue(failed.reviewExplanation.contains("未能完成"))
    }
    private func message(_ id: String, _ role: String, _ kind: String, text: String = "正文", status: String = "completed", callID: String? = nil, name: String? = nil, arguments: String? = nil, output: String? = nil, state: String? = nil) -> RemoteMessage {
        RemoteMessage(id: id, role: role, kind: kind, text: text, status: status, callID: callID, name: name, arguments: arguments, output: output, state: state)
    }
    func testProcessTitleUsesStructuredKnownName() {
        XCTAssertEqual(message("c", "assistant", "function_call", text: "任意内容", name: "read_file").processTitle, "读取文件")
    }
    func testProcessTitleUsesStructuredUnknownName() {
        XCTAssertEqual(message("c", "assistant", "function_call", text: "任意内容", name: "mcp_abc").processTitle, "调用 mcp_abc")
    }
    func testStructuredToolFieldsDecodeAndLegacyFieldsStayOptional() throws {
        let data = #"{"id":"c","role":"assistant","kind":"function_call","text":"legacy","call_id":"c1","name":"read_file","arguments":"{\"path\":\"a.md\"}","output":"内容","state":"success"}"#.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(RemoteMessage.self, from: data)
        XCTAssertEqual(decoded.callID, "c1")
        XCTAssertEqual(decoded.name, "read_file")
        XCTAssertEqual(decoded.arguments, #"{"path":"a.md"}"#)
        XCTAssertEqual(decoded.output, "内容")
        XCTAssertEqual(decoded.state, "success")
        let legacy = try JSONDecoder().decode(RemoteMessage.self, from: #"{"id":"a","text":"正文"}"#.data(using: .utf8)!)
        XCTAssertNil(legacy.callID)
        XCTAssertNil(legacy.name)
        XCTAssertNil(legacy.arguments)
        XCTAssertNil(legacy.output)
        XCTAssertNil(legacy.state)
    }
    func testProcessGroupsNeverAbsorbUserAnswerOrNotice() {
        let frames = [message("u", "user", "message"), message("r", "assistant", "reasoning"), message("c", "assistant", "function_call"), message("o", "tool", "function_call_output"), message("a", "assistant", "message"), message("n", "system", "notice"), message("r2", "assistant", "reasoning"), message("u2", "user", "message")]
        let rows = RemoteConversationRow.make(frames)
        XCTAssertEqual(rows.map(\.id), ["u", "r", "a", "n", "r2", "u2"])
        XCTAssertEqual(rows[1].messages.map(\.id), ["r", "c", "o"])
        XCTAssertTrue(rows[3].messages[0].isNotice)
        XCTAssertFalse(rows[2].isProcess)
        XCTAssertFalse(message("user-code", "user", "function_call").isProcess)
    }
    func testProcessIdentitySurvivesStreamingAppendAndDoesNotUseLogAsTitle() {
        let initial = message("r", "assistant", "reasoning", text: "", status: "in_progress")
        XCTAssertEqual(RemoteConversationRow.make([initial]).first?.id, RemoteConversationRow.make([initial, message("c", "assistant", "function_call")]).first?.id)
        XCTAssertTrue(initial.isStreaming)
        XCTAssertEqual(message("c", "assistant", "function_call", text: "read_file\n{\"path\":\"/project/README.md\"}").processTitle, "读取文件")
        XCTAssertEqual(message("o", "tool", "function_call_output", text: "private output").processTitle, "执行结果")
        XCTAssertEqual(message("c", "assistant", "function_call", text: "secret content with spaces").processTitle, "执行操作")
    }
    func testReplayUpdatesPreserveEmptyRunningProcessAndFilterEmptyAnswers() throws {
        let data = #"{"chat":{"id":"chat","session_id":"chat","name":"Demo"},"status":"running","messages":[{"id":"r","role":"assistant","kind":"reasoning","text":"old","status":"in_progress"}],"live":[{"id":"r","role":"assistant","kind":"reasoning","text":"new","status":"completed"},{"id":"c","role":"assistant","kind":"function_call","text":"","status":"in_progress"},{"id":"a","role":"assistant","kind":"message","text":"","status":"in_progress"}],"approvals":[],"questions":[]}"#.data(using: .utf8)!
        let snapshot = try JSONDecoder().decode(RemoteSnapshot.self, from: data)
        XCTAssertEqual(snapshot.displayMessages.map(\.id), ["r", "c"])
        XCTAssertEqual(snapshot.displayMessages[0].text, "new")
        XCTAssertEqual(RemoteConversationRow.make(snapshot.displayMessages).count, 1)
    }

    // The audit's red regression now exercises real storage and command binding.
    @MainActor
    func testAccountDraftsAreScopedToTargetComputer() throws {
        let relay = URL(string: "https://fixture.invalid")!
        let owner = String(repeating: "a", count: 64)
        let first = RemoteDevice(id: "00000000-0000-4000-8000-000000000001", name: "First Mac", relay: relay, owner: owner)
        let second = RemoteDevice(id: "00000000-0000-4000-8000-000000000002", name: "Second Mac", relay: relay, owner: owner)
        // Account credentials are intentionally shared; task drafts must not be.
        XCTAssertEqual(first.account, second.account)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let repository = RemoteDraftRepository(file: directory.appendingPathComponent("drafts.json"))
        for project in [nil, "/same/project"] as [String?] {
            let firstAddress = RemoteDraftAddress(device: first, projectPath: project)
            let secondAddress = RemoteDraftAddress(device: second, projectPath: project)
            let request = RemotePendingSend(id: UUID().uuidString, text: "Only First Mac", chatID: nil, projectPath: project, target: firstAddress.target)
            try repository.reserve(request, at: firstAddress, text: request.text)
            XCTAssertEqual(try repository.load(firstAddress).pending, request)
            XCTAssertTrue(try repository.load(secondAddress).text.isEmpty)
            XCTAssertNil(try repository.load(secondAddress).pending)
            XCTAssertThrowsError(try request.arguments(for: second))
        }
    }
}
