import SwiftUI

struct LibraryView: View {
    @ObservedObject var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""
    @State private var showTrash = false
    @State private var renameID: UUID?
    @State private var name = ""
    var filtered: [Conversation] {
        let values = showTrash ? store.conversations.filter { $0.deletedAt != nil }.sorted { $0.updatedAt > $1.updatedAt } : store.visibleConversations
        return values.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search) || $0.messages.contains { message in
            message.text.localizedCaseInsensitiveContains(search) || (message.versions ?? []).contains { $0.text.localizedCaseInsensitiveContains(search) }
        } }
    }
    var body: some View {
        NavigationStack {
            List {
                if filtered.isEmpty {
                    ContentUnavailableView(search.isEmpty ? (showTrash ? "最近删除为空" : "从一段对话开始") : "没有找到相关对话", systemImage: search.isEmpty ? "bubble.left.and.bubble.right" : "magnifyingglass", description: Text(search.isEmpty ? "你的对话会保存在这台 iPhone。" : "试试标题或消息里的其他关键词。"))
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
                                HStack { Text(chat.title).font(.headline).lineLimit(1); if chat.pinned { Image(systemName: "pin.fill").font(.caption) } }
                                Text(chat.messages.last?.displayText ?? "还没有消息").font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(2)
                                Text(chat.updatedAt, style: .date).font(.caption).foregroundStyle(Palette.secondary)
                            }
                            Spacer(minLength: 0)
                            if chat.id == store.selectedID { Image(systemName: "checkmark").accessibilityLabel("当前对话") }
                        }.padding(.vertical, 8)
                    }.foregroundStyle(Palette.ink)
                    .swipeActions(edge: .trailing) {
                        if showTrash { Button("恢复") { store.restore(chat.id) }.tint(.blue) }
                        else { Button("删除", role: .destructive) { store.trash(chat.id) } }
                    }
                    .contextMenu {
                        if showTrash { Button("恢复", systemImage: "arrow.uturn.backward") { store.restore(chat.id) } }
                        else {
                            Button(chat.pinned ? "取消置顶" : "置顶", systemImage: "pin") { store.update(chat.id) { $0.pinned.toggle() } }
                            Button("重命名", systemImage: "pencil") { name = chat.title; renameID = chat.id }
                            Button("移到最近删除", systemImage: "trash", role: .destructive) { store.trash(chat.id) }
                        }
                    }
                }
            }.scrollContentBackground(.hidden).background(Palette.canvas)
                .searchable(text: $search, prompt: "搜索标题和消息")
                .navigationTitle(showTrash ? "最近删除" : "对话")
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) { Button { showTrash.toggle() } label: { Image(systemName: showTrash ? "bubble.left.and.bubble.right" : "trash") }.accessibilityLabel(showTrash ? "所有对话" : "最近删除") }
                    ToolbarItem(placement: .topBarTrailing) { Button("完成") { dismiss() } }
                    ToolbarItem(placement: .bottomBar) { Button("新对话", systemImage: "square.and.pencil") { store.newChat(); dismiss() } }
                }
                .alert("重命名对话", isPresented: Binding(get: { renameID != nil }, set: { if !$0 { renameID = nil } })) {
                    TextField("对话标题", text: $name)
                    Button("取消", role: .cancel) { renameID = nil }
                    Button("保存") { if let id = renameID, !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { store.update(id) { $0.title = String(name.prefix(100)) } }; renameID = nil }
                }
        }
    }
}

