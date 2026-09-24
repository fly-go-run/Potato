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
    /// Survives backgrounding, so a long absence reads as stale on return.
    private(set) var lastConfirmedAt: Date?
    private(set) var failure: String?
    private(set) var suspended = false
    mutating func received(requestedAt: Date) { confirmedAt = requestedAt; lastConfirmedAt = requestedAt; failure = nil; suspended = false }
    mutating func failed(_ message: String) { failure = message }
    mutating func suspend() { suspended = true }
    mutating func resume() { suspended = false; confirmedAt = nil }
    func isCurrent(at now: Date) -> Bool {
        guard !suspended, failure == nil, let confirmedAt else { return false }
        let age = now.timeIntervalSince(confirmedAt)
        return age >= 0 && age < Self.freshness
    }
    /// Presentation only: one slow poll is not worth a warning. Actions still require `isCurrent`.
    static let staleAfter: TimeInterval = 20
    func isStale(at now: Date) -> Bool {
        if failure != nil { return true }
        // Opening the task: the first poll is on its way.
        guard !suspended, let lastConfirmedAt else { return false }
        return now.timeIntervalSince(lastConfirmedAt) >= Self.staleAfter
    }
    func title(snapshot: RemoteSnapshot?, at now: Date) -> String {
        if suspended { return L10n.tr("已暂停更新") }
        if failure != nil { return L10n.tr("连接中断，任务状态未确认") }
        if snapshot == nil { return L10n.tr("正在读取任务状态…") }
        if isStale(at: now) { return L10n.tr("正在确认任务状态…") }
        return snapshot!.activityTitle
    }
}

extension RemoteSnapshot {
    var activityTitle: String {
        if status != "running" {
            if outcome?.status == "cancelled" { return L10n.tr("本轮任务已停止") }
            if outcome?.status == "failed" { return L10n.tr("本轮任务失败") }
            if outcome?.status == "completed" { return L10n.tr("本轮任务已完成") }
            return L10n.tr("任务已就绪")
        }
        if !approvals.isEmpty { return L10n.tr("等待你的批准") }
        if questions.contains(where: { $0.status == "pending" }) { return L10n.tr("等待你的回答") }
        // A completed tool output supersedes an older in-progress call frame.
        // Do not keep claiming reasoning when a later body or tool result arrived.
        guard let latest = displayMessages.last(where: { $0.role != "user" && !$0.isNotice }) else { return L10n.tr("正在准备回复…") }
        if latest.kind == "reasoning" && latest.isStreaming { return L10n.tr("正在思考") }
        if latest.isProcess && latest.isStreaming { return latest.processTitle }
        if !latest.isProcess && latest.isStreaming { return L10n.tr("正在回复") }
        return L10n.tr("电脑正在处理…")
    }
    /// The status line under a running reply names the kind of work; the group row names the step.
    var phaseTitle: String {
        guard status == "running", !needsUserResponse,
              let latest = displayMessages.last(where: { $0.role != "user" && !$0.isNotice }),
              latest.isProcess, latest.kind != "reasoning", latest.isStreaming else { return activityTitle }
        return L10n.tr("正在执行")
    }
    /// The newest process row is a thought still streaming; that row already says 正在思考.
    var isThinking: Bool {
        activeProcessID != nil && displayMessages.last(where: { $0.role != "user" && !$0.isNotice })?.kind == "reasoning"
    }
    var needsUserResponse: Bool { !approvals.isEmpty || questions.contains(where: { $0.status == "pending" }) }
    var activeProcessID: String? {
        guard status == "running", !needsUserResponse, let last = displayMessages.last(where: { $0.role != "user" && !$0.isNotice }), last.isProcess, last.isStreaming else { return nil }
        return last.id
    }
}
