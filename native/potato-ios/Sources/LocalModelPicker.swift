import SwiftUI

struct LocalModelPicker: View {
    @ObservedObject var store: WorkspaceStore
    let openConnection: () -> Void
    var retryMessage: ChatMessage? = nil
    var regenerate: ((LocalModelChoice) -> Bool)? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var page = ModelPickerPage.models
    @State private var query = ""
    @State private var manual = ""
    @State private var issue: String?
    @State private var fetching: Task<Void, Never>?
    @State private var pendingChoice: LocalModelChoice?
    private var choice: LocalModelChoice { pendingChoice ?? (retryMessage?.displayModelChoice ?? store.localModelChoice) }
    private var sameService: Bool { choice.endpoint == store.settings.serviceIdentity }
    private var cloud: Bool { store.settings.cloudAccount != nil }
    private var available: Bool { !cloud || allModels.contains(where: { $0.id == choice.model }) }
    private var entry: LocalModelEntry { store.settings.modelEntry(choice.model) }
    private var allModels: [LocalModelEntry] {
        var result = store.settings.currentCatalog?.models ?? []
        if !cloud && sameService && !choice.model.isEmpty && !result.contains(where: { $0.id == choice.model }) { result.insert(entry, at: 0) }
        return result
    }
    private var models: [LocalModelEntry] {
        allModels.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) || $0.id.localizedCaseInsensitiveContains(query) }
    }
    private var featured: [LocalModelEntry] {
        var values = Array(allModels.prefix(4))
        if sameService, let selected = allModels.first(where: { $0.id == choice.model }), !values.contains(where: { $0.id == selected.id }) {
            if values.count == 4 { values.removeLast() }; values.insert(selected, at: 0)
        }
        return values
    }
    var body: some View {
        ModelPickerSheet(page: $page, prefix: "local", compactHeight: min(660, (retryMessage == nil ? 300 : 430) + CGFloat(featured.count) * 64), rootTitle: retryMessage == nil ? nil : "换模型重新回答", close: { dismiss() }) {
            if !sameService { Text("此会话属于之前的服务，请重新选择模型。草稿会保留。").font(.footnote).foregroundStyle(.red).accessibilityIdentifier("local-model-service-changed") }
            if sameService && !available { Text("原模型已不在云端列表中，请选择一个模型重新回答。").font(.footnote).foregroundStyle(.secondary) }
            if let issue { Text(issue).font(.footnote).foregroundStyle(.red).accessibilityIdentifier("local-model-issue") }
            if fetching != nil { ProgressView("正在读取模型…") }
            switch page {
            case .models:
                Section {
                    ForEach(featured) { model in modelRow(model) }
                    if featured.isEmpty { Text(cloud ? "暂无可用模型，请刷新云端列表。" : "暂无可用模型，请在更多模型中添加。").foregroundStyle(.secondary) }
                }
                if sameService && available && !choice.model.isEmpty {
                    Section { ModelPickerDisclosure(title: "思考", value: choice.compactThinkingLabel, id: "local-thinking-open") { page = .thinking } }
                }
                Section { ModelPickerDisclosure(title: "更多模型", id: "local-more-models") { page = .more } }
            case .thinking:
                thinkingOptions
            case .more:
                Section {
                    TextField("搜索模型", text: $query).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("local-model-search")
                    ForEach(models) { model in modelRow(model) }
                    if models.isEmpty { Text("没有找到模型").foregroundStyle(.secondary) }
                }
                if !cloud { Section("手动指定") {
                    TextField("服务提供的模型名称", text: $manual).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("local-manual-model")
                    Button("使用此模型") {
                        choose(LocalModelChoice(endpoint: store.settings.serviceIdentity ?? "", model: manual.trimmingCharacters(in: .whitespacesAndNewlines)))
                        if issue == nil { query = ""; page = .models }
                    }.disabled(manual.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("local-use-manual-model")
                } }
                Section {
                    Button("刷新模型列表", systemImage: "arrow.clockwise", action: reload).disabled(fetching != nil).accessibilityIdentifier("local-model-refresh")
                    if retryMessage == nil { Button("连接设置", action: openConnection).accessibilityIdentifier("local-open-connection") }
                    if store.settings.demo { Text("本地体验模式，选择会保存但不调用模型。").font(.footnote).foregroundStyle(.secondary) }
                }
            }
            if let retryMessage {
                Section {
                    Button("重新回答") {
                        if regenerate?(choice) == true { dismiss() }
                        else { issue = store.error ?? "这条回复已改变，请关闭面板后重新选择。" }
                    }
                    .fontWeight(.semibold).frame(maxWidth: .infinity, minHeight: 44)
                    .disabled(!sameService || !available || choice.model.isEmpty || store.generatingID != nil || store.selected.messages.last?.id != retryMessage.id)
                    .accessibilityIdentifier("reply-regenerate-confirm")
                } footer: { Text("只重新回答这一条，保留旧回复；输入框的模型和草稿不变。") }
            }
        }.onAppear { manual = choice.model; reload() }.onDisappear { fetching?.cancel(); fetching = nil }
    }
    private func modelRow(_ model: LocalModelEntry) -> some View {
        row(model.name, detail: cloud || model.name == model.id ? nil : model.id, selected: sameService && choice.model == model.id, id: "local-model-\(model.id)") {
            // Selecting the current model must not reset its thinking setting.
            if !sameService || choice.model != model.id { choose(LocalModelChoice(endpoint: store.settings.serviceIdentity ?? "", model: model.id)) }
            if page == .more && issue == nil { query = ""; page = .models }
        }
    }
    private var thinkingOptions: some View {
        Section {
            row("服务默认", selected: choice.thinkingMode == nil && choice.reasoningEffort == nil, id: "local-thinking-default") {
                var next = choice; next.thinkingMode = nil; next.reasoningEffort = nil; choose(next)
            }
            if entry.modes.contains("disabled") {
                row("关闭思考", selected: choice.thinkingMode == "disabled", id: "local-thinking-disabled") { var next = choice; next.thinkingMode = "disabled"; next.reasoningEffort = nil; choose(next) }
            }
            if entry.modes.contains("enabled") && entry.efforts.isEmpty {
                row("开启思考", selected: choice.thinkingMode == "enabled" && choice.reasoningEffort == nil, id: "local-thinking-enabled") { var next = choice; next.thinkingMode = "enabled"; next.reasoningEffort = nil; choose(next) }
            }
            ForEach(entry.efforts, id: \.self) { effort in
                row(remoteEffortName(effort), selected: choice.reasoningEffort == effort, id: "local-effort-\(effort)") {
                    var next = choice; next.reasoningEffort = effort; next.thinkingMode = entry.modes.contains("enabled") ? "enabled" : nil; choose(next)
                }
            }
        } footer: { Text(entry.modes.isEmpty && entry.efforts.isEmpty ? "此模型未提供可选思考档位，将使用服务默认。" : (retryMessage == nil ? "用于下一次发送。思考越深入，通常需要等待越久。" : "用于这次重新回答。思考越深入，通常需要等待越久。")) }
    }
    private func choose(_ value: LocalModelChoice) {
        do {
            if retryMessage != nil { try value.validate(settings: store.settings); pendingChoice = value }
            else { try store.chooseLocalModel(value) }
            issue = nil
        } catch { issue = error.localizedDescription }
    }
    private func reload() {
        guard fetching == nil else { return }
        fetching = Task { @MainActor in
            do { try await store.reloadLocalModels(); guard !Task.isCancelled else { return }; issue = nil }
            catch { guard !Task.isCancelled else { return }; issue = ChatService.failureDescription(error) }
            fetching = nil
        }
    }
    private func row(_ title: String, detail: String? = nil, selected: Bool, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                VStack(alignment: .leading, spacing: 4) { Text(title); if let detail { Text(detail).font(.caption).foregroundStyle(.secondary) } }
                Spacer(minLength: 8); if selected { Image(systemName: "checkmark").fontWeight(.semibold).foregroundStyle(Color.blue).accessibilityHidden(true) }
            }.padding(.vertical, 5).frame(minHeight: 44).contentShape(Rectangle())
        }.accessibilityIdentifier(id).accessibilityValue(selected ? "已选择" : "未选择")
    }
}

