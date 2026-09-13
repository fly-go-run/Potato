import Foundation

/// Captured when the confirmation opens, never replaced by subsequent polls.
struct RemoteStopRequest: Equatable {
    let chatID: String
    let runID: String
    init?(snapshot: RemoteSnapshot) {
        guard snapshot.stop_protocol == 1, snapshot.status == "running",
              let runID = snapshot.running_request_id, !runID.isEmpty else { return nil }
        chatID = snapshot.chat.id; self.runID = runID
    }
    var arguments: [String: Any] { ["chat_id": chatID, "expected_run_id": runID] }
}

/// A snapshot is evidence of a past state, not a promise that a task is still running.
struct RemoteTaskObservation {
    static let freshness: TimeInterval = 8
    private(set) var confirmedAt: Date?
    private(set) var failure: String?
    private(set) var suspended = false
    mutating func received(requestedAt: Date) { confirmedAt = requestedAt; failure = nil; suspended = false }
    mutating func failed(_ message: String) { failure = message }
    mutating func suspend() { suspended = true }
    mutating func resume() { suspended = false; confirmedAt = nil }
    func isCurrent(at now: Date) -> Bool {
        guard !suspended, failure == nil, let confirmedAt else { return false }
        let age = now.timeIntervalSince(confirmedAt)
        return age >= 0 && age < Self.freshness
    }
    func title(snapshot: RemoteSnapshot?, at now: Date) -> String {
        if suspended { return "已暂停更新" }
        if failure != nil { return "连接中断，任务状态未确认" }
        if snapshot == nil { return "正在读取任务状态…" }
        if !isCurrent(at: now) { return "正在确认任务状态…" }
        return snapshot!.activityTitle
    }
}

extension RemoteSnapshot {
    var activityTitle: String {
        if status != "running" {
            if outcome?.status == "cancelled" { return "本轮任务已停止" }
            if outcome?.status == "failed" { return "本轮任务失败" }
            if outcome?.status == "completed" { return "本轮任务已完成" }
            return "任务已就绪"
        }
        if !approvals.isEmpty { return "等待你的批准" }
        if questions.contains(where: { $0.status == "pending" }) { return "等待你的回答" }
        // A completed tool output supersedes an older in-progress call frame.
        // Do not keep claiming reasoning when a later body or tool result arrived.
        guard let latest = displayMessages.last(where: { $0.role != "user" && !$0.isNotice }) else { return "正在准备回复…" }
        if latest.kind == "reasoning" && latest.isStreaming { return "正在思考" }
        if latest.isProcess && latest.isStreaming { return latest.processTitle }
        if !latest.isProcess && latest.isStreaming { return "正在回复" }
        return "电脑正在处理…"
    }
    var needsUserResponse: Bool { !approvals.isEmpty || questions.contains(where: { $0.status == "pending" }) }
    var activeProcessID: String? {
        guard status == "running", !needsUserResponse, let last = displayMessages.last(where: { $0.role != "user" && !$0.isNotice }), last.isProcess, last.isStreaming else { return nil }
        return last.id
    }
}
