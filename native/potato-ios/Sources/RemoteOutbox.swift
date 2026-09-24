import SwiftUI

enum RemoteDeliveryMode: String, Codable { case queue, interrupt }

struct RemoteOutbox: Decodable {
    struct Item: Decodable, Identifiable {
        let id: String
        let text: String
        let state: String
        let attachments: Int
    }
    var items: [Item]
    let paused: Bool
    let reason: String
    let interrupt: Bool
}

/// Pending messages are part of the conversation, with actions above each bubble.
struct RemoteQueuedMessages: View {
    let queue: RemoteOutbox?
    let pending: RemotePendingSend?
    let sending: Bool
    let delivered: Set<String>
    let enabled: Bool
    let action: (String, String, String?) async -> Bool
    @State private var editing: RemoteOutbox.Item?
    @State private var editText = ""
    @State private var editError = false
    @State private var saving = false

    private var items: [RemoteOutbox.Item] {
        RemoteOutbox.visibleItems(queue: queue, pending: pending, sending: sending, delivered: delivered)
    }
    private func status(_ item: RemoteOutbox.Item) -> String {
        if item.state == "sending" || item.state == "dispatching" { return L10n.tr("正在发送…") }
        if item.state == "unconfirmed" || item.state == "uncertain" { return L10n.tr("发送待确认") }
        if queue?.interrupt == true && item.id == queue?.items.first?.id { return L10n.tr("正在打断，随后发送…") }
        if queue?.paused == true { return L10n.tr("已暂停") }
        return item.state == "editing" ? L10n.tr("正在编辑") : L10n.tr("排队中")
    }

    var body: some View {
        VStack(alignment: .trailing, spacing: 22) {
            ForEach(items) { item in
                HStack(alignment: .top, spacing: 0) {
                    Spacer(minLength: 36)
                    VStack(alignment: .trailing, spacing: 0) {
                        Menu {
                            Button(L10n.tr("打断并发送"), systemImage: "arrow.turn.up.right") { Task { _ = await action("promote", item.id, nil) } }
                                .disabled(item.state != "pending")
                            Button(L10n.tr("编辑消息"), systemImage: "pencil") { editText = item.text; editError = false; editing = item }
                                .disabled(item.state != "pending")
                            Button(L10n.tr("删除"), systemImage: "trash", role: .destructive) { Task { _ = await action("delete", item.id, nil) } }
                        } label: {
                            HStack(spacing: 7) {
                                Image(systemName: "arrow.turn.down.right")
                                Text(status(item))
                                Image(systemName: "ellipsis")
                            }.font(.subheadline).foregroundStyle(Palette.secondary)
                                .frame(minHeight: 44).contentShape(Rectangle())
                        }.disabled(!enabled || queue?.interrupt == true || ["sending", "dispatching", "unconfirmed"].contains(item.state))
                            .accessibilityLabel("\(status(item))，\(item.text)")
                            .accessibilityIdentifier("remote-queue-item-\(item.id)")
                        VStack(alignment: .leading, spacing: 8) {
                            Text(item.text.isEmpty ? L10n.tr("附件消息") : item.text).textSelection(.enabled)
                                .fixedSize(horizontal: false, vertical: true)
                            if item.attachments > 0 { Label(L10n.tr("\(item.attachments) 个附件"), systemImage: "paperclip").font(.caption).foregroundStyle(Palette.secondary) }
                        }.padding(.horizontal, 16).padding(.vertical, 12)
                            .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 22))
                            .overlay(RoundedRectangle(cornerRadius: 22).strokeBorder(Palette.ink.opacity(0.09), lineWidth: 1))
                            .accessibilityIdentifier("remote-queued-bubble-\(item.id)")
                    }
                }.frame(maxWidth: .infinity, alignment: .trailing)
            }
            if queue?.paused == true && !items.isEmpty {
                VStack(alignment: .trailing, spacing: 4) {
                    Text(queue?.reason ?? L10n.tr("队列已暂停")).font(.caption).foregroundStyle(Palette.secondary)
                    Button(L10n.tr("继续发送")) { Task { _ = await action("resume", "", nil) } }.frame(minHeight: 44)
                        .disabled(!enabled).accessibilityIdentifier("remote-queue-resume")
                }
            }
        }
            .sheet(item: $editing) { item in
                NavigationStack {
                    VStack(alignment: .leading, spacing: 12) {
                        TextEditor(text: $editText).accessibilityIdentifier("remote-queue-editor")
                        if editError { Text(L10n.tr("未能保存，消息可能已开始发送。")).font(.footnote).foregroundStyle(.red) }
                    }.padding().navigationTitle(L10n.tr("编辑待发送消息")).navigationBarTitleDisplayMode(.inline)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { editing = nil } }
                            ToolbarItem(placement: .confirmationAction) {
                                Button(L10n.tr("保存")) {
                                    saving = true
                                    Task {
                                        if await action("save", item.id, editText) { editing = nil } else { editError = true }
                                        saving = false
                                    }
                                }.disabled(!enabled || saving || editText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            }
                        }
                }
            }
    }
}

extension RemoteOutbox {
    static func visibleItems(queue: RemoteOutbox?, pending: RemotePendingSend?, sending: Bool, delivered: Set<String>) -> [Item] {
        var items = (queue?.items ?? []).filter { !delivered.contains($0.id) }
        if let pending, pending.deliveryMode != nil, !delivered.contains(pending.id), !items.contains(where: { $0.id == pending.id }) {
            items.append(Item(id: pending.id, text: pending.text, state: sending ? "sending" : "unconfirmed", attachments: 0))
        }
        return items
    }
}
