import SwiftUI

struct ReplyDelta: Equatable {
    var text = ""
    var reasoning = ""
    var limited = false
}

/// Client-observed reasoning intervals, not a server-reported compute timer.
struct ReasoningTrace: Codable, Equatable {
    var text = ""
    var state: MessageState = .streaming
    var elapsed: TimeInterval = 0
    var activeSince: Date?
    var lastReceivedAt: Date?

    mutating func append(_ delta: String, at now: Date) {
        guard !delta.isEmpty else { return }
        if activeSince == nil { activeSince = now }
        state = .streaming; text += delta; lastReceivedAt = now
    }
    mutating func finish(_ state: MessageState, at now: Date) {
        guard self.state == .streaming else { return }
        if let start = activeSince { elapsed += max(0, now.timeIntervalSince(start)) }
        activeSince = nil; self.state = state
    }
    func duration(at now: Date) -> TimeInterval {
        elapsed + (activeSince.map { max(0, now.timeIntervalSince($0)) } ?? 0)
    }
}

extension ChatMessage {
    mutating func receive(_ delta: ReplyDelta, at now: Date = Date()) {
        guard role == "assistant", state == .streaming else { return }
        if !delta.reasoning.isEmpty {
            recordActivity("reasoning")
            if reasoning == nil { reasoning = ReasoningTrace() }
            reasoning?.append(delta.reasoning, at: now)
        }
        if !delta.text.isEmpty {
            reasoning?.finish(.complete, at: now)
            text += delta.text
        }
    }
    mutating func finishReply(_ target: MessageState, failure: String? = nil, at now: Date = Date()) {
        guard state == .streaming else { return }
        reasoning?.finish(target, at: now)
        state = target; self.failure = failure
        stopActiveCodeRuns(target)
        searches = searches?.map { run in
            var run = run
            if run.state == "searching" { run.state = target == .failed ? "failed" : "stopped" }
            return run
        }
    }
    mutating func stopActiveCodeRuns(_ target: MessageState) {
        codeRuns = codeRuns?.map { run in
            var run = run
            if run.state == "running" { run.state = target == .failed ? "failed" : "stopped"; run.message = L10n.tr("执行连接已结束，未收到完整结果。") }
            return run
        }
    }
    mutating func recoverInterruptedReply() {
        guard state == .streaming else { return }
        // Never include the time the app was not running in a recovered clock.
        let lastReceived = reasoning?.lastReceivedAt ?? createdAt
        reasoning?.finish(.stopped, at: lastReceived)
        state = .stopped; failure = L10n.tr("上次生成已中断，内容已保留。")
        stopActiveCodeRuns(.stopped)
    }
}
