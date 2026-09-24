import SwiftUI

extension RemoteMessage {
    var isProcess: Bool { role != "user" && (kind == "reasoning" || kind?.contains("call") == true || role == "tool") }
    var isNotice: Bool { role == "system" || kind == "notice" }
    var isStreaming: Bool { status == "running" || status == "in_progress" }
    var processTitle: String {
        if kind == "reasoning" { return isStreaming ? L10n.tr("正在思考") : L10n.tr("思考过程") }
        if role == "tool" || kind?.contains("output") == true { return L10n.tr("执行结果") }
        let name = self.name.flatMap { $0.isEmpty ? nil : $0 } ?? String(text.split(separator: "\n", maxSplits: 1).first ?? "")
        let titles = ["read_file": L10n.tr("读取文件"), "write_file": L10n.tr("写入文件"), "edit_file": L10n.tr("编辑文件"), "exec_command": L10n.tr("执行命令"), "shell": L10n.tr("执行命令"), "list_directory": L10n.tr("查看目录"), "web_search": L10n.tr("搜索网页")]
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
                VStack(alignment: .leading, spacing: 10) {
                    if !row.processMessages.isEmpty {
                        RemoteProcessView(messages: row.processMessages, running: running, confirmed: confirmed, activeProcessID: activeProcessID, onExpand: onExpand)
                    }
                    ForEach(row.answerMessages) { answer in
                        MarkdownContent(text: answer.text, codeBackground: Palette.muted, codeBorder: Palette.line)
                            .tint(.blue)
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
    let onExpand: () -> Void
    @State private var showing = false
    private var steps: [ActivityStep] { ActivityStep.remote(messages, running: running, confirmed: confirmed, activeID: activeProcessID) }
    private var title: String {
        let tools = steps.filter { !$0.reasoning }.count
        let thought = steps.contains(where: \.reasoning)
        let summary = tools == 0 ? L10n.tr("思考过程") : thought ? L10n.tr("思考与 \(tools) 个步骤") : L10n.tr("\(tools) 个步骤")
        return confirmed ? summary : summary + L10n.tr(" · 状态待确认")
    }
    var body: some View {
        Button { onExpand(); showing = true } label: {
            HStack(spacing: 8) {
                Image(systemName: "clock.arrow.circlepath").font(.system(size: 15))
                Text(title).font(.subheadline).multilineTextAlignment(.leading)
                Image(systemName: "chevron.right").font(.system(size: 11, weight: .medium))
                Spacer(minLength: 0)
            }.foregroundStyle(Palette.secondary).frame(minHeight: 44).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier("remote-process-toggle")
            .accessibilityLabel(L10n.tr("\(title)，查看完整过程"))
            .sheet(isPresented: $showing) {
                ActivityProcessSheet(steps: steps, running: running, confirmed: confirmed, storage: nil)
                    .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
                    .presentationCornerRadius(32)
            }
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
            } label: { Image(systemName: copied ? "checkmark" : "square.on.square").font(.system(size: 18)).frame(width: 44, height: 44).contentShape(Rectangle()) }
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
