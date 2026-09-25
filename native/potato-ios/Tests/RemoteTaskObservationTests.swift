import XCTest
@testable import PotatoMobile

final class RemoteTaskObservationTests: XCTestCase {
    func testRecoveredAndOlderReceiptsDecode() throws {
        let chat: [String: Any] = ["id": "test", "session_id": "test", "name": "Test"]
        let old = try JSONDecoder().decode(RemoteSent.self, from: JSONSerialization.data(withJSONObject: ["chat": chat]))
        XCTAssertNil(old.delivery)
        let recovered = try JSONDecoder().decode(RemoteSent.self, from: JSONSerialization.data(withJSONObject: ["chat": chat, "delivery": "recovered"]))
        XCTAssertEqual(recovered.delivery, "recovered")
    }
    func testStopRequiresDeclaredProtocolAndCapturesOneExactRun() throws {
        var value = try snapshot()
        value.running_request_id = "first"
        XCTAssertNil(RemoteStopRequest(snapshot: value), "Old computers may ignore the expected run argument")
        value.stop_protocol = 1
        let target = try XCTUnwrap(RemoteStopRequest(snapshot: value))
        value.running_request_id = "replacement"
        XCTAssertEqual(target.arguments["expected_run_id"] as? String, "first")
        XCTAssertEqual(target.arguments["chat_id"] as? String, "test")
        XCTAssertNotEqual(target, RemoteStopRequest(snapshot: value))
        value.running_request_id = ""; XCTAssertNil(RemoteStopRequest(snapshot: value))
        value.running_request_id = nil; XCTAssertNil(RemoteStopRequest(snapshot: value))
        value.running_request_id = "first"; value.stop_protocol = 2; XCTAssertNil(RemoteStopRequest(snapshot: value))
        var idle = try snapshot(status: "idle"); idle.stop_protocol = 1; idle.running_request_id = "first"
        XCTAssertNil(RemoteStopRequest(snapshot: idle))
    }
    private func snapshot(status: String = "running", messages: [[String: Any]] = [], approvals: [[String: Any]] = [], questions: [[String: Any]] = [], outcome: String? = nil) throws -> RemoteSnapshot {
        var json: [String: Any] = ["chat": ["id": "test", "session_id": "test", "name": "Test"], "status": status, "messages": messages, "live": [], "approvals": approvals, "questions": questions]
        if let outcome { json["outcome"] = ["status": outcome] }
        return try JSONDecoder().decode(RemoteSnapshot.self, from: JSONSerialization.data(withJSONObject: json))
    }
    func testFreshnessExpiresDuringSlowRequestAndDoesNotResetOnFailure() throws {
        let now = Date(timeIntervalSince1970: 100)
        var state = RemoteTaskObservation()
        XCTAssertFalse(state.isCurrent(at: now))
        state.received(requestedAt: now)
        XCTAssertTrue(state.isCurrent(at: now.addingTimeInterval(7.9)))
        XCTAssertFalse(state.isCurrent(at: now.addingTimeInterval(8)))
        state.failed("offline")
        XCTAssertFalse(state.isCurrent(at: now.addingTimeInterval(1)))
        XCTAssertEqual(state.confirmedAt, now)
        XCTAssertEqual(state.title(snapshot: try snapshot(), at: now), "连接中断")
        state.received(requestedAt: now.addingTimeInterval(10))
        XCTAssertTrue(state.isCurrent(at: now.addingTimeInterval(11)))
        XCTAssertNil(state.failure)
    }
    func testResumeRequiresNewConfirmationAndDelayedResponsesStayStale() {
        let now = Date(timeIntervalSince1970: 100)
        var state = RemoteTaskObservation(); state.received(requestedAt: now)
        state.suspend(); XCTAssertFalse(state.isCurrent(at: now))
        XCTAssertEqual(state.title(snapshot: nil, at: now), "已暂停更新")
        state.resume(); XCTAssertFalse(state.isCurrent(at: now))
        state.received(requestedAt: now.addingTimeInterval(-12))
        XCTAssertFalse(state.isCurrent(at: now))
        state.received(requestedAt: now.addingTimeInterval(1))
        XCTAssertFalse(state.isCurrent(at: now), "A backwards wall clock jump cannot keep stale actions enabled")
    }
    func testLatestActivitySupersedesOldStreamingFrames() throws {
        let reasoning: [String: Any] = ["id": "r", "role": "assistant", "kind": "reasoning", "text": "analysis", "status": "in_progress"]
        let tool: [String: Any] = ["id": "t", "role": "assistant", "kind": "function_call", "text": "exec_command\n{}", "status": "in_progress"]
        let answer: [String: Any] = ["id": "a", "role": "assistant", "kind": "message", "text": "answer", "status": "in_progress"]
        XCTAssertEqual(try snapshot(messages: [reasoning]).activityTitle, "正在思考")
        XCTAssertEqual(try snapshot(messages: [reasoning, tool]).activityTitle, "执行命令")
        XCTAssertEqual(try snapshot(messages: [reasoning, tool]).phaseTitle, "正在执行")
        XCTAssertEqual(try snapshot(messages: [reasoning]).phaseTitle, "正在思考")
        XCTAssertEqual(try snapshot(messages: [reasoning, tool]).activeProcessID, "t")
        XCTAssertEqual(try snapshot(messages: [reasoning, tool, answer]).activityTitle, "正在回复")
        XCTAssertNil(try snapshot(messages: [reasoning, tool, answer]).activeProcessID)
        let output: [String: Any] = ["id": "o", "role": "tool", "kind": "function_call_output", "text": "done", "status": "completed"]
        XCTAssertEqual(try snapshot(messages: [reasoning, tool, output]).activityTitle, "电脑正在处理…")
        XCTAssertNil(try snapshot(messages: [reasoning, tool, output]).activeProcessID)
    }
    func testWaitingForApprovalOrAnswerNeverClaimsActiveReasoning() throws {
        let approval = try snapshot(approvals: [["request_id": "approval"]])
        XCTAssertEqual(approval.activityTitle, "等待你的批准"); XCTAssertTrue(approval.needsUserResponse); XCTAssertNil(approval.activeProcessID)
        let question = try snapshot(questions: [["request_id": "q", "title": "Choose", "status": "pending", "options": []]])
        XCTAssertEqual(question.activityTitle, "等待你的回答"); XCTAssertTrue(question.needsUserResponse)
        XCTAssertEqual(try snapshot(status: "idle", outcome: "cancelled").activityTitle, "本轮任务已停止")
        XCTAssertEqual(try snapshot(status: "idle", outcome: "failed").activityTitle, "本轮任务失败")
        XCTAssertEqual(try snapshot(status: "idle", outcome: "completed").activityTitle, "本轮任务已完成")
    }
    func testStalenessIgnoresOneSlowPollButNotALongAbsence() {
        let start = Date(); var state = RemoteTaskObservation()
        XCTAssertFalse(state.isStale(at: start))
        state.received(requestedAt: start)
        XCTAssertFalse(state.isCurrent(at: start.addingTimeInterval(10)))
        XCTAssertFalse(state.isStale(at: start.addingTimeInterval(10)))
        state.suspend(); state.resume()
        XCTAssertTrue(state.isStale(at: start.addingTimeInterval(RemoteTaskObservation.staleAfter + 1)))
        state.received(requestedAt: start); state.failed("offline")
        XCTAssertTrue(state.isStale(at: start))
    }
}