// Shared native sheet keeps local and remote model selection visually consistent.
enum ModelPickerPage { case models, thinking, more
    var title: String { switch self { case .models: "选择模型"; case .thinking: "思考"; case .more: "更多模型" } }
}
struct ModelPickerSheet<Content: View>: View {
    @Binding var page: ModelPickerPage
    let prefix: String
    var compactHeight: CGFloat = 530
    var rootTitle: String? = nil
    let close: () -> Void
    @ViewBuilder let content: () -> Content
    @Environment(\.dynamicTypeSize) private var typeSize
    var body: some View {
        NavigationStack {
            List { content().listRowBackground(Color.white) }
                .contentMargins(.top, 16, for: .scrollContent)
                .listStyle(.insetGrouped).listSectionSpacing(16)
                .scrollContentBackground(.hidden).background(Color(white: 0.97))
                .scrollDismissesKeyboard(.interactively)
                .navigationTitle(page == .models ? (rootTitle ?? page.title) : page.title).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        if page != .models {
                            Button { page = .models } label: { Image(systemName: "chevron.left").frame(width: 44, height: 44) }
                                .accessibilityLabel("返回模型").accessibilityIdentifier("\(prefix)-model-back")
                        } else {
                            Button(action: close) { Image(systemName: "xmark").frame(width: 44, height: 44).background(.white, in: Circle()) }
                                .accessibilityLabel("完成").accessibilityIdentifier("\(prefix)-model-done")
                        }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        if page != .models { Button("完成", action: close).accessibilityIdentifier("\(prefix)-model-done") }
                    }
                }
        }.tint(Palette.ink)
            .presentationDetents(typeSize.isAccessibilitySize || page != .models ? [.large] : [.height(compactHeight), .large])
            .presentationDragIndicator(.visible).presentationCornerRadius(32)
    }
}
struct ModelPickerDisclosure: View {
    let title: String
    var value: String? = nil
    let id: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Text(title); Spacer(minLength: 8)
                if let value { Text(value).foregroundStyle(.secondary) }
                Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(.tertiary)
            }.frame(minHeight: 44).contentShape(Rectangle())
        }.accessibilityIdentifier(id)
    }
}
