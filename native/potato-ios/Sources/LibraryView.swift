import SwiftUI

struct ConversationHistoryView: View {
    @ObservedObject var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""
    @State private var showTrash = false
    @State private var renameID: UUID?
    @State private var name = ""
    var filtered: [Conversation] {
        let values = showTrash ? store.conversations.filter { $0.deletedAt != nil }.sorted { $0.updatedAt > $1.updatedAt } : store.visibleConversations.filter { !$0.isEmptyShell }
        return values.filter { search.isEmpty || $0.displayTitle.localizedCaseInsensitiveContains(search) || $0.messages.contains { message in
            message.text.localizedCaseInsensitiveContains(search) || (message.versions ?? []).contains { $0.text.localizedCaseInsensitiveContains(search) }
        } }
    }
    var body: some View {
        NavigationStack {
            List {
                if filtered.isEmpty {
                    ContentUnavailableView(search.isEmpty ? (showTrash ? L10n.tr("最近删除为空") : L10n.tr("从一段对话开始")) : L10n.tr("没有找到相关对话"), systemImage: search.isEmpty ? "bubble.left.and.bubble.right" : "magnifyingglass", description: Text(search.isEmpty ? L10n.tr("你的对话会保存在这台 iPhone。") : L10n.tr("试试标题或消息里的其他关键词。")))
                        .listRowBackground(Color.clear)
                }
                ForEach(filtered) { chat in
                    Button {
                        if showTrash { store.restore(chat.id) }
                        store.select(chat.id); dismiss()
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: showTrash ? "arrow.uturn.backward" : chat.draft == nil ? "bubble.left" : "doc.text").font(.title3).foregroundStyle(Palette.secondary)
                            VStack(alignment: .leading, spacing: 6) {
                                HStack { Text(chat.displayTitle).font(.headline).lineLimit(1); if chat.pinned { Image(systemName: "pin.fill").font(.caption) } }
                                Text(chat.messages.last?.displayText ?? L10n.tr("还没有消息")).font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(2)
                                Text(chat.updatedAt, style: .date).font(.caption).foregroundStyle(Palette.secondary)
                            }
                            Spacer(minLength: 0)
                            if chat.id == store.selectedID { Image(systemName: "checkmark").accessibilityLabel(L10n.tr("当前对话")) }
                        }.padding(.vertical, 8)
                    }.foregroundStyle(Palette.ink)
                    .swipeActions(edge: .trailing) {
                        if showTrash { Button(L10n.tr("恢复")) { store.restore(chat.id) }.tint(.blue) }
                        else { Button(L10n.tr("删除"), role: .destructive) { store.trash(chat.id) } }
                    }
                    .contextMenu {
                        if showTrash { Button(L10n.tr("恢复"), systemImage: "arrow.uturn.backward") { store.restore(chat.id) } }
                        else {
                            Button(chat.pinned ? L10n.tr("取消置顶") : L10n.tr("置顶"), systemImage: "pin") { store.update(chat.id) { $0.pinned.toggle() } }
                            Button(L10n.tr("重命名"), systemImage: "pencil") { name = chat.displayTitle; renameID = chat.id }
                            Button(L10n.tr("移到最近删除"), systemImage: "trash", role: .destructive) { store.trash(chat.id) }
                        }
                    }
                }
            }.scrollContentBackground(.hidden).background(Palette.canvas)
                .searchable(text: $search, prompt: L10n.tr("搜索标题和消息"))
                .navigationTitle(showTrash ? L10n.tr("最近删除") : L10n.tr("对话"))
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) { Button { showTrash.toggle() } label: { Image(systemName: showTrash ? "bubble.left.and.bubble.right" : "trash") }.accessibilityLabel(showTrash ? L10n.tr("所有对话") : L10n.tr("最近删除")) }
                    ToolbarItem(placement: .topBarTrailing) { Button(L10n.tr("完成")) { dismiss() } }
                    ToolbarItem(placement: .bottomBar) { Button(L10n.tr("新对话"), systemImage: "square.and.pencil") { store.newChat(); dismiss() } }
                }
                .alert(L10n.tr("重命名对话"), isPresented: Binding(get: { renameID != nil }, set: { if !$0 { renameID = nil } })) {
                    TextField(L10n.tr("对话标题"), text: $name)
                    Button(L10n.tr("取消"), role: .cancel) { renameID = nil }
                    Button(L10n.tr("保存")) { if let id = renameID { store.rename(id, to: name) }; renameID = nil }
                }
        }
    }
}

