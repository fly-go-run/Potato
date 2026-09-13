import SwiftUI

struct RecallView: View {
    @ObservedObject var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    @State private var editing: PersonalMemory?
    @State private var text = ""
    @State private var editorVisible = false
    @State private var busy = false
    @State private var failure: String?
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("跨对话检索", isOn: Binding(get: { store.settings.recallEnabled == true }, set: { store.settings.recallEnabled = $0; store.persist(); if $0 { store.syncRecallInBackground() } }))
                    Toggle("自动记录长期记忆", isOn: Binding(get: { store.settings.automaticMemory == true }, set: { store.settings.automaticMemory = $0; store.persist() })).disabled(store.settings.recallEnabled != true)
                } footer: { Text("开启后，消息文字会同步到云端，并交给 E2B 按需检索。附件、示例和排除的对话不参与。关闭后停止本机检索与新增同步，删除和排除仍会同步，已保存的其他云端历史保留。自动记忆保存你明确表达的长期偏好，也可以手动添加。") }
                Section {
                    if let notice = store.recallNotice { Text(notice).font(.footnote).foregroundStyle(.secondary) }
                    Button { store.syncRecallInBackground(cleanup: true) } label: { HStack { Text("立即同步历史"); if store.recallSyncing { Spacer(); ProgressView() } } }.disabled(store.recallSyncing || store.settings.recallEnabled == nil || store.settings.demo)
                } footer: { Text("发送消息前会确认历史同步完成；离线、同步冲突或服务未配置时会提示重试。关键词检索可能需要换词，不保证找全所有语义相近的内容。") }
                memorySection
                conversationSection
            }
            .navigationTitle("记忆与历史").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("完成") { dismiss() } } }
            .task { if !store.settings.demo { await store.refreshMemories() } }
            .sheet(isPresented: $editorVisible) {
                memoryEditor
            }
            .alert("记忆操作未完成", isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) { Button("好") { failure = nil } } message: { Text(failure ?? "") }
        }
    }
    private var memorySection: some View {
                Section {
                    if store.visibleMemories.isEmpty { Text("还没有保存的记忆").foregroundStyle(.secondary) }
                    ForEach(store.visibleMemories) { memory in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(memory.text)
                            if !memory.sources.isEmpty { Text("来自：" + Set(memory.sources.map(\.title)).sorted().joined(separator: "、")).font(.caption).foregroundStyle(.secondary) }
                            HStack {
                                Button("编辑") { editing = memory; text = memory.text; editorVisible = true }
                                Spacer()
                                Button("忘记", role: .destructive) { Task { busy = true; defer { busy = false }; do { try await store.saveMemory(memory, text: "", forget: true) } catch { failure = error.localizedDescription } } }
                            }.buttonStyle(.borderless).disabled(busy)
                        }
                    }
                    Button("添加记忆", systemImage: "plus") { editing = nil; text = ""; editorVisible = true }.disabled(store.settings.demo)
                } header: { Text("长期记忆") } footer: { Text("忘记后，该条自动记忆的原始消息不会再次用于自动记忆；原始对话仍可被历史检索，需彻底排除时请关闭下方对应对话。") }

    }
    private var conversationSection: some View {
                Section {
                    ForEach(store.visibleConversations.filter { !$0.isExample && !$0.messages.isEmpty }) { chat in
                        Toggle(chat.title, isOn: Binding(get: { store.conversations.first(where: { $0.id == chat.id })?.recallExcluded != true }, set: { store.setRecallExcluded(!$0, conversation: chat.id) }))
                    }
                } header: { Text("参与检索的对话") } footer: { Text("关闭某段对话后，会同步移除其云端内容和来源记忆。删除对话也会移除；网络不可用时需联网重试同步。") }
    }
    private var memoryEditor: some View {
                NavigationStack {
                    TextEditor(text: $text).padding().navigationTitle(editing == nil ? "添加记忆" : "编辑记忆")
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) { Button("取消") { editorVisible = false } }
                            ToolbarItem(placement: .confirmationAction) { Button("保存") { Task { busy = true; defer { busy = false }; do { try await store.saveMemory(editing, text: text); editorVisible = false } catch { failure = error.localizedDescription } } }.disabled(busy || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || text.count > 1500) }
                        }
                }
    }

}

struct RecallSourcesView: View {
    var runs: [RecallRun]
    @ObservedObject var store: WorkspaceStore
    @State private var expanded = false
    private var sources: [RecallSource] {
        var seen = Set<String>()
        return runs.flatMap(\.sources).filter { source in
            let local = store.conversations.first { $0.id.uuidString.lowercased() == source.conversation }
            return local?.deletedAt == nil && local?.recallExcluded != true && seen.insert(source.identity).inserted
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if runs.last?.state == "searching" { HStack { ProgressView().controlSize(.small); Text("正在检索历史与记忆…").font(.caption) } }
            if let failed = runs.last(where: { $0.state == "failed" }) { Text(failed.message ?? "历史检索未完成").font(.caption).foregroundStyle(.secondary) }
            if !sources.isEmpty {
                DisclosureGroup("检索到的历史 · \(sources.count) 条", isExpanded: $expanded) {
                    ForEach(sources, id: \.identity) { source in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(source.title).font(.subheadline.bold())
                            Text(String(source.date.prefix(10)) + " · " + (source.role == "user" ? "你" : "助手")).font(.caption).foregroundStyle(.secondary)
                            Text(source.text).font(.footnote).lineLimit(5).textSelection(.enabled)
                            Button("查看原对话", systemImage: "arrow.up.forward") { store.openRecallSource(source) }.font(.footnote)
                        }.padding(.vertical, 8)
                    }
                }.font(.footnote)
            }
        }
    }
}
