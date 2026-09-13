import SwiftUI

struct SandboxRunSheet: View {
    let code: String
    let attachments: [Attachment]
    @ObservedObject var store: WorkspaceStore
    let messageID: UUID
    @State private var selected: Set<UUID> = []
    @State private var running = false
    @State private var failure: String?
    @State private var task: Task<Void, Never>?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                Section {
                    Label("运行 Python", systemImage: "chevron.left.forwardslash.chevron.right").font(.headline)
                    Text("这段代码将在独立的云端环境运行，可用于分析文件、生成图表和文档。最多运行 60 秒。只有勾选的文件会上传。").font(.footnote).foregroundStyle(Palette.secondary)
                    DisclosureGroup("查看代码") { Text(code).font(.system(.footnote, design: .monospaced)).textSelection(.enabled) }
                }
                if !attachments.isEmpty {
                    Section("输入文件 · 合计最多 2 MB") {
                        ForEach(attachments) { file in
                            Button { if selected.contains(file.id) { selected.remove(file.id) } else if selected.count < 4 { selected.insert(file.id) } } label: {
                                HStack { Image(systemName: selected.contains(file.id) ? "checkmark.circle.fill" : "circle"); Text(file.name); Spacer() }
                            }.disabled(running).accessibilityValue(selected.contains(file.id) ? "已选择" : "未选择")
                        }
                        ForEach(Array(chosen.enumerated()), id: \.element.id) { index, file in
                            Text("\(file.name) → /home/user/\(SandboxService.filename(file, index: index))").font(.caption).textSelection(.enabled)
                        }
                    }
                }
                Section {
                    Text("文档请保存到 /home/user/output；图表可以直接显示。运行结束后，结果会保存在这条回复下。").font(.footnote).foregroundStyle(Palette.secondary)
                    if running { HStack { ProgressView(); Text("正在云端计算…") }.accessibilityIdentifier("sandbox-running") }
                    if let failure { Text(failure).foregroundStyle(.red).accessibilityIdentifier("sandbox-error") }
                    Button(running ? "停止等待" : "运行代码") { if running { task?.cancel(); running = false } else { run() } }.disabled(store.generatingID != nil).accessibilityIdentifier("run-sandbox")
                }
            }.navigationTitle("云端计算").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("完成") { dismiss() } } }
        }.onDisappear { task?.cancel() }
    }
    private var chosen: [Attachment] { attachments.filter { selected.contains($0.id) } }
    private func run() {
        failure = nil; running = true
        let conversationID = store.selectedID
        task = Task { @MainActor in
            defer { running = false }
            do {
                let request = try SandboxService.request(code: code, files: chosen, settings: store.settings, token: store.connectionToken, storage: store.storage)
                let execution = try await SandboxService.run(request)
                try Task.checkCancellation()
                try store.saveExecution(execution, messageID: messageID, conversationID: conversationID)
                dismiss()
            } catch { if !Task.isCancelled { failure = ChatService.failureDescription(error) } }
        }
    }
}
