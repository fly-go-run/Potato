import SwiftUI

extension RemoteMessage {
    var isProcess: Bool { role != "user" && (kind == "reasoning" || kind?.contains("call") == true || role == "tool") }
    var isNotice: Bool { role == "system" || kind == "notice" }
    var isStreaming: Bool { status == "running" || status == "in_progress" }
    var processTitle: String {
        if kind == "reasoning" { return isStreaming ? "正在思考" : "思考过程" }
        if role == "tool" || kind?.contains("output") == true { return "执行结果" }
        let name = String(text.split(separator: "\n", maxSplits: 1).first ?? "")
        let titles = ["read_file": "读取文件", "write_file": "写入文件", "edit_file": "编辑文件", "exec_command": "执行命令", "shell": "执行命令", "list_directory": "查看目录", "web_search": "搜索网页"]
        if let title = titles[name] { return title }
        // The legacy protocol carries the tool name as the first line. Only
        // surface an identifier, never use arbitrary log text as a title.
        if !name.isEmpty && name.count < 60 && name.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_" || $0 == "." || $0 == "-") }) { return "调用 \(name)" }
        return "执行操作"
    }
}

/// Consecutive process frames share one disclosure, without crossing a user
/// message, an assistant answer or a system notice. Frame IDs survive polling.
struct RemoteConversationRow: Identifiable {
    let id: String
    var messages: [RemoteMessage]
    var isProcess: Bool { messages.first?.isProcess == true }
    static func make(_ messages: [RemoteMessage]) -> [Self] {
        var rows: [Self] = []
        for message in messages {
            if message.isProcess, rows.last?.isProcess == true {
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
        if row.isProcess {
            RemoteProcessView(messages: row.messages, running: running, confirmed: confirmed, activeProcessID: activeProcessID, onExpand: onExpand)
        } else if let message = row.messages.first {
            if message.role == "user" {
                HStack {
                    Spacer(minLength: 36)
                    Text(message.text).textSelection(.enabled)
                        .padding(.horizontal, 16).padding(.vertical, 12)
                        .background(Color(white: 0.95), in: RoundedRectangle(cornerRadius: 22))
                        .accessibilityIdentifier("remote-user-\(message.id)")
                        .contextMenu { Button("复制消息", systemImage: "doc.on.doc") { UIPasteboard.general.string = message.text } }
                }.frame(maxWidth: .infinity, alignment: .trailing)
            } else if message.isNotice {
                Label(message.text, systemImage: "info.circle").font(.footnote).foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    MarkdownContent(text: message.text, codeBackground: Color(white: 0.965), codeBorder: Color.black.opacity(0.08))
                        .tint(.blue)
                    if !message.text.isEmpty && (!running || !message.isStreaming) {
                        RemoteReplyActions(text: message.text)
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
    @State private var expanded = false
    private var title: String {
        if messages.allSatisfy({ $0.kind == "reasoning" }) { return "思考过程" }
        return "思考与执行"
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button { expanded.toggle(); if expanded { onExpand() } } label: {
                HStack(spacing: 8) {
                    Image(systemName: "text.line.first.and.arrowtriangle.forward")
                    Text(title).font(.subheadline)
                    Text("\(messages.count) 项").font(.caption)
                    Image(systemName: expanded ? "chevron.down" : "chevron.right").font(.caption)
                    Spacer(minLength: 0)
                }.foregroundStyle(.secondary).frame(minHeight: 44).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("remote-process-toggle")
                .accessibilityValue(expanded ? "已展开" : "已折叠")
            if expanded {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(messages) { message in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(message.kind == "reasoning" ? "思考过程" : message.processTitle).font(.subheadline.weight(.medium))
                                Spacer()
                                if message.status == "failed" { Text("失败").foregroundStyle(.red) }
                                else if message.isStreaming { Text(!confirmed ? "状态待确认" : !running ? "已结束" : message.id == activeProcessID ? "进行中" : "过程记录") }
                            }.font(.caption).foregroundStyle(.secondary)
                            if message.kind == "reasoning" {
                                Text(message.text.isEmpty ? "尚未收到思考文字" : message.text)
                                    .font(.subheadline).foregroundStyle(.secondary).textSelection(.enabled)
                                    .fixedSize(horizontal: false, vertical: true)
                            } else {
                                DisclosureGroup("查看详情") {
                                    Text(message.text.isEmpty ? "暂无文字记录" : message.text)
                                        .font(.system(.footnote, design: .monospaced)).textSelection(.enabled)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .fixedSize(horizontal: false, vertical: true)
                                }.font(.subheadline).tint(.secondary)
                            }
                        }
                    }
                }.padding(.leading, 14).overlay(alignment: .leading) { Rectangle().fill(Color.black.opacity(0.1)).frame(width: 2) }
            }
        }
    }
}

private struct RemoteReplyActions: View {
    let text: String
    @State private var copied = false
    @State private var selecting = false
    var body: some View {
        HStack(spacing: 2) {
            IconButton(symbol: copied ? "checkmark" : "doc.on.doc", label: copied ? "已复制回复" : "复制回复", id: "remote-copy-reply") {
                UIPasteboard.general.string = text; copied = true
            }
            IconButton(symbol: "text.cursor", label: "选择文字", id: "remote-select-reply") { selecting = true }
            ShareLink(item: text) { Image(systemName: "square.and.arrow.up").font(.system(size: 20)).frame(width: 44, height: 44) }
                .accessibilityLabel("分享回复").accessibilityIdentifier("remote-share-reply")
            Spacer(minLength: 0)
        }.buttonStyle(.plain).foregroundStyle(.secondary)
            .onChange(of: text) { _, _ in copied = false }
            .task(id: copied) { if copied { try? await Task.sleep(for: .seconds(2)); if !Task.isCancelled { copied = false } } }
            .sheet(isPresented: $selecting) { ReplyTextSelection(text: text) }
    }
}
