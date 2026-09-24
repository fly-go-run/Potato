import SwiftUI

struct RecallView: View {
    @ObservedObject var store: WorkspaceStore
    @State private var editing: PersonalMemory?
    @State private var text = ""
    @State private var editorVisible = false
    @State private var busy = false
    @State private var failure: String?
    var body: some View {
            Form {
                Section {
                    Toggle(L10n.tr("参考历史对话"), isOn: Binding(get: { store.settings.recallEnabled == true }, set: { store.settings.recallEnabled = $0; store.persist(); if $0 { store.syncRecallInBackground() } }))
                    Toggle(L10n.tr("自动记住重要信息"), isOn: Binding(get: { store.settings.automaticMemory == true }, set: { store.settings.automaticMemory = $0; store.persist() })).disabled(store.settings.recallEnabled != true)
                } footer: { Text(L10n.tr("开启后，对话文字会同步到你的账号，供回答时参考。")) }
                if store.settings.recallEnabled != nil {
                Section {
                    if let notice = store.recallNotice { Text(notice).font(.footnote).foregroundStyle(.secondary) }
                    Button { store.syncRecallInBackground(cleanup: true) } label: { HStack { Text(L10n.tr("立即同步历史")); if store.recallSyncing { Spacer(); ProgressView() } } }.disabled(store.recallSyncing || store.settings.recallEnabled == nil || store.settings.demo)
                }
                }
                memorySection
                if store.settings.recallEnabled == true { conversationSection }
            }
            .navigationTitle(L10n.tr("记忆")).navigationBarTitleDisplayMode(.inline)
            .task { if !store.settings.demo { await store.refreshMemories() } }
            .sheet(isPresented: $editorVisible) {
                memoryEditor
            }
            .alert(L10n.tr("记忆操作未完成"), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) { Button(L10n.tr("好")) { failure = nil } } message: { Text(failure ?? "") }
    }
    private var memorySection: some View {
                Section {
                    if store.visibleMemories.isEmpty { Text(L10n.tr("还没有保存的记忆")).foregroundStyle(.secondary) }
                    ForEach(store.visibleMemories) { memory in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(memory.text)
                            if !memory.sources.isEmpty { Text(L10n.tr("来自：") + Set(memory.sources.map(\.title)).sorted().joined(separator: "、")).font(.caption).foregroundStyle(.secondary) }
                            HStack {
                                Button(L10n.tr("编辑")) { editing = memory; text = memory.text; editorVisible = true }
                                Spacer()
                                Button(L10n.tr("忘记"), role: .destructive) { Task { busy = true; defer { busy = false }; do { try await store.saveMemory(memory, text: "", forget: true) } catch { failure = error.localizedDescription } } }
                            }.buttonStyle(.borderless).disabled(busy)
                        }
                    }
                    Button(L10n.tr("添加记忆"), systemImage: "plus") { editing = nil; text = ""; editorVisible = true }.disabled(store.settings.demo)
                } header: { Text(L10n.tr("已保存的记忆")) }

    }
    private var conversationSection: some View {
                Section {
                    ForEach(store.visibleConversations.filter { !$0.isExample && !$0.messages.isEmpty }) { chat in
                        Toggle(chat.displayTitle, isOn: Binding(get: { store.conversations.first(where: { $0.id == chat.id })?.recallExcluded != true }, set: { store.setRecallExcluded(!$0, conversation: chat.id) }))
                    }
                } header: { Text(L10n.tr("可被参考的对话")) }
    }
    private var memoryEditor: some View {
                NavigationStack {
                    TextEditor(text: $text).padding().navigationTitle(editing == nil ? L10n.tr("添加记忆") : L10n.tr("编辑记忆"))
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { editorVisible = false } }
                            ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("保存")) { Task { busy = true; defer { busy = false }; do { try await store.saveMemory(editing, text: text); editorVisible = false } catch { failure = error.localizedDescription } } }.disabled(busy || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || text.count > 1500) }
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
            if runs.last?.state == "searching" { Text(L10n.tr("正在检索历史与记忆")).font(.caption).foregroundStyle(Palette.secondary).shimmering() }
            if let failed = runs.last(where: { $0.state == "failed" }) { Text(failed.message ?? L10n.tr("历史检索未完成")).font(.caption).foregroundStyle(.secondary) }
            if !sources.isEmpty {
                DisclosureGroup(L10n.tr("参考了 \(sources.count) 段历史"), isExpanded: $expanded) {
                    ForEach(sources, id: \.identity) { source in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(source.title).font(.subheadline.bold())
                            Text(String(source.date.prefix(10)) + " · " + (source.role == "user" ? L10n.tr("你") : L10n.tr("助手"))).font(.caption).foregroundStyle(.secondary)
                            Text(source.text).font(.footnote).lineLimit(5).textSelection(.enabled)
                            Button(L10n.tr("查看原对话"), systemImage: "arrow.up.forward") { store.openRecallSource(source) }.font(.footnote)
                        }.padding(.vertical, 8)
                    }
                }.font(.footnote)
            }
        }
    }
}