struct SettingsView: View {
    @ObservedObject var store: WorkspaceStore
    @State private var showCloud = false
    @Environment(\.dismiss) private var dismiss
    @State private var configuration: ConnectionSettings
    @State private var token = SecureToken.read()
    @State private var error: String?
    @State private var connectionTask: Task<Void, Never>?
    @State private var connectionResult: String?
    @State private var connectionSucceeded = false
    private enum Field: Hashable { case endpoint, model, token, preference }
    @FocusState private var focusedField: Field?
    init(store: WorkspaceStore) { self.store = store; _configuration = State(initialValue: store.settings) }
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack(spacing: 14) {
                        Image("PotatoMark").resizable().frame(width: 56, height: 56).clipShape(RoundedRectangle(cornerRadius: 14)).accessibilityHidden(true)
                        VStack(alignment: .leading, spacing: 5) { Text("Potato").font(.title2.bold()); Text("你的想法，随时继续。").font(.subheadline).foregroundStyle(Palette.secondary) }
                    }.padding(.vertical, 8)
                }
                Section("云端模型") {
                    if let account = configuration.cloudAccount {
                        Label(account.email, systemImage: "person.crop.circle")
                        Text("模型由云端统一提供，电脑离线也可使用。").font(.footnote).foregroundStyle(Palette.secondary)
                    }
                    Button(configuration.cloudAccount == nil ? "登录 Cloudflare，使用云端模型" : "管理云端账号") { showCloud = true }
                        .accessibilityIdentifier("cloud-model-login")
                }
                Section {
                    Toggle("本地体验模式", isOn: $configuration.demo).accessibilityIdentifier("demo-mode")
                } footer: { Text(configuration.demo ? "普通对话使用示例回复，不发送对话或附件。模型列表和“测试连接”会访问配置的服务。" : "发送时，会将当前对话及其附件交给下方配置的服务。") }
                if configuration.cloudAccount == nil {
                Section("模型连接") {
                    TextField("", text: $configuration.endpoint, prompt: Text("完整接口地址（HTTPS）").foregroundStyle(Palette.secondary)).textContentType(.URL).keyboardType(.URL).autocorrectionDisabled().textInputAutocapitalization(.never).accessibilityIdentifier("endpoint")
                        .focused($focusedField, equals: .endpoint).submitLabel(.next).onSubmit { focusedField = .model }
                    TextField("", text: $configuration.model, prompt: Text("模型名称").foregroundStyle(Palette.secondary)).autocorrectionDisabled().textInputAutocapitalization(.never).accessibilityIdentifier("model-name")
                        .focused($focusedField, equals: .model).submitLabel(.next).onSubmit { focusedField = .token }
                    SecureField("", text: $token, prompt: Text("连接令牌（可选）").foregroundStyle(Palette.secondary)).textContentType(.password).autocorrectionDisabled().textInputAutocapitalization(.never)
                        .focused($focusedField, equals: .token).submitLabel(.done).onSubmit { focusedField = nil }
                    Text("支持 Cloudflare Worker 提供的 OpenAI 兼容流式接口，例如 /v1/chat/completions。令牌只保存在此设备的 Keychain。").font(.footnote).foregroundStyle(Palette.secondary)
                }
                Section {
                    Button {
                        if connectionTask != nil { cancelConnectionTest(); connectionResult = "测试已取消。" }
                        else { testConnection() }
                    } label: {
                        HStack { if connectionTask != nil { ProgressView() }; Text(connectionTask == nil ? "测试连接" : "取消测试") }
                    }.disabled(configuration.validatedURL == nil || configuration.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("test-connection")
                    if let connectionResult {
                        Label(connectionResult, systemImage: connectionSucceeded ? "checkmark.circle" : "info.circle")
                            .font(.subheadline).foregroundStyle(connectionSucceeded ? Palette.ink : Palette.secondary)
                            .accessibilityElement(children: .ignore).accessibilityLabel(connectionResult)
                            .accessibilityIdentifier("connection-result")
                    }
                } footer: { Text("点击后向上方服务发送一条短测试消息，不包含对话和附件，可能消耗少量模型额度。测试成功后仍需保存设置。") }
                } else {
                    Section {
                        Text(configuration.displayModelName)
                        Button("改为手动连接") { configuration.cloudAccount = nil; configuration.endpoint = ""; configuration.model = ""; configuration.modelCatalog = nil; configuration.demo = true; token = "" }
                    }
                }
                Section("偏好") { Toggle("触感反馈", isOn: $configuration.haptics) }
                Section("回复偏好") { TextEditor(text: $configuration.systemPrompt).frame(minHeight: 100).accessibilityLabel("回复偏好").focused($focusedField, equals: .preference) }
                Section("数据与隐私") {
                    Label("对话与附件保存在本机", systemImage: "iphone")
                    Text("尚未启用云端同步。删除的对话可在“最近删除”恢复。卸载应用会移除本机记录。").font(.footnote).foregroundStyle(Palette.secondary)
                    Button("打开系统权限设置", systemImage: "gear") { if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) } }
                }
                Section { Text("版本 0.2 · iPhone 原生预览版").font(.footnote).foregroundStyle(Palette.secondary) }
            }.scrollDismissesKeyboard(.interactively).navigationTitle("设置").navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } }
                    ToolbarItem(placement: .confirmationAction) { Button("保存") { save() }.bold().accessibilityIdentifier("save-settings") }
                    ToolbarItemGroup(placement: .keyboard) { Spacer(); Button("收起键盘") { focusedField = nil }.accessibilityIdentifier("dismiss-settings-keyboard") }
                }
                .alert("无法保存", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("知道了", role: .cancel) {} } message: { Text(error ?? "") }
        }.interactiveDismissDisabled()
            .sheet(isPresented: $showCloud) { CloudAccountView(store: store, connected: { dismiss() }) }
            .onChange(of: configuration.endpoint) { _, value in
                configuration.modelCatalog = nil
                if URL(string: value)?.host != store.settings.validatedURL?.host { token = "" }
            }
            .onChange(of: configuration) { _, _ in cancelConnectionTest() }
            .onChange(of: token) { _, _ in cancelConnectionTest() }
            .onDisappear { cancelConnectionTest() }
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
                connectionSucceeded = true; connectionResult = "已收到模型回复，流式连接正常。"
            } catch {
                guard !Task.isCancelled else { return }
                connectionResult = ChatService.failureDescription(error)
            }
            connectionTask = nil
        }
    }
    private func save() {
        if !configuration.demo && (configuration.validatedURL == nil || configuration.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) { error = "请填写有效的 HTTPS 接口地址和模型名称。"; return }
        do {
            let credential = token.trimmingCharacters(in: .whitespacesAndNewlines)
            if configuration.cloudAccount == nil {
                if credential != SecureToken.read() { configuration.modelCatalog = nil }
                try SecureToken.save(credential)
            }
            store.settings = configuration; store.persist(); dismiss()
        }
        catch { self.error = error.localizedDescription }
    }
}


