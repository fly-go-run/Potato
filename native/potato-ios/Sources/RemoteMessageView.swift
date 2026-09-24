import SwiftUI

extension RemoteMessage {
    var isProcess: Bool { role != "user" && (kind == "reasoning" || kind?.contains("call") == true || role == "tool") }
    var isNotice: Bool { role == "system" || kind == "notice" }
    var isStreaming: Bool { status == "running" || status == "in_progress" }
    /// The structured name, or the legacy first line of the call frame.
    var toolName: String { name.flatMap { $0.isEmpty ? nil : $0 } ?? String(text.split(separator: "\n", maxSplits: 1).first ?? "") }
    var processTitle: String {
        if kind == "reasoning" { return isStreaming ? L10n.tr("正在思考") : L10n.tr("思考过程") }
        if role == "tool" || kind?.contains("output") == true { return L10n.tr("执行结果") }
        let name = toolName
        let titles = ["read_file": L10n.tr("读取文件"), "write_file": L10n.tr("写入文件"), "append_file": L10n.tr("写入文件"), "edit_file": L10n.tr("编辑文件"),
                      "execute_shell_command": L10n.tr("执行命令"), "exec_command": L10n.tr("执行命令"), "shell": L10n.tr("执行命令"),
                      "list_directory": L10n.tr("查看目录"), "grep_search": L10n.tr("搜索文件"), "glob_search": L10n.tr("搜索文件"),
                      "web_search": L10n.tr("搜索网页"), "memory_search": L10n.tr("查阅记忆"), "read_skill": L10n.tr("读取技能指南")]
        if let title = titles[name] { return title }
        // Prefer the structured name, falling back to the legacy first line. Only
        // surface an identifier, never use arbitrary log text as a title.
        if !name.isEmpty && name.count < 60 && name.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_" || $0 == "." || $0 == "-") }) { return L10n.tr("调用 \(name)") }
        return L10n.tr("执行操作")
    }
}

/// Present a reply as one conversation turn, even when the desktop interleaves
/// reasoning, tools and commentary. User messages and notices remain boundaries.
/// Keep every frame and the first ID so polling doesn't replace the visible turn.
struct RemoteConversationRow: Identifiable {
    let id: String
    var messages: [RemoteMessage]
    var isAssistantTurn: Bool { messages.first.map { $0.role != "user" && !$0.isNotice } ?? false }
    var processMessages: [RemoteMessage] { messages.filter(\.isProcess) }
    var answerMessages: [RemoteMessage] { messages.filter { !$0.isProcess && !$0.isNotice && $0.role != "user" && !$0.text.isEmpty } }
    var replyText: String { answerMessages.map(\.text).joined(separator: "\n\n") }
    /// The reply in the order it happened: commentary, the tools it led to,
    /// more commentary. Adjacent process frames form one group, keyed by its first frame.
    var segments: [RemoteSegment] {
        var result: [RemoteSegment] = []
        for message in messages where message.role != "user" && !message.isNotice {
            if message.isProcess {
                if case .process(var group) = result.last { group.append(message); result[result.count - 1] = .process(group) }
                else { result.append(.process([message])) }
            } else if !message.text.isEmpty { result.append(.text(message)) }
        }
        return result
    }
    static func make(_ messages: [RemoteMessage]) -> [Self] {
        var rows: [Self] = []
        for message in messages {
            if message.role != "user", !message.isNotice, rows.last?.isAssistantTurn == true {
                rows[rows.count - 1].messages.append(message)
            } else { rows.append(Self(id: message.id, messages: [message])) }
        }
        return rows
    }
}

enum RemoteSegment: Identifiable {
    case text(RemoteMessage)
    case process([RemoteMessage])
    var id: String {
        switch self {
        case .text(let message): message.id
        case .process(let messages): "process:" + (messages.first?.id ?? "")
        }
    }
}

