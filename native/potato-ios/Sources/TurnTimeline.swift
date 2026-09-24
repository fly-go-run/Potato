import SwiftUI

/// What a step did, so a group reads "读取 2 个文件、执行 3 条命令" rather than "5 个步骤".
enum ActivityKind: CaseIterable {
    case read, list, find, write, edit, command, web, memory, code, image, other

    init(tool name: String?) {
        switch name ?? "" {
        case "read_file", "read_skill": self = .read
        case "list_directory": self = .list
        case "grep_search", "glob_search": self = .find
        case "write_file", "append_file", "create_office_file", "fill_office_template": self = .write
        case "edit_file": self = .edit
        case "execute_shell_command", "exec_command", "shell": self = .command
        case "web_search": self = .web
        case "memory_search", "memory_write", "remember", "forget_memory", "recall_history": self = .memory
        case "generate_image_gpt", "edit_image": self = .image
        default: self = .other
        }
    }

    func phrase(_ count: Int) -> String {
        let one = count == 1
        switch self {
        case .read: return one ? L10n.tr("读取 1 个文件") : L10n.tr("读取 \(count) 个文件")
        case .list: return one ? L10n.tr("查看 1 个目录") : L10n.tr("查看 \(count) 个目录")
        case .find: return one ? L10n.tr("搜索 1 次文件") : L10n.tr("搜索 \(count) 次文件")
        case .write: return one ? L10n.tr("写入 1 个文件") : L10n.tr("写入 \(count) 个文件")
        case .edit: return one ? L10n.tr("编辑 1 个文件") : L10n.tr("编辑 \(count) 个文件")
        case .command: return one ? L10n.tr("执行 1 条命令") : L10n.tr("执行 \(count) 条命令")
        case .web: return one ? L10n.tr("搜索 1 次网页") : L10n.tr("搜索 \(count) 次网页")
        case .memory: return one ? L10n.tr("查阅 1 次记忆") : L10n.tr("查阅 \(count) 次记忆")
        case .code: return one ? L10n.tr("运行 1 段代码") : L10n.tr("运行 \(count) 段代码")
        case .image: return one ? L10n.tr("生成 1 次图片") : L10n.tr("生成 \(count) 次图片")
        case .other: return one ? L10n.tr("调用 1 个工具") : L10n.tr("调用 \(count) 个工具")
        }
    }
}

extension Array where Element == ActivityStep {
    /// Names what the tools did, in the order they first happened. File steps
    /// count distinct files; thinking is only named when nothing else ran.
    var actionSummary: String {
        let tools = filter { !$0.reasoning }
        guard !tools.isEmpty else {
            guard let thought = last(where: \.reasoning) else { return "" }
            if let seconds = thought.duration.map({ Swift.max(1, Int($0.rounded())) }), !thought.isRunning { return L10n.tr("已思考 \(seconds) 秒") }
            return L10n.tr("思考过程")
        }
        var order: [ActivityKind] = []
        var counts: [ActivityKind: Int] = [:]
        var targets: [ActivityKind: Set<String>] = [:]
        for step in tools {
            if !order.contains(step.kind) { order.append(step.kind) }
            if let target = step.target, [.read, .write, .edit].contains(step.kind) {
                if targets[step.kind, default: []].insert(target).inserted { counts[step.kind, default: 0] += 1 }
            } else { counts[step.kind, default: 0] += 1 }
        }
        return ActivityKind.join(order.map { $0.phrase(counts[$0] ?? 1) })
    }
}

extension ActivityKind {
    /// "读取 1 个文件、执行 3 条命令" / "Read a file, ran 3 commands".
    static func join(_ phrases: [String]) -> String {
        guard AppLocalization.shared.selection.resolved() == .english else { return phrases.joined(separator: "、") }
        return phrases.enumerated().map { index, phrase in
            index == 0 ? phrase : phrase.prefix(1).lowercased() + phrase.dropFirst()
        }.joined(separator: ", ")
    }
}

/// One reply, in the order it happened: commentary, the tools it led to, more commentary.
enum TurnSegment: Identifiable {
    case text(id: String, text: String)
    case activity(id: String, steps: [ActivityStep])
    var id: String {
        switch self { case .text(let id, _), .activity(let id, _): id }
    }
}

extension ChatMessage {
    /// Legacy replies carry no anchors and keep one group above the text.
    var turnSegments: [TurnSegment] {
        let steps = activitySteps
        let text = displayText
        let anchors = (selectedVersion != nil ? selectedVersion?.activityAnchors : activityAnchors) ?? [:]
        guard !steps.isEmpty else { return text.isEmpty ? [] : [.text(id: "text:0", text: text)] }
        guard !anchors.isEmpty else {
            return [.activity(id: "activity:\(steps[0].id)", steps: steps)] + (text.isEmpty ? [] : [.text(id: "text:0", text: text)])
        }
        let units = Array(text.utf16)
        var segments: [TurnSegment] = []
        var group: [ActivityStep] = []
        var cut = 0
        func flushGroup() {
            guard let first = group.first else { return }
            segments.append(.activity(id: "activity:\(first.id)", steps: group)); group = []
        }
        for step in steps {
            let anchor = min(units.count, max(cut, anchors[step.id] ?? cut))
            if anchor > cut, Self.isSafeBreak(String(decoding: units[..<anchor], as: UTF16.self)) {
                let chunk = String(decoding: units[cut..<anchor], as: UTF16.self).trimmingCharacters(in: .whitespacesAndNewlines)
                if !chunk.isEmpty {
                    flushGroup()
                    segments.append(.text(id: "text:\(cut)", text: chunk))
                    cut = anchor
                }
            }
            group.append(step)
        }
        flushGroup()
        let rest = String(decoding: units[cut...], as: UTF16.self).trimmingCharacters(in: .whitespacesAndNewlines)
        if !rest.isEmpty { segments.append(.text(id: "text:\(cut)", text: rest)) }
        return segments
    }

    /// Never split inside a code fence; the tool then joins the next break.
    static func isSafeBreak(_ prefix: String) -> Bool {
        prefix.components(separatedBy: "```").count % 2 == 1
    }
}

/// The single line under a reply in progress: a pulsing dot and what is happening now.
struct TurnStatusLine: View {
    let title: String
    var animated = true
    var identifier = "turn-status"
    var body: some View {
        HStack(spacing: 8) {
            ReplyPendingDot()
            // While the answer itself streams, the dot alone says it is still going.
            if !title.isEmpty { Text(title).font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(1).shimmering(animated) }
        }.frame(minHeight: 28, alignment: .leading)
            .accessibilityElement(children: .ignore).accessibilityLabel(title.isEmpty ? L10n.tr("正在生成") : title).accessibilityIdentifier(identifier)
    }
}

extension ChatMessage {
    /// A streaming thought is named by its own activity row; a status line would only repeat it.
    var showsStatusLine: Bool {
        guard state == .streaming else { return false }
        guard cloudReply?.notice == nil, let trace = displayReasoning else { return true }
        return trace.state != .streaming || trace.text.isEmpty
    }
    /// What the in-progress reply is doing, for the status line.
    var phaseTitle: String {
        if let notice = cloudReply?.notice { return notice }
        if displayReasoning?.state == .streaming { return L10n.tr("正在思考") }
        let steps = activitySteps
        if steps.contains(where: { !$0.reasoning && $0.isRunning }) { return L10n.tr("正在执行") }
        if !text.isEmpty { return "" }
        return steps.isEmpty ? L10n.tr("正在准备回复") : L10n.tr("正在整理结果")
    }
}