struct SettingsView: View {
    @ObservedObject var store: WorkspaceStore
    @State private var showCloud = false
    @Binding private var appearancePreview: AppAppearance?
    @Environment(\.dismiss) private var dismiss
    /// Only the developer connection is edited here and committed with “完成”; other options apply at once.
    @State private var configuration: ConnectionSettings
    @State private var token = SecureToken.read()
    @State private var savedToken = SecureToken.read()
    @State private var error: String?
    @State private var connectionTask: Task<Void, Never>?
    @State private var connectionResult: String?
    @State private var connectionSucceeded = false
    @State private var versionTaps = 0
    @State private var developerNotice: String?
    @State private var confirmingTrashAll = false
    private enum Field: Hashable { case endpoint, model, token }
    @FocusState private var focusedField: Field?
    init(store: WorkspaceStore, appearancePreview: Binding<AppAppearance?>) {
        self.store = store
        _configuration = State(initialValue: store.settings)
        _appearancePreview = appearancePreview
    }
    private var showsDeveloper: Bool { store.settings.developerMode == true || AppEnvironment.isUITesting }
    private var developerDirty: Bool {
        configuration.demo != store.settings.demo || configuration.endpoint != store.settings.endpoint || configuration.model != store.settings.model
            || configuration.cloudAccount != store.settings.cloudAccount || token != savedToken
    }
    private var appearance: Binding<AppAppearance> { Binding(get: { store.settings.appearanceMode }, set: { store.settings.appearanceMode = $0; store.persist() }) }
    private var language: Binding<AppLanguage> { Binding(get: { store.settings.languageMode }, set: { store.settings.languageMode = $0; AppLocalization.shared.selection = $0; store.persist() }) }
    private var haptics: Binding<Bool> { Binding(get: { store.settings.haptics }, set: { store.settings.haptics = $0; store.persist() }) }
    /// Only a signed-in account's catalog offers a choice; a manual connection names its model directly.
    private var defaultModels: [LocalModelEntry] { store.settings.cloudAccount != nil && !store.settings.demo ? store.settings.currentCatalog?.models ?? [] : [] }
    private var defaultModel: Binding<String> { Binding(get: { store.settings.model }, set: { store.settings.model = $0; configuration.model = $0; store.persist() }) }
    var body: some View {
        NavigationStack {
            Form {
                accountSection
                Section {
                    Picker(L10n.tr("外观"), selection: appearance) {
                        ForEach(AppAppearance.allCases, id: \.self) { mode in Text(mode.title).tag(mode) }
                    }.pickerStyle(.segmented).accessibilityIdentifier("appearance-picker")
                    Picker(L10n.tr("语言"), selection: language) {
                        ForEach(AppLanguage.allCases, id: \.self) { language in Text(language.title).tag(language) }
                    }.accessibilityIdentifier("language-picker")
                    Toggle(L10n.tr("触感反馈"), isOn: haptics)
                } header: { Text(L10n.tr("通用")) }
                Section(L10n.tr("个性化")) {
                    NavigationLink {
                        CustomInstructionsView(store: store)
                    } label: {
                        HStack { Text(L10n.tr("自定义指令")); Spacer(minLength: 12); Text(instructionsSummary).foregroundStyle(Palette.secondary).lineLimit(1) }
                    }.accessibilityIdentifier("custom-instructions-open")
                    NavigationLink {
                        RecallView(store: store)
                    } label: {
                        HStack { Text(L10n.tr("记忆")); Spacer(minLength: 12); Text(store.settings.recallEnabled == true ? L10n.tr("已开启") : L10n.tr("未开启")).foregroundStyle(Palette.secondary) }
                    }.accessibilityIdentifier("settings-memory")
                    if defaultModels.count > 1 {
                        Picker(L10n.tr("新对话默认模型"), selection: defaultModel) {
                            ForEach(defaultModels) { model in Text(model.displayName).tag(model.id) }
                        }.accessibilityIdentifier("default-model-picker")
                    }
                }
                if showsDeveloper { developerSections }
                Section(L10n.tr("数据")) {
                    Button(L10n.tr("照片、相机与麦克风权限")) { if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) } }
                    Button(L10n.tr("删除所有对话"), role: .destructive) { confirmingTrashAll = true }
                        .disabled(store.visibleConversations.allSatisfy(\.isEmptyShell)).accessibilityIdentifier("trash-all-conversations")
                }
                Section {
                    Button { tapVersion() } label: {
                        VStack(spacing: 4) {
                            Text("Potato \(AppEnvironment.versionLabel)").font(.footnote).foregroundStyle(Palette.secondary)
                            if let developerNotice { Text(developerNotice).font(.caption).foregroundStyle(Palette.secondary) }
                        }.frame(maxWidth: .infinity)
                    }.buttonStyle(.plain).accessibilityIdentifier("settings-version")
                }.listRowBackground(Color.clear)
            }.scrollDismissesKeyboard(.interactively).navigationTitle(L10n.tr("设置")).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    if developerDirty { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { dismiss() }.accessibilityIdentifier("discard-settings") } }
                    ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { save() }.bold().accessibilityIdentifier("save-settings") }
                    ToolbarItemGroup(placement: .keyboard) { Spacer(); Button(L10n.tr("收起键盘")) { focusedField = nil }.accessibilityIdentifier("dismiss-settings-keyboard") }
                }
                .confirmationDialog(L10n.tr("删除所有对话？"), isPresented: $confirmingTrashAll, titleVisibility: .visible) {
                    Button(L10n.tr("删除所有对话"), role: .destructive) { store.trashAll(); dismiss() }
                } message: { Text(L10n.tr("可以在最近删除中恢复。")) }
                .alert(L10n.tr("无法保存"), isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button(L10n.tr("知道了"), role: .cancel) {} } message: { Text(error ?? "") }
        }.presentationDragIndicator(.visible)
            .sheet(isPresented: $showCloud) { CloudAccountView(store: store, connected: { dismiss() }) }
            .onChange(of: configuration.endpoint) { _, value in
                configuration.modelCatalog = nil
                if URL(string: value)?.host != store.settings.validatedURL?.host { token = "" }
            }
            .onChange(of: configuration) { _, _ in cancelConnectionTest() }
            .onChange(of: token) { _, _ in cancelConnectionTest() }
            .onAppear { appearancePreview = nil }
            .onDisappear { AppLocalization.shared.selection = store.settings.languageMode; cancelConnectionTest() }
    }
    private var instructionsSummary: String {
        let value = store.settings.customInstructions?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return value.isEmpty ? L10n.tr("未设置") : value
    }
    @ViewBuilder private var accountSection: some View {
        Section {
            if let account = store.settings.cloudAccount {
                HStack(spacing: 12) {
                    Image(systemName: "person.crop.circle.fill").font(.system(size: 36)).foregroundStyle(Palette.secondary).accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(account.email).font(.body.weight(.medium)).lineLimit(1)
                        if store.authorizationExpired { Text(L10n.tr("登录已过期")).font(.footnote).foregroundStyle(Color.orange) }
                    }
                }.padding(.vertical, 4)
                Button(store.authorizationExpired ? L10n.tr("重新登录") : L10n.tr("管理账号")) { showCloud = true }.accessibilityIdentifier("cloud-model-login")
                if store.settings.currentCatalog?.canEdit == true && !store.authorizationExpired {
                    NavigationLink(L10n.tr("云端模型")) { CloudModelsView(store: store) }.accessibilityIdentifier("cloud-models-open")
                }
            } else {
                HStack(spacing: 14) {
                    Image("PotatoMark").resizable().frame(width: 52, height: 52).clipShape(RoundedRectangle(cornerRadius: 13)).accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Potato").font(.title3.bold())
                        if !store.settings.demo, let host = store.settings.validatedURL?.host {
                            Text(store.authorizationExpired ? L10n.tr("自定义服务 · 连接令牌无效") : L10n.tr("已连接自定义服务 · \(host)")).font(.subheadline).foregroundStyle(store.authorizationExpired ? Color.orange : Palette.secondary).lineLimit(2)
                        }
                    }
                }.padding(.vertical, 6)
                Button(L10n.tr("登录 Potato 账号")) { showCloud = true }.fontWeight(.semibold).accessibilityIdentifier("cloud-model-login")
            }
        }
    }
    @ViewBuilder private var developerSections: some View {
        Section {
            Toggle(L10n.tr("本地体验模式"), isOn: $configuration.demo).accessibilityIdentifier("demo-mode")
        } header: { Text(L10n.tr("开发者选项")) } footer: { Text(configuration.demo ? L10n.tr("普通对话使用示例回复，不发送对话或附件。模型列表和“测试连接”会访问配置的服务。") : L10n.tr("发送时，会将当前对话及其附件交给下方配置的服务。")) }
        if configuration.cloudAccount == nil {
            Section(L10n.tr("模型连接")) {
                TextField("", text: $configuration.endpoint, prompt: Text(L10n.tr("完整接口地址（HTTPS）")).foregroundStyle(Palette.secondary)).textContentType(.URL).keyboardType(.URL).autocorrectionDisabled().textInputAutocapitalization(.never).accessibilityIdentifier("endpoint")
                    .focused($focusedField, equals: .endpoint).submitLabel(.next).onSubmit { focusedField = .model }
                TextField("", text: $configuration.model, prompt: Text(L10n.tr("模型名称")).foregroundStyle(Palette.secondary)).autocorrectionDisabled().textInputAutocapitalization(.never).accessibilityIdentifier("model-name")
                    .focused($focusedField, equals: .model).submitLabel(.next).onSubmit { focusedField = .token }
                SecureField("", text: $token, prompt: Text(L10n.tr("连接令牌（可选）")).foregroundStyle(Palette.secondary)).textContentType(.password).autocorrectionDisabled().textInputAutocapitalization(.never)
                    .focused($focusedField, equals: .token).submitLabel(.done).onSubmit { focusedField = nil }
                Text(L10n.tr("支持 Cloudflare Worker 提供的 OpenAI 兼容流式接口，例如 /v1/chat/completions。令牌只保存在此设备的 Keychain。")).font(.footnote).foregroundStyle(Palette.secondary)
            }
            Section {
                Button {
                    if connectionTask != nil { cancelConnectionTest(); connectionResult = L10n.tr("测试已取消。") }
                    else { testConnection() }
                } label: {
                    HStack { if connectionTask != nil { ProgressView() }; Text(connectionTask == nil ? L10n.tr("测试连接") : L10n.tr("取消测试")) }
                }.disabled(configuration.validatedURL == nil || configuration.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("test-connection")
                if let connectionResult {
                    Label(connectionResult, systemImage: connectionSucceeded ? "checkmark.circle" : "info.circle")
                        .font(.subheadline).foregroundStyle(connectionSucceeded ? Palette.ink : Palette.secondary)
                        .accessibilityElement(children: .ignore).accessibilityLabel(connectionResult)
                        .accessibilityIdentifier("connection-result")
                }
            } footer: { Text(L10n.tr("点击后向上方服务发送一条短测试消息，不包含对话和附件，可能消耗少量模型额度。测试成功后仍需保存设置。")) }
        } else {
            Section {
                Text(ModelNaming.displayName(id: configuration.model, name: configuration.displayModelName))
                Button(L10n.tr("改为手动连接")) { configuration.cloudAccount = nil; configuration.endpoint = ""; configuration.model = ""; configuration.modelCatalog = nil; configuration.demo = true; token = "" }
            } header: { Text(L10n.tr("模型连接")) }
        }
        if store.settings.developerMode == true {
            Section { Button(L10n.tr("关闭开发者选项")) { store.settings.developerMode = false; store.persist(); versionTaps = 0; developerNotice = nil } }
        }
    }
    private func tapVersion() {
        guard store.settings.developerMode != true else { return }
        versionTaps += 1
        if versionTaps >= 7 {
            store.settings.developerMode = true; store.persist(); developerNotice = L10n.tr("已开启开发者选项")
        } else if versionTaps >= 4 {
            developerNotice = L10n.tr("再点 \(7 - versionTaps) 次开启开发者选项")
        }
    }
    private func cancelConnectionTest() {
        connectionTask?.cancel(); connectionTask = nil; connectionResult = nil; connectionSucceeded = false
    }
    private func testConnection() {
        let settings = configuration, credential = token.trimmingCharacters(in: .whitespacesAndNewlines)
        connectionResult = nil; connectionSucceeded = false
        connectionTask = Task { @MainActor in
            do {
                try await ChatService.testConnection(settings: settings, token: credential)
                guard !Task.isCancelled else { return }
                connectionSucceeded = true; connectionResult = L10n.tr("已收到模型回复，流式连接正常。")
            } catch {
                guard !Task.isCancelled else { return }
                connectionResult = ChatService.failureDescription(error)
            }
            connectionTask = nil
        }
    }
    /// General options are already applied. Commit the developer connection only when it changed.
    private func save() {
        guard developerDirty else { dismiss(); return }
        if !configuration.demo && (configuration.validatedURL == nil || configuration.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) { error = L10n.tr("请填写有效的 HTTPS 接口地址和模型名称。"); return }
        do {
            var next = store.settings
            next.demo = configuration.demo; next.endpoint = configuration.endpoint; next.model = configuration.model
            next.cloudAccount = configuration.cloudAccount; next.modelCatalog = configuration.modelCatalog
            let credential = token.trimmingCharacters(in: .whitespacesAndNewlines)
            if next.cloudAccount == nil {
                if credential != savedToken { next.modelCatalog = nil }
                try SecureToken.save(credential)
            }
            store.settings = next; store.persist(); dismiss()
        }
        catch { self.error = error.localizedDescription }
    }
}