struct RemoteConversationRowView: View {
    let row: RemoteConversationRow
    let running: Bool
    var confirmed = true
    var activeProcessID: String? = nil
    var onExpand: () -> Void = {}
    var body: some View {
        if let message = row.messages.first {
            if message.role == "user" {
                HStack {
                    Spacer(minLength: 36)
                    Text(message.text).textSelection(.enabled)
                        .padding(16)
                        .background(Palette.muted, in: RoundedRectangle(cornerRadius: 22))
                        .accessibilityIdentifier("remote-user-\(message.id)")
                        .contextMenu { Button(L10n.tr("复制消息"), systemImage: "doc.on.doc") { UIPasteboard.general.string = message.text } }
                }.frame(maxWidth: .infinity, alignment: .trailing)
            } else if message.isNotice {
                Label(message.text, systemImage: "info.circle").font(.footnote).foregroundStyle(.secondary)
            } else {
                let segments = row.segments
                let lastProcess = segments.lastIndex { if case .process = $0 { true } else { false } }
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(Array(segments.enumerated()), id: \.element.id) { index, segment in
                        let last = index == segments.count - 1
                        switch segment {
                        case .process(let messages):
                            // Only the newest group can still be working; earlier ones settle into summaries.
                            RemoteProcessView(messages: messages, running: running && last, confirmed: confirmed, activeProcessID: activeProcessID,
                                              replyStarted: !last, identifier: index == lastProcess ? "remote-process-toggle" : "remote-process-toggle-\(index)", onExpand: onExpand)
                        case .text(let answer):
                            // Polls land every two seconds; pace the newest text across the gap.
                            StreamingMarkdown(text: answer.text, streaming: running && last, pace: 1.6, codeBackground: Palette.muted, codeBorder: Palette.line)
                                .tint(.blue)
                        }
                    }
                    if !row.replyText.isEmpty && !running {
                        RemoteReplyActions(text: row.replyText)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}

private struct RemoteProcessView: View {
    let messages: [RemoteMessage]
    let running: Bool
    let confirmed: Bool
    let activeProcessID: String?
    var replyStarted = false
    var identifier = "remote-process-toggle"
    let onExpand: () -> Void
    var body: some View {
        ActivitySummaryView(steps: ActivityStep.remote(messages, running: running, confirmed: confirmed, activeID: activeProcessID),
                            running: running, confirmed: confirmed, replyStarted: replyStarted, identifier: identifier, onExpand: onExpand)
    }
}

private struct RemoteReplyActions: View {
    let text: String
    @State private var copied = false
    @State private var selecting = false
    var body: some View {
        HStack(spacing: 0) {
            Button {
                UIPasteboard.general.string = text; copied = true
            } label: { Image(systemName: copied ? "checkmark" : "doc.on.doc").font(.system(size: 18)).frame(width: 44, height: 44).contentShape(Rectangle()) }
                .accessibilityLabel(copied ? L10n.tr("已复制回复") : L10n.tr("复制回复")).accessibilityIdentifier("remote-copy-reply")
            Menu {
                Button(L10n.tr("选择文字"), systemImage: "text.cursor") { selecting = true }.accessibilityIdentifier("remote-select-reply")
                ShareLink(item: text) { Label(L10n.tr("分享回复"), systemImage: "square.and.arrow.up") }.accessibilityIdentifier("remote-share-reply")
            } label: { Image(systemName: "ellipsis").font(.system(size: 18)).frame(width: 44, height: 44).contentShape(Rectangle()) }
                .accessibilityLabel(L10n.tr("回复更多操作")).accessibilityIdentifier("remote-reply-more")
            Spacer(minLength: 0)
        }.buttonStyle(.plain).foregroundStyle(Palette.secondary)
            .onChange(of: text) { _, _ in copied = false }
            .task(id: copied) { if copied { try? await Task.sleep(for: .seconds(2)); if !Task.isCancelled { copied = false } } }
            .sheet(isPresented: $selecting) { ReplyTextSelection(text: text) }
    }
}