/// A drawer using the same information hierarchy as the supplied ChatGPT reference.
struct WorkspaceSidebar: View {
    @ObservedObject var store: WorkspaceStore
    let remoteSelected: Bool
    let selectRemote: () -> Void
    let selectChat: (UUID) -> Void
    let newChat: () -> Void
    let library: () -> Void
    let settings: () -> Void
    @State private var search = ""
    @State private var searchVisible = false
    @FocusState private var searching: Bool
    private var chats: [Conversation] {
        store.visibleConversations.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search) || $0.messages.contains { $0.text.localizedCaseInsensitiveContains(search) } }
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Potato").font(.title2.weight(.semibold))
                Spacer()
                Button { searchVisible.toggle() } label: { Image(systemName: "magnifyingglass").font(.title3).frame(width: 44, height: 44) }.accessibilityLabel("搜索会话")
            }.padding(.horizontal, 22).padding(.top, 8)
            if searchVisible || !search.isEmpty {
                HStack { Image(systemName: "magnifyingglass"); TextField("搜索标题和消息", text: $search).focused($searching).accessibilityIdentifier("sidebar-search").onAppear { searching = true } }
                    .padding(12).background(Palette.muted, in: Capsule()).padding(.horizontal, 16).padding(.vertical, 8)
                    .excludesSidebarGesture()
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if search.isEmpty {
                        sidebarAction("资料库", icon: "books.vertical", action: library)
                        sidebarAction("远程", icon: "desktopcomputer", selected: remoteSelected, action: selectRemote).accessibilityIdentifier("sidebar-remote")
                    }
                    if !chats.filter(\.pinned).isEmpty {
                        heading("置顶")
                        ForEach(chats.filter(\.pinned)) { row($0) }
                    }
                    heading(search.isEmpty ? "最近对话" : "搜索结果")
                    ForEach(chats.filter { !$0.pinned }) { row($0) }
                    if chats.isEmpty { Text(search.isEmpty ? "从一段新对话开始" : "没有找到相关对话").font(.subheadline).foregroundStyle(Palette.secondary).padding(18) }
                }.padding(.horizontal, 10).padding(.bottom, 16)
            }
            HStack {
                Button(action: newChat) { Label("对话", systemImage: "square.and.pencil").font(.headline).padding(.horizontal, 20).frame(minHeight: 48).foregroundStyle(.white).background(Palette.ink, in: Capsule()) }.accessibilityIdentifier("sidebar-new-chat")
                Spacer()
                Button(action: settings) { Image(systemName: "gearshape").font(.title3).frame(width: 48, height: 48).background(.white.opacity(0.8), in: Circle()) }.accessibilityLabel("设置").accessibilityIdentifier("sidebar-settings")
            }.padding(.horizontal, 22).padding(.vertical, 12)
        }.foregroundStyle(Palette.ink).background(Palette.canvas)
    }
    private func heading(_ title: String) -> some View { Text(title).font(.headline).padding(.horizontal, 12).padding(.top, 26).padding(.bottom, 12).accessibilityAddTraits(.isHeader) }
    private func sidebarAction(_ title: String, icon: String, selected: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) { Label(title, systemImage: icon).font(.body.weight(.medium)).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 12).frame(minHeight: 48).background(selected ? Palette.muted : .clear, in: RoundedRectangle(cornerRadius: 13)) }.buttonStyle(.plain)
    }
    private func row(_ chat: Conversation) -> some View {
        Button { selectChat(chat.id) } label: { Text(chat.title).font(.body).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 12).frame(minHeight: 48) }.buttonStyle(.plain)
            .contextMenu {
                Button(chat.pinned ? "取消置顶" : "置顶", systemImage: "pin") { store.update(chat.id) { $0.pinned.toggle() } }
                Button("移到最近删除", systemImage: "trash", role: .destructive) { store.trash(chat.id) }
            }
    }
}
