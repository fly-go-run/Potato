import XCTest
@testable import PotatoMobile

final class ActivityPresentationTests: XCTestCase {
    func testInterleavedOrderSurvivesCodingAndVersionSelection() throws {
        var message = ChatMessage(role: "assistant", text: "Done", state: .streaming)
        message.receive(ReplyDelta(reasoning: "Plan"))
        message.searches = [WebSearchRun(id: "search", query: "topic", state: "complete", results: [])]
        message.codeRuns = [CodeExecutionRun(id: "python", state: "failed", code: "fail()")]
        message.recordActivity("code:python"); message.recordActivity("search:search"); message.recordActivity("code:python")
        message.finishReply(.complete)
        let restored = try JSONDecoder().decode(ChatMessage.self, from: JSONEncoder().encode(message))
        XCTAssertEqual(restored.activitySteps.map(\.id), ["reasoning", "code:python", "search:search"])
        XCTAssertTrue(restored.activitySteps[1].failed)
        let version = ReplyVersion(activityOrder: ["search:old"], searches: [WebSearchRun(id: "old", query: "old", state: "complete", results: [])], text: "Previous", state: .complete)
        message.versions = [version]; message.selectedVersionID = version.id
        XCTAssertEqual(message.activitySteps.map(\.id), ["search:old"])
    }
    func testActionTitleIsUsedAndOversizedTitlesAreRejected() throws {
        let run = CodeExecutionRun(id: "deck", title: "生成演示文稿", state: "running", code: "print(1)")
        var message = ChatMessage(role: "assistant", text: ""); message.codeRuns = [run]
        XCTAssertEqual(message.activitySteps[0].title, "生成演示文稿")
        var invalid = run; invalid.title = String(repeating: "a", count: 121)
        XCTAssertThrowsError(try invalid.validate())
    }
    func testPPTXArtifactIsSavedWithCorrectTypeAndUnsafeNamesAreRejected() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let storage = LocalStorage(root: root)
        let bytes = Data("fixture-bytes".utf8)
        let file = try storage.importArtifact(SandboxArtifact(name: "deck.pptx", mime: "application/octet-stream", base64: bytes.base64EncodedString()))
        XCTAssertEqual(file.type, "application/vnd.openxmlformats-officedocument.presentationml.presentation")
        XCTAssertEqual(try Data(contentsOf: storage.url(for: file)), bytes)
        XCTAssertThrowsError(try storage.importArtifact(SandboxArtifact(name: "../deck.pptx", mime: file.type, base64: bytes.base64EncodedString())))
    }
    func testSameNamedFilesStayWithProducingCall() {
        let first = Attachment(name: "slide.png", filename: "one", type: "image/png", size: 100)
        let second = Attachment(name: "slide.png", filename: "two", type: "image/png", size: 100)
        var message = ChatMessage(role: "assistant", text: "", attachments: [first, second])
        message.codeRuns = [CodeExecutionRun(id: "one", attachmentIDs: [first.id], state: "complete", code: ""), CodeExecutionRun(id: "two", attachmentIDs: [second.id], state: "complete", code: "")]
        XCTAssertEqual(message.activitySteps.map { $0.attachments.map(\.id) }, [[first.id], [second.id]])
    }
    func testLegacyRecordsKeepFilesAndStableSteps() throws {
        let raw = #"{"id":"old","state":"complete","code":"print(1)","result":{"status":"complete","stdout":"1","stderr":"","text":"","artifacts":[]}}"#
        let run = try JSONDecoder().decode(CodeExecutionRun.self, from: Data(raw.utf8))
        XCTAssertNil(run.attachmentIDs)
        var message = ChatMessage(role: "assistant", text: "old"); message.codeRuns = [run]
        XCTAssertEqual(message.activitySteps.first?.output, "1")
    }
    private func remote(_ id: String, kind: String = "function_call", call: String? = "c", status: String = "completed", state: String? = nil, output: String? = nil) -> RemoteMessage {
        RemoteMessage(id: id, role: kind.contains("output") ? "tool" : "assistant", kind: kind, text: "", status: status, callID: call, name: "exec_command", arguments: #"{"command":"node deck.js","description":"生成演示文稿"}"#, output: output, state: state)
    }
    func testRemotePairsResultsAndRetainsFailureAndOrphans() {
        let messages = [remote("call"), remote("output", kind: "function_call_output", output: #"{"exit_code":1,"stderr":"shape error"}"#), remote("orphan", kind: "function_call_output", call: "other", output: "orphan")]
        let steps = ActivityStep.remote(messages, running: false, confirmed: true, activeID: nil)
        XCTAssertEqual(steps.count, 2); XCTAssertEqual(steps[0].id, "call")
        XCTAssertEqual(steps[0].title, "生成演示文稿"); XCTAssertEqual(steps[0].input, "node deck.js")
        XCTAssertTrue(steps[0].failed); XCTAssertTrue(steps[0].output.contains("shape error"))
        XCTAssertEqual(steps[1].output, "orphan")
    }
    func testRemoteUnconfirmedNeverAnimatesOrClaimsSuccess() {
        let frame = remote("call", status: "in_progress")
        let stale = ActivityStep.remote([frame], running: true, confirmed: false, activeID: "call")[0]
        XCTAssertFalse(stale.isRunning); XCTAssertEqual(stale.status, "状态待确认")
        let active = ActivityStep.remote([frame], running: true, confirmed: true, activeID: "call")[0]
        XCTAssertTrue(active.isRunning)
        let ended = ActivityStep.remote([frame], running: false, confirmed: true, activeID: nil)[0]
        XCTAssertEqual(ended.status, "状态待确认")
        let thought = remote("thought", kind: "reasoning", status: "in_progress")
        let superseded = ActivityStep.remote([thought, frame], running: true, confirmed: true, activeID: "call")[0]
        XCTAssertEqual(superseded.title, "思考记录"); XCTAssertFalse(superseded.isRunning)
    }
}