struct CustomInstructionsView: View {
    @ObservedObject var store: WorkspaceStore
    @State private var text: String
    init(store: WorkspaceStore) {
        self.store = store
        _text = State(initialValue: store.settings.customInstructions ?? "")
    }
    var body: some View {
        Form {
            Section {
                TextEditor(text: $text).frame(minHeight: 200).accessibilityIdentifier("custom-instructions")
            } header: { Text(L10n.tr("希望 Potato 了解什么")) } footer: { Text(L10n.tr("例如职业、常用语言、回答长度和语气。")) }
        }.navigationTitle(L10n.tr("自定义指令")).navigationBarTitleDisplayMode(.inline)
            .onChange(of: text) { _, value in
                let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                store.settings.customInstructions = trimmed.isEmpty ? nil : String(value.prefix(4000))
                store.scheduleSave()
            }
            .onDisappear { store.persist() }
    }
}


/// A drawer using the same information hierarchy as the supplied ChatGPT reference.
struct WorkspaceSidebar: View {
    @ObservedObject var store: WorkspaceStore
    let remoteSelected: Bool
    var librarySelected = false
    let selectRemote: () -> Void
    let selectChat: (UUID) -> Void
    let newChat: () -> Void
    let library: () -> Void
    let history: () -> Void
    let settings: () -> Void
    @State private var search = ""
    @State private var searchVisible = false
    @State private var renameID: UUID?
    @State private var renameText = ""
    @FocusState private var searching: Bool
    /// Empty conversations are not history; the current one stays so it can be highlighted.
    private var chats: [Conversation] {
        store.visibleConversations.filter { !$0.isEmptyShell || $0.id == store.selectedID }
            .filter { search.isEmpty || $0.displayTitle.localizedCaseInsensitiveContains(search) || $0.messages.contains { $0.text.localizedCaseInsensitiveContains(search) } }
    }
    private var periods: [(ConversationPeriod, [Conversation])] {
        let groups = Dictionary(grouping: chats.filter { !$0.pinned }) { ConversationPeriod.of($0.updatedAt) }
        return ConversationPeriod.allCases.compactMap { period in groups[period].map { (period, $0) } }
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Potato").font(.title2.weight(.semibold)).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                Spacer()
                Button { searchVisible.toggle() } label: { Image(systemName: "magnifyingglass").font(.system(size: 20)).frame(width: 44, height: 44).contentShape(Circle()) }.accessibilityLabel(L10n.tr("搜索会话"))
                Button(action: history) { Image(systemName: "ellipsis").font(.system(size: 20)).frame(width: 44, height: 44).contentShape(Circle()) }.accessibilityLabel(L10n.tr("管理对话与最近删除")).accessibilityIdentifier("sidebar-history-manage")
            }.padding(.horizontal, 22).padding(.top, 8)
            if searchVisible || !search.isEmpty {
                HStack { Image(systemName: "magnifyingglass"); TextField(L10n.tr("搜索标题和消息"), text: $search).focused($searching).accessibilityIdentifier("sidebar-search").onAppear { searching = true } }
                    .padding(12).background(Palette.muted, in: Capsule()).padding(.horizontal, 16).padding(.vertical, 8)
                    .excludesSidebarGesture()
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if search.isEmpty {
                        sidebarAction(L10n.tr("资料库"), icon: "books.vertical", selected: librarySelected, action: library).accessibilityIdentifier("sidebar-library")
                        sidebarAction(L10n.tr("远程"), icon: "laptopcomputer.and.iphone", selected: remoteSelected, action: selectRemote).accessibilityIdentifier("sidebar-remote")
                    }
                    if !chats.filter(\.pinned).isEmpty {
                        heading(L10n.tr("置顶"))
                        ForEach(chats.filter(\.pinned)) { row($0) }
                    }
                    ForEach(Array(periods.enumerated()), id: \.element.0) { index, group in
                        heading(search.isEmpty ? group.0.title : (index == 0 ? L10n.tr("搜索结果") : group.0.title))
                        ForEach(group.1) { row($0) }
                    }
                    if chats.isEmpty { Text(search.isEmpty ? L10n.tr("从一段新对话开始") : L10n.tr("没有找到相关对话")).font(.subheadline).foregroundStyle(Palette.secondary).padding(18) }
                }.padding(.horizontal, 10).padding(.bottom, 16)
            }
            HStack {
                Button(action: newChat) { Label(L10n.tr("新对话"), systemImage: "plus").font(.body.weight(.medium)).dynamicTypeSize(...DynamicTypeSize.xxxLarge).lineLimit(1).padding(.horizontal, 20).frame(minHeight: 48).foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule()) }.accessibilityIdentifier("sidebar-new-chat")
                Spacer()
                Button(action: settings) { Image(systemName: "gearshape").font(.system(size: 20)).frame(width: 48, height: 48).contentShape(Circle()) }.buttonStyle(.plain).chatGlass(in: Circle(), interactive: true).accessibilityLabel(L10n.tr("设置")).accessibilityIdentifier("sidebar-settings")
            }.padding(.horizontal, 22).padding(.vertical, 12)
        }.foregroundStyle(Palette.ink).background(Palette.canvas)
            .alert(L10n.tr("重命名对话"), isPresented: Binding(get: { renameID != nil }, set: { if !$0 { renameID = nil } })) {
                TextField(L10n.tr("对话标题"), text: $renameText)
                Button(L10n.tr("取消"), role: .cancel) { renameID = nil }
                Button(L10n.tr("保存")) { if let id = renameID { store.rename(id, to: renameText) }; renameID = nil }
            }
    }
    private func heading(_ title: String) -> some View { Text(title).font(.subheadline).foregroundStyle(Palette.secondary).padding(.horizontal, 12).padding(.top, 26).padding(.bottom, 12).accessibilityAddTraits(.isHeader) }
    private func sidebarAction(_ title: String, icon: String, selected: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                // Device symbols carry a gray screen layer; drop it so every row is a plain outline.
                Image(systemName: icon).font(.system(size: 20)).symbolRenderingMode(.palette).foregroundStyle(Palette.ink, .clear)
                    .frame(width: 24).accessibilityHidden(true)
                Text(title).font(.body.weight(.medium)).fixedSize(horizontal: false, vertical: true)
            }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 12).padding(.vertical, 6)
                .frame(minHeight: 48).background(selected ? Palette.muted : .clear, in: RoundedRectangle(cornerRadius: 13, style: .continuous))
                .contentShape(Rectangle())
        }.buttonStyle(.plain)
    }
    private func row(_ chat: Conversation) -> some View {
        let selected = !librarySelected && !remoteSelected && store.selectedID == chat.id
        return Button { selectChat(chat.id) } label: {
            Text(chat.displayTitle).font(.body).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12).frame(minHeight: 48)
                .background(selected ? Palette.muted : .clear, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                .contentShape(Rectangle())
        }.buttonStyle(.plain)
            .accessibilityAddTraits(selected ? .isSelected : [])
            .contextMenu {
                Button(L10n.tr("重命名"), systemImage: "pencil") { renameText = chat.displayTitle; renameID = chat.id }
                Button(chat.pinned ? L10n.tr("取消置顶") : L10n.tr("置顶"), systemImage: "pin") { store.update(chat.id) { $0.pinned.toggle() } }
                Button(L10n.tr("移到最近删除"), systemImage: "trash", role: .destructive) { store.trash(chat.id) }
            }
    }
}
