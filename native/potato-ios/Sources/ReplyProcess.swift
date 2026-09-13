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
            if run.state == "running" { run.state = target == .failed ? "failed" : "stopped"; run.message = "执行连接已结束，未收到完整结果。" }
            return run
        }
    }
    mutating func recoverInterruptedReply() {
        guard state == .streaming else { return }
        // Never include the time the app was not running in a recovered clock.
        let lastReceived = reasoning?.lastReceivedAt ?? createdAt
        reasoning?.finish(.stopped, at: lastReceived)
        state = .stopped; failure = "上次生成已中断，内容已保留。"
        stopActiveCodeRuns(.stopped)
    }
}

struct ReasoningProcessView: View {
    let trace: ReasoningTrace
    let reduceMotion: Bool
    var onExpand: () -> Void
    @State private var expanded = false
    @Environment(\.scenePhase) private var scenePhase
    private var active: Bool { trace.state == .streaming && scenePhase == .active }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if active {
                TimelineView(.periodic(from: .now, by: 1)) { context in header(at: context.date) }
            } else { header(at: trace.lastReceivedAt ?? Date()) }
            if expanded {
                Text(trace.text).font(.subheadline).foregroundStyle(Palette.secondary)
                    .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("reasoning-content")
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
    private func header(at now: Date) -> some View {
        Button { expanded.toggle(); if expanded { onExpand() } } label: {
            HStack(spacing: 8) {
                if active && !reduceMotion { ProgressView().controlSize(.mini).accessibilityHidden(true) }
                else { Image(systemName: trace.state == .complete ? "checkmark" : "ellipsis").accessibilityHidden(true) }
                Text(title).font(.subheadline)
                Text(duration(at: now)).font(.caption).monospacedDigit()
                Image(systemName: expanded ? "chevron.up" : "chevron.down").font(.caption2).accessibilityHidden(true)
                Spacer(minLength: 0)
            }.foregroundStyle(Palette.secondary).frame(minHeight: 44).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityLabel("\(title)，\(duration(at: now))，\(expanded ? "收起" : "展开")思考内容")
            .accessibilityValue(expanded ? "已展开" : "已收起").accessibilityIdentifier("reasoning-toggle")
    }
    private var title: String {
        switch trace.state {
        case .streaming: "正在思考"
        case .complete: "已思考"
        case .stopped: "思考已停止"
        case .failed: "思考已中断"
        }
    }
    private func duration(at now: Date) -> String {
        let seconds = Int(max(0, trace.duration(at: now)))
        return seconds < 1 ? "少于 1 秒" : "\(seconds) 秒"
    }
}
