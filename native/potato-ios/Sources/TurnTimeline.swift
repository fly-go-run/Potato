import SwiftUI

/// What a step did, so a group reads "读取文件、执行命令" rather than "5 个步骤".
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

    /// What was done, not how many times; the process sheet lists every step.
    /// English is past tense here, while the same Chinese words as step titles are imperative there.
    var phrase: String {
        let english = AppLocalization.shared.selection.resolved() == .english
        switch self {
        case .read: return english ? "Read files" : "读取文件"
        case .list: return english ? "Listed folders" : "查看目录"
        case .find: return english ? "Searched files" : "搜索文件"
        case .write: return english ? "Wrote files" : "写入文件"
        case .edit: return english ? "Edited files" : "编辑文件"
        case .command: return english ? "Ran commands" : "执行命令"
        case .web: return english ? "Searched the web" : "搜索网页"
        case .memory: return english ? "Checked memory" : "查阅记忆"
        case .code: return english ? "Ran code" : "运行代码"
        case .image: return english ? "Generated images" : "生成图片"
        case .other: return english ? "Used tools" : "调用工具"
        }
    }
}

extension Array where Element == ActivityStep {
    /// Names what the tools did, in the order they first happened; a group
    /// that only thought is "思考过程". No counts or seconds: that is sheet detail.
    var actionSummary: String {
        var order: [ActivityKind] = []
        for step in self where !step.reasoning && !order.contains(step.kind) { order.append(step.kind) }
        guard !order.isEmpty else { return contains(where: \.reasoning) ? L10n.tr("思考过程") : "" }
        return ActivityKind.join(order.map(\.phrase))
    }
}

extension ActivityKind {
    /// "读取文件、执行命令" / "Read files, ran commands".
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

/// The tail of a reply in progress. The dot stays in this one place from sending to the
/// last word, so hand-offs between thinking, tools and text never make it vanish or jump;
/// the rows above name the work. Text beside it is only for what no row can say.
struct TurnStatusLine: View {
    var title = ""
    /// What VoiceOver hears: the current phase, even when nothing is written.
    var label = ""
    var identifier = "turn-status"
    var body: some View {
        HStack(spacing: 8) {
            ReplyPendingDot()
            if !title.isEmpty { Text(title).font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(1).shimmering() }
        }.frame(minHeight: 28, alignment: .leading)
            .accessibilityElement(children: .ignore).accessibilityLabel(title.isEmpty ? label.isEmpty ? L10n.tr("正在生成") : label : title).accessibilityIdentifier(identifier)
    }
}

extension ChatMessage {
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
