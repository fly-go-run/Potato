import SwiftUI

/// Cached titles stay readable while navigation waits for a live connection.
private struct RemoteDirectoryRowStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.opacity(configuration.isPressed ? 0.65 : 1)
    }
}

struct RemoteView: View {
    @ObservedObject var store: RemoteStore
    let openSidebar: () -> Void
    var settings: ConnectionSettings = ConnectionSettings()
    @State private var selectedDevice: String?
    @State private var search = ""
    @State private var showPairing = false
    @State private var showDevices = false
    @State private var showLogin = false
    @State private var loginRelay = "https://potato-remote.recodex.top"
    @State private var pairing = ""
    @State private var pairingBusy = false
    @State private var pairingError: String?
    @State private var newTask: RemoteDestination?
    @Environment(\.scenePhase) private var scenePhase
    private var visibleDevices: [RemoteDevice] { store.devices.filter { selectedDevice == nil || selectedDevice == $0.id } }
    private var connectedDevices: [RemoteDevice] { visibleDevices.filter { store.online[$0.id] == true } }
    private var multiple: Bool { visibleDevices.count > 1 }
    private func chats(_ device: RemoteDevice) -> [RemoteChat] { (store.overviews[device.id]?.chats ?? []).filter { search.isEmpty || $0.name.localizedCaseInsensitiveContains(search) } }
    private func projects(_ device: RemoteDevice) -> [RemoteProject] { (store.overviews[device.id]?.projects ?? []).filter { search.isEmpty || $0.name.localizedCaseInsensitiveContains(search) } }
    private var hasResults: Bool { visibleDevices.contains { !chats($0).isEmpty || !projects($0).isEmpty } }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Button(action: openSidebar) { Image(systemName: "line.3.horizontal").font(.title3).frame(width: 44, height: 44) }.buttonStyle(.plain).chatGlass(in: Circle(), interactive: true).accessibilityLabel(L10n.tr("打开侧栏")).accessibilityIdentifier("remote-sidebar")
                Spacer()
                Text(L10n.tr("远程")).font(.headline).accessibilityAddTraits(.isHeader)
                Spacer()
                Menu {
                    if store.profile == nil { Button(L10n.tr("登录 Potato 账号"), systemImage: "person.crop.circle") { showLogin = true } }
                    Button(L10n.tr("通过配对码添加电脑"), systemImage: "plus") { showPairing = true }
                    Button(L10n.tr("管理电脑"), systemImage: "laptopcomputer") { showDevices = true }
                    Button(L10n.tr("刷新"), systemImage: "arrow.clockwise") { Task { await store.refresh() } }
                } label: { Image(systemName: "ellipsis").font(.title3).frame(width: 44, height: 44) }.chatGlass(in: Circle()).accessibilityLabel(L10n.tr("远程选项")).accessibilityIdentifier("remote-options")
            }.padding(.horizontal, 18).padding(.vertical, 8)
            if !store.devices.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        deviceChip(L10n.tr("全部"), id: nil, online: nil)
                        ForEach(store.devices) { device in deviceChip(device.name, id: device.id, online: store.online[device.id]) }
                    }.padding(.horizontal, 20).padding(.vertical, 6)
                }.excludesSidebarGesture().accessibilityIdentifier("remote-device-picker")
            }
            if !store.devices.isEmpty, let connectionSummary {
                HStack(spacing: 7) {
                    Text(connectionSummary).lineLimit(2).accessibilityIdentifier("remote-directory-status")
                    Spacer(minLength: 0)
                }.font(.caption).foregroundStyle(Palette.secondary)
                    .frame(minHeight: 28).padding(.horizontal, 24)
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if store.loadingDirectory {
                        directoryPlaceholder
                    } else if store.canShowSetup {
                        VStack(spacing: 14) {
                            Image(systemName: "laptopcomputer.and.iphone").font(.system(size: 44, weight: .light)).foregroundStyle(Palette.secondary).padding(.bottom, 4).accessibilityHidden(true)
                            Text(L10n.tr("连接你的电脑")).font(.title3.weight(.semibold))
                            Text(L10n.tr("在电脑上登录同一账号并开启远程访问。"))
                                .font(.subheadline).foregroundStyle(Palette.secondary).multilineTextAlignment(.center)
                            if store.profile == nil {
                                Button { showLogin = true } label: {
                                    Text(L10n.tr("登录账号")).font(.body.weight(.semibold)).frame(maxWidth: 260, minHeight: 46)
                                        .foregroundStyle(Palette.onAccent).background(Palette.accent, in: Capsule())
                                }.buttonStyle(.plain).accessibilityIdentifier("remote-sign-in")
                            }
                            Button { showPairing = true } label: {
                                Text(L10n.tr("通过配对码添加")).font(.body.weight(.medium)).frame(maxWidth: 260, minHeight: 46)
                                    .overlay { Capsule().stroke(Palette.line, lineWidth: 1) }
                            }.buttonStyle(.plain).accessibilityIdentifier("remote-pair-empty")
                        }.frame(maxWidth: .infinity).padding(.horizontal, 12).padding(.vertical, 48)
                    } else if store.devices.isEmpty {
                        VStack(alignment: .leading, spacing: 12) {
                            Label(L10n.tr("暂时无法读取电脑列表"), systemImage: "wifi.exclamationmark").font(.subheadline)
                            Text(L10n.tr("请检查网络后重试。")).font(.footnote).foregroundStyle(Palette.secondary)
                            Button(L10n.tr("重新连接")) { Task { await store.refresh() } }.frame(minHeight: 44).accessibilityIdentifier("remote-directory-retry")
                        }.padding(.vertical, 28)
                    } else {
                        if visibleDevices.contains(where: { chats($0).contains { $0.pinned == true } }) {
                            sectionTitle(L10n.tr("置顶"))
                            ForEach(visibleDevices) { device in ForEach(chats(device).filter { $0.pinned == true }) { chat in chatRow(chat, device: device) } }
                        }
                        if visibleDevices.contains(where: { !projects($0).isEmpty }) {
                            sectionTitle(L10n.tr("项目"))
                            ForEach(visibleDevices) { device in
                                ForEach(projects(device)) { project in
                                    // The project page carries the new-task button; the list keeps it as a shortcut only.
                                    NavigationLink { RemoteProjectView(device: device, project: project, chats: chats(device).filter { $0.project_path == project.path }, settings: settings) } label: {
                                        HStack(spacing: 14) { Image(systemName: "folder").font(.title3).foregroundStyle(Palette.ink); rowTitle(project.name, device: device); Spacer(minLength: 0) }.frame(minHeight: 48).contentShape(Rectangle())
                                    }.buttonStyle(RemoteDirectoryRowStyle()).accessibilityIdentifier("remote-project-\(device.id)-\(project.path)")
                                        .contextMenu { Button(L10n.tr("在\(project.name)新建任务"), systemImage: "square.and.pencil") { newTask = RemoteDestination(device: device, project: project) } }
                                        .accessibilityAction(named: L10n.tr("在\(project.name)新建任务")) { newTask = RemoteDestination(device: device, project: project) }
                                        .disabled(store.online[device.id] != true)
                                }
                            }
                        }
                        if visibleDevices.contains(where: { !chats($0).filter { $0.pinned != true }.isEmpty }) {
                            sectionTitle(L10n.tr("会话"))
                            ForEach(visibleDevices) { device in ForEach(chats(device).filter { $0.pinned != true }) { chat in chatRow(chat, device: device) } }
                        }
                        if !hasResults {
                            if search.isEmpty && (store.refreshing || visibleDevices.contains { store.online[$0.id] == nil && store.deviceIssues[$0.id] == nil }) {
                                directoryPlaceholder
                            } else if !search.isEmpty {
                                ContentUnavailableView.search(text: search).padding(.top, 30)
                            } else if !connectedDevices.isEmpty {
                                Text(L10n.tr("还没有项目或会话")).font(.subheadline).foregroundStyle(Palette.secondary).padding(.top, 28)
                            }
                        }
                    }
                }.padding(.horizontal, 24).padding(.bottom, 24)
            }.refreshable { await store.refresh() }.scrollDismissesKeyboard(.interactively)
            if !store.canShowSetup { HStack(spacing: 10) {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").font(.title3)
                    TextField(L10n.tr("搜索会话"), text: $search).submitLabel(.search).accessibilityIdentifier("remote-search")
                    if !search.isEmpty { Button { search = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Palette.secondary) }.accessibilityLabel(L10n.tr("清除搜索")) }
                }.padding(.horizontal, 16).frame(minHeight: 48).background(Palette.surface.opacity(0.95), in: Capsule())
                composeButton(voice: true)
                composeButton(voice: false)
            }.padding(.horizontal, 22).padding(.top, 8).padding(.bottom, 12)
                .excludesSidebarGesture() }
        }.foregroundStyle(Palette.ink).background(Palette.canvas.ignoresSafeArea()).toolbar(.hidden, for: .navigationBar)
            .navigationDestination(item: $newTask) { destination in RemoteTaskView(device: destination.device, project: destination.project, settings: settings, startWithVoice: destination.voice) }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                while !Task.isCancelled { await store.refresh(); try? await Task.sleep(for: .seconds(5)) }
            }
            .onChange(of: store.devices) { _, devices in
                if let selectedDevice, !devices.contains(where: { $0.id == selectedDevice }) { self.selectedDevice = nil }
            }
            .sheet(isPresented: $showLogin) { loginSheet }
            .onChange(of: store.profile?.owner) { _, value in if value != nil { showLogin = false } }
            .sheet(isPresented: $showDevices) { deviceManagement }
            .sheet(isPresented: $showPairing, onDismiss: { pairingError = nil }) { pairingSheet }
    }
    private var connectionSummary: String? {
        let hasSavedList = visibleDevices.contains { store.overviews[$0.id] != nil }
        if store.error != nil || visibleDevices.contains(where: { store.deviceIssues[$0.id] != nil }) {
            return hasSavedList ? L10n.tr("暂时无法更新，显示上次列表") : L10n.tr("暂时无法连接，下拉重试")
        }
        let offline = visibleDevices.filter { store.online[$0.id] == false }.count
        if offline > 0 {
            let name = visibleDevices.count == 1 ? L10n.tr("电脑离线") : L10n.tr("\(offline) 台电脑离线")
            return name + (hasSavedList ? L10n.tr(" · 已保留上次列表") : L10n.tr("，连接后自动更新"))
        }
        return nil
    }
    private var directoryPlaceholder: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionTitle(L10n.tr("项目"))
            HStack(spacing: 14) {
                Image(systemName: "folder").font(.title3).foregroundStyle(Palette.secondary.opacity(0.35))
                RoundedRectangle(cornerRadius: 5).fill(Palette.ink.opacity(0.06)).frame(width: 140, height: 15)
            }.frame(height: 48)
            sectionTitle(L10n.tr("会话"))
            ForEach([220.0, 165, 250, 190], id: \.self) { width in
                RoundedRectangle(cornerRadius: 5).fill(Palette.ink.opacity(0.06))
                    .frame(maxWidth: width).frame(height: 15).frame(height: 52)
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityElement(children: .ignore).accessibilityLabel(L10n.tr("正在读取项目和会话"))
            .accessibilityIdentifier("remote-directory-loading")
    }
    private func sectionTitle(_ name: String) -> some View { Text(name).font(.headline).padding(.top, 26).padding(.bottom, 12).accessibilityAddTraits(.isHeader) }
    private func rowTitle(_ title: String, device: RemoteDevice) -> some View {
        VStack(alignment: .leading, spacing: 3) { Text(title).font(.body).foregroundStyle(Palette.ink).lineLimit(1); if multiple { Text(device.name).font(.caption2).foregroundStyle(Palette.secondary).lineLimit(1) } }
    }
    private func deviceChip(_ name: String, id: String?, online: Bool?) -> some View {
        Button { selectedDevice = id } label: {
            HStack(spacing: 7) {
                if id != nil {
                    Circle().fill(online == true ? Color(red: 0.12, green: 0.71, blue: 0.5) : Color.gray.opacity(online == false ? 1 : 0.35)).frame(width: 7, height: 7)
                }
                Text(name).lineLimit(1)
            }
                .font(.subheadline).padding(.horizontal, 15).frame(minHeight: 38)
                .foregroundStyle(selectedDevice == id ? Palette.onInk : Palette.ink)
                .background(selectedDevice == id ? Palette.ink : Palette.muted, in: Capsule()).padding(.vertical, 3)
        }.buttonStyle(.plain).accessibilityAddTraits(selectedDevice == id ? [.isSelected] : [])
            .accessibilityIdentifier("remote-device-\(id ?? "all")")
            .accessibilityValue(id.flatMap { value in store.devices.first { $0.id == value } }.map { store.connectionLabel($0) } ?? "")
    }
    private func chatRow(_ chat: RemoteChat, device: RemoteDevice) -> some View {
        NavigationLink { RemoteTaskView(device: device, chat: chat, settings: settings) } label: {
            HStack { rowTitle(chat.name, device: device); Spacer(minLength: 8); if chat.status == "running" { Text(store.online[device.id] == true ? L10n.tr("执行中") : L10n.tr("上次执行中")).font(.caption).foregroundStyle(Palette.secondary) } }.frame(minHeight: 52).contentShape(Rectangle())
        }.buttonStyle(RemoteDirectoryRowStyle()).disabled(store.online[device.id] != true)
            .accessibilityIdentifier("remote-chat-\(device.id)-\(chat.id)")
    }
    @ViewBuilder private func composeButton(voice: Bool) -> some View {
        if connectedDevices.count > 1 {
            Menu { ForEach(connectedDevices) { device in Button(device.name) { newTask = RemoteDestination(device: device, voice: voice) } } } label: { composeIcon(voice: voice) }.accessibilityLabel(voice ? L10n.tr("选择电脑并语音输入") : L10n.tr("选择电脑并新建任务"))
        } else {
            Button { if let device = connectedDevices.first { newTask = RemoteDestination(device: device, voice: voice) } else if store.devices.isEmpty { showLogin = store.profile == nil } } label: { composeIcon(voice: voice) }
                .disabled(connectedDevices.isEmpty)
                .accessibilityLabel(voice ? L10n.tr("语音指令") : L10n.tr("新远程任务")).accessibilityIdentifier(voice ? "remote-voice" : "remote-new-task")
        }
    }
    private func composeIcon(voice: Bool) -> some View { Image(systemName: voice ? "waveform" : "square.and.pencil").font(.system(size: 21)).foregroundStyle(voice ? Palette.ink : Palette.onAccent).frame(width: 46, height: 46).background(voice ? Palette.surface : Palette.accent, in: Circle()) }
    private var deviceManagement: some View {
        NavigationStack {
            List {
                if let profile = store.profile {
                    Section(L10n.tr("Potato 账号")) {
                        Label(profile.email, systemImage: "person.crop.circle")
                        Button(L10n.tr("退出这台 iPhone 的登录"), role: .destructive) { Task { await store.logout() } }
                    }
                }
                ForEach(store.devices) { device in Section(device.name) {
                    Label(store.connectionLabel(device), systemImage: "laptopcomputer")
                    if device.owner != nil {
                        Text(L10n.tr("撤销后，所有手机都无法再控制这台电脑。")).font(.footnote).foregroundStyle(Palette.secondary)
                        Button(L10n.tr("撤销此电脑的远程访问"), role: .destructive) { Task { await store.revoke(device); if selectedDevice == device.id { selectedDevice = nil } } }
                    } else {
                        Text(L10n.tr("只移除这台 iPhone 上的配对。")).font(.footnote).foregroundStyle(Palette.secondary)
                        Button(L10n.tr("从这台 iPhone 移除"), role: .destructive) { store.forget(device); if selectedDevice == device.id { selectedDevice = nil } }
                    }
                } }
                Button(L10n.tr("添加电脑")) { showDevices = false; showPairing = true }
            }.warmGroupedBackground().navigationTitle(L10n.tr("管理电脑")).navigationBarTitleDisplayMode(.inline).toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { showDevices = false } } }
        }
    }
    private var loginSheet: some View {
        NavigationStack {
            Form {
                Section {
                    Label(L10n.tr("使用 Potato 账号关联"), systemImage: "person.crop.circle").font(.headline)
                    if settings.developerMode == true { TextField(L10n.tr("服务地址"), text: $loginRelay).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL) }
                }
                if let login = store.login {
                    Section(L10n.tr("核对登录验证码")) {
                        Text(login.code).font(.title.monospaced()).textSelection(.enabled)
                        Text(L10n.tr("在浏览器中确认验证码一致后返回。")).font(.subheadline)
                        Button(L10n.tr("打开登录页面")) { UIApplication.shared.open(login.verification_url) }
                        Button(L10n.tr("我已登录，刷新")) { Task { await store.refresh() } }
                        Button(L10n.tr("重新开始")) { store.cancelLogin() }
                    }
                } else {
                    Button { Task { if let url = await store.beginLogin(relay: loginRelay) { await UIApplication.shared.open(url) } } } label: {
                        HStack { if store.signingIn { ProgressView() }; Text(L10n.tr("继续登录")) }
                    }.disabled(store.signingIn)
                }
                if let error = store.error { Text(error).font(.footnote).foregroundStyle(.red) }
            }.navigationTitle(L10n.tr("登录 Potato")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { store.cancelLogin(); showLogin = false }.disabled(store.signingIn) } }
        }
    }
    private var pairingSheet: some View {
        NavigationStack {
            Form {
                Section(L10n.tr("连接你的电脑")) {
                    Label(L10n.tr("电脑 Potato → 设置 → 能力 → iPhone 远程控制"), systemImage: "laptopcomputer")
                    SecureField(L10n.tr("粘贴完整配对码"), text: $pairing).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("remote-pairing-code")
                    Button { pairingBusy = true; pairingError = nil; Task { pairingError = await store.pair(pairing); pairingBusy = false; if pairingError == nil { pairing = ""; showPairing = false } } } label: { HStack { if pairingBusy { ProgressView() }; Text(L10n.tr("连接电脑")) } }.disabled(pairing.isEmpty || pairingBusy).accessibilityIdentifier("remote-pair-submit")
                }
                if let pairingError { Section { Text(pairingError).foregroundStyle(.red) } }
            }.navigationTitle(L10n.tr("添加电脑")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { showPairing = false }.disabled(pairingBusy) } }
        }.interactiveDismissDisabled(pairingBusy)
    }
}
private struct RemoteDestination: Identifiable, Hashable {
    let id = UUID(); let device: RemoteDevice; var project: RemoteProject?; var voice = false
}

private struct RemoteProjectView: View {
    let device: RemoteDevice; let project: RemoteProject; let chats: [RemoteChat]; let settings: ConnectionSettings
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Label(device.name, systemImage: "laptopcomputer").font(.caption).foregroundStyle(Palette.secondary)
                NavigationLink { RemoteTaskView(device: device, project: project, settings: settings) } label: { Label(L10n.tr("在此项目新建任务"), systemImage: "square.and.pencil").frame(minHeight: 48) }
                Text(L10n.tr("项目会话")).font(.headline).padding(.top, 12)
                ForEach(chats) { chat in NavigationLink { RemoteTaskView(device: device, chat: chat, settings: settings) } label: { Text(chat.name).frame(maxWidth: .infinity, minHeight: 48, alignment: .leading) }.buttonStyle(.plain) }
            }.padding(24)
        }.background(Palette.canvas).navigationTitle(project.name).navigationBarTitleDisplayMode(.inline)
    }
}

struct RemoteTaskView: View {
    let device: RemoteDevice
    let project: RemoteProject?
    let settings: ConnectionSettings
    let startWithVoice: Bool
    @StateObject private var draft: RemoteDraftSession
    @StateObject private var dictation = VoiceComposer()
    @State private var appeared = false
    @State private var chat: RemoteChat?
    @State private var snapshot: RemoteSnapshot?
    @State private var observation = RemoteTaskObservation()
    @State private var observedNow = Date()
    @State private var modelCatalog: RemoteModelCatalog?
    @State private var modelIssue: String?
    @State private var modelsLoading = false
    @State private var showModels = false
    @State private var showLegacyRecovery = false
    @State private var showOtherDrafts = false
    @State private var error: String?
    @State private var busy = false
    @State private var loading = false
    @State private var mutationRevision = 0
    @State private var presentedApproval: RemoteApproval?
    @State private var seenApprovals: Set<String> = []
    @State private var resolvedApprovals: Set<String> = []
    @State private var approvalError: String?
    @State private var approvalBusy = false
    @State private var stopConfirmation = false
    @State private var stopTarget: RemoteStopRequest?
    @State private var receiptNotice: String?
    @State private var reviewedPending: RemotePendingSend?
    @State private var showUnconfirmedRecords = false
    @State private var followBottom = true
    @State private var latestOffscreen = true
    @State private var promptFocused = false
    @State private var promptSelection: NSRange?
    @State private var inputExpanded = false
    @State private var expandAfterVoice = false
    @State private var restoreInputAfterExpansion = false
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    init(device: RemoteDevice, chat: RemoteChat? = nil, project: RemoteProject? = nil, settings: ConnectionSettings = ConnectionSettings(), startWithVoice: Bool = false) {
        self.device = device; self.project = project; self.settings = settings; self.startWithVoice = startWithVoice; _chat = State(initialValue: chat)
        _draft = StateObject(wrappedValue: RemoteDraftSession(device: device, chatID: chat?.id, projectPath: project?.path))
    }
    private var text: String { draft.text }
    private var pending: RemotePendingSend? { draft.pending }
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    private var reduceMotion: Bool {
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--reduce-motion-preview") { return true }
        #endif
        return systemReduceMotion
    }
    private var confirmed: Bool { observation.isCurrent(at: observedNow) && scenePhase == .active }
    /// What the page shows: the last known state until polls have really fallen behind.
    /// Stopping, approving and sending still wait for `confirmed`.
    private var stale: Bool { chat != nil && observation.isStale(at: observedNow) }
    private var settled: Bool { scenePhase == .active && !stale }
    private var running: Bool { snapshot?.status == "running" }
    private var supportsQueue: Bool { snapshot?.outbox_protocol == 1 }
    private var usesQueue: Bool { supportsQueue && (running || snapshot?.outbox?.items.isEmpty == false) }
    private var canAct: Bool { chat == nil || confirmed }
    private var canSend: Bool { !busy && canAct && snapshot?.outbox?.interrupt != true && pending == nil && draft.storageError == nil && !dictation.active && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    private var activityVisible: Bool { settled && running && snapshot?.needsUserResponse != true }
    /// A confirmed thought is named by its own process row; a status line would only repeat it.
    private var processRowSpeaks: Bool { activityVisible && confirmed && snapshot?.isThinking == true }

    /// The model the next message will use: the chosen one, else the computer's own.
    private var modelParts: (name: String, effort: String?) {
        guard let choice = draft.modelChoice ?? modelCatalog?.active else { return (L10n.tr("跟随电脑"), nil) }
        let name = modelCatalog?.models.first(where: { $0.matches(choice) })?.name ?? choice.model
        return (name, remoteEffortName(choice.reasoning_effort))
    }
    private var modelLabel: String { [modelParts.name, modelParts.effort].compactMap { $0 }.joined(separator: " · ") }
    private var scrollRevision: String {
        guard let snapshot else { return "empty" }
        let messages = snapshot.displayMessages
        let queueRevision = snapshot.outbox?.items.map { "\($0.id):\($0.text):\($0.state)" }.joined(separator: "|") ?? ""
        return "\(queueRevision)|\(pending?.id ?? "")|\(snapshot.outbox?.interrupt ?? false)|\(messages.count)|\(messages.last?.text ?? "")|\(snapshot.approvals.map(\.id))|\(snapshot.questions.filter { $0.status == "pending" }.map(\.id))|\(snapshot.status)|\(snapshot.outcome?.status ?? "")"
    }
    @ViewBuilder private var conversationStatus: some View {
        if chat != nil && !processRowSpeaks {
            if snapshot == nil && observation.failure == nil {
                ReplyPendingDot().accessibilityElement().accessibilityLabel(L10n.tr("正在读取任务状态"))
            } else {
                let title = activityVisible ? snapshot?.phaseTitle ?? "" : observation.title(snapshot: snapshot, at: observedNow)
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        if activityVisible {
                            // One line while the computer works: a pulsing dot and what it is doing.
                            if reduceMotion { Image(systemName: "ellipsis").accessibilityLabel(title).accessibilityIdentifier("remote-static-activity") }
                            else { ReplyPendingDot().accessibilityElement().accessibilityLabel(title).accessibilityIdentifier("remote-activity-spinner") }
                            Text(title).font(.subheadline).lineLimit(1).shimmering(!reduceMotion).accessibilityHidden(true)
                        } else {
                            if stale { Image(systemName: observation.failure == nil ? "arrow.triangle.2.circlepath" : "wifi.exclamationmark").font(.system(size: 16)) }
                            Text(title).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("remote-current-status")
                        }
                        Spacer(minLength: 0)
                        if stale { Button(L10n.tr("刷新")) { Task { await refresh() } }.fixedSize().frame(minHeight: 44).disabled(loading).accessibilityIdentifier("remote-status-refresh") }
                    }
                    if stale, let snapshot { Text(L10n.tr("上次确认：\(snapshot.activityTitle)")).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("remote-last-status") }
                }.font(.footnote).foregroundStyle(Palette.secondary).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                    .frame(maxWidth: .infinity, alignment: .leading).accessibilityElement(children: .contain)
            }
        }
    }

    /// "Potato · MacBook Pro": the project first, as on the desktop.
    private var headerSubtitle: String {
        let path = project?.path ?? chat?.project_path
        let name = project?.name ?? path.map { ($0 as NSString).lastPathComponent }
        return [name, device.name].compactMap { $0?.isEmpty == false ? $0 : nil }.joined(separator: " · ")
    }
    private var promptPlaceholder: String {
        if usesQueue { return L10n.tr("排队到本轮之后…") }
        return running ? L10n.tr("补充当前任务…") : L10n.tr("问问 Potato")
    }
    private var hasPrompt: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    private var remoteComposer: some View {
        VStack(alignment: .leading, spacing: 6) {
            if dictation.active {
                VoiceComposerPanel(voice: dictation, text: text, maximumHeight: dynamicTypeSize.isAccessibilitySize ? 120 : 220) {
                    expandAfterVoice = true; dictation.finish(send: false)
                }
            } else {
            ZStack(alignment: .topLeading) {
                if text.isEmpty {
                    Text(promptPlaceholder).font(.body).foregroundStyle(Palette.secondary)
                        .allowsHitTesting(false).accessibilityHidden(true)
                }
                ComposerTextInput(text: $draft.text, selection: $promptSelection, focused: $promptFocused,
                                  placeholder: promptPlaceholder,
                                  minimumHeight: 40,
                                  maximumHeight: UIFont.preferredFont(forTextStyle: .body).lineHeight * (dynamicTypeSize.isAccessibilitySize ? 3 : 6) + 4,
                                  identifier: "remote-prompt")
                    .disabled(busy || dictation.active)
            }.padding(.horizontal, 6).padding(.top, 4)
            HStack(spacing: 4) {
                Button { promptFocused = false; showModels = true; Task { await reloadModels() } } label: {
                    HStack(spacing: 6) {
                        (Text(modelParts.name) + Text(modelParts.effort.map { " " + $0 } ?? "").foregroundStyle(Palette.secondary)).lineLimit(1)
                        if !running || supportsQueue { Image(systemName: "chevron.down").font(.system(size: 11)) }
                    }.font(.subheadline).padding(.horizontal, 12).frame(minHeight: 36)
                        .background(Palette.ink.opacity(0.045), in: Capsule()).frame(minHeight: 44).contentShape(Capsule())
                }.accessibilityIdentifier("remote-model-settings").accessibilityLabel(L10n.tr("模型与思考，\(modelLabel)"))
                    .disabled(busy || pending != nil || (running && !supportsQueue) || !canAct).composerControl()
                Spacer(minLength: 0)
                if !dictation.active {
                    Button { startDictation() } label: {
                        Image(systemName: "mic").font(.system(size: 20)).frame(width: 44, height: 44)
                            .background { Circle().fill(Palette.ink.opacity(0.045)).frame(width: 36, height: 36) }
                    }.accessibilityLabel(L10n.tr("语音输入")).accessibilityIdentifier("remote-voice-input").disabled(busy || pending != nil || draft.storageError != nil).composerControl()
                }
                if running, let target = snapshot.flatMap(RemoteStopRequest.init(snapshot:)) {
                    Button { stopTarget = target; stopConfirmation = true } label: {
                        Image(systemName: "stop.fill").font(.system(size: 16, weight: .medium))
                            .foregroundStyle(hasPrompt ? Palette.accent : Palette.onAccent).frame(width: 38, height: 38)
                            .background(hasPrompt ? Palette.canvas : Palette.accent, in: Circle())
                            .frame(width: 44, height: 44).contentShape(Circle())
                    }.disabled(busy || !confirmed).accessibilityLabel(L10n.tr("停止任务")).accessibilityIdentifier("remote-stop").composerControl()
                }
                if hasPrompt || !running || snapshot.flatMap(RemoteStopRequest.init(snapshot:)) == nil {
                    Button { send() } label: {
                        if busy { ProgressView().frame(width: 44, height: 44) }
                        else {
                            Image(systemName: "arrow.up").font(.system(size: 21, weight: .medium))
                                .foregroundStyle(Palette.onAccent).frame(width: 38, height: 38)
                                .background(canSend ? Palette.accent : Palette.secondary.opacity(0.35), in: Circle())
                                .frame(width: 44, height: 44).contentShape(Circle())
                        }
                    }.disabled(!canSend).accessibilityLabel(usesQueue ? L10n.tr("发送消息，默认排队") : running ? L10n.tr("补充当前任务") : L10n.tr("发送到电脑")).accessibilityIdentifier("remote-send").composerControl()
                }
            }
            }
        }.padding(8).chatGlass(in: RoundedRectangle(cornerRadius: 26, style: .continuous))
            .composerWhitespaceFocus(enabled: !busy && !dictation.active) { promptFocused = true }
            .overlay(alignment: .top) {
                if dictation.active { Label(L10n.tr("上滑发送"), systemImage: "chevron.up").font(.system(size: 10)).foregroundStyle(.tertiary).offset(y: -19).accessibilityHidden(true) }
            }
            .excludesSidebarGesture()
    }

    var body: some View {
        Group {
            ScrollViewReader { proxy in
            ScrollView {
                // Snapshots are bounded to 120 messages. Eager layout avoids
                // lazy height estimation oscillating with keyboard resizing and scrollTo.
                VStack(alignment: .leading, spacing: 24) {
                    if let error { Label(error, systemImage: "exclamationmark.circle").font(.subheadline).foregroundStyle(.red); Button(L10n.tr("刷新任务状态")) { Task { await refresh() } } }
                    if let receiptNotice { Text(receiptNotice).font(.subheadline).foregroundStyle(Palette.secondary).accessibilityIdentifier("remote-recovered-send") }
                    if let snapshot {
                        let rows = RemoteConversationRow.make(snapshot.displayMessages)
                        ForEach(rows) { row in
                            RemoteConversationRowView(row: row, running: running && row.id == rows.last(where: \.isAssistantTurn)?.id, confirmed: settled, activeProcessID: snapshot.activeProcessID, onExpand: { followBottom = false; promptFocused = false })
                        }
                        ForEach(snapshot.approvals.filter { !resolvedApprovals.contains($0.id) }) { approval in
                            Button {
                                promptFocused = false; approvalError = nil
                                seenApprovals.insert(approval.id); presentedApproval = approval
                            } label: {
                                HStack(spacing: 12) {
                                    Image(systemName: "hand.raised.fill")
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(L10n.tr("需要你的批准")).font(.headline)
                                        Text(approval.findings_summary ?? approval.tool_name ?? L10n.tr("电脑操作")).font(.subheadline).lineLimit(2)
                                    }
                                    Spacer(); Image(systemName: "chevron.right")
                                }.padding(16).frame(minHeight: 56)
                                    .background(Palette.muted, in: RoundedRectangle(cornerRadius: 18))
                            }.buttonStyle(.plain).accessibilityIdentifier("remote-open-approval")
                        }
                        ForEach(snapshot.questions.filter { $0.status == "pending" }) { question in RemoteQuestionCard(question: question, busy: busy || !confirmed) { args in act("answer", args) } }
                        if !running && snapshot.outcome?.status == "failed" { Label(snapshot.outcome?.error?.message ?? L10n.tr("本轮任务失败，请检查后重试。"), systemImage: "exclamationmark.circle").font(.subheadline).foregroundStyle(.red) }
                    } else if chat == nil { if !dictation.active { VStack(spacing: 10) {
                        BrandIllustration(scene: .computer).padding(.bottom, 6)
                        Text(L10n.tr("让电脑帮你做点什么")).font(.title2.weight(.semibold)).foregroundStyle(Palette.ink)
                        Text(project.map { L10n.tr("在“\($0.name)”项目中开始对话") } ?? L10n.tr("发条消息，在电脑上继续完成。"))
                            .font(.subheadline).foregroundStyle(Palette.secondary)
                    }.multilineTextAlignment(.center).frame(maxWidth: .infinity).padding(.vertical, 64) } }
                    if settled && (running || ["cancelled", "failed"].contains(snapshot?.outcome?.status ?? "")) { conversationStatus }
                    if running && snapshot.flatMap(RemoteStopRequest.init(snapshot:)) == nil {
                        Text(L10n.tr("电脑端版本过旧，请在电脑上停止任务。"))
                            .font(.footnote).foregroundStyle(Palette.secondary)
                            .dynamicTypeSize(...DynamicTypeSize.xxxLarge).accessibilityIdentifier("remote-stop-update-required")
                    }
                    RemoteQueuedMessages(queue: snapshot?.outbox, pending: pending, sending: busy,
                                         delivered: Set(snapshot?.displayMessages.compactMap(\.remoteOperationID) ?? []),
                                         enabled: confirmed && !busy && pending == nil, action: queueAction)
                    Color.clear.frame(height: 1).id("remote-bottom")
                }.padding(20).background(ScrollActivityObserver { followBottom = false; promptFocused = false })
            }.accessibilityIdentifier("remote-conversation").scrollClipDisabled()
                .onLatestOffscreenChange { offscreen in latestOffscreen = offscreen; if !offscreen { followBottom = true } }
                .refreshable { await refresh() }
                .scrollDismissesKeyboard(.interactively)
                .contentShape(Rectangle())
                .simultaneousGesture(TapGesture().onEnded { promptFocused = false })
                .onAppear { if chat != nil { proxy.scrollTo("remote-bottom", anchor: .bottom) } }
                .onChange(of: scrollRevision) { _, _ in if followBottom { proxy.scrollTo("remote-bottom", anchor: .bottom) } }
                .overlay(alignment: .bottom) {
                    if !followBottom && latestOffscreen {
                        IconButton(symbol: "arrow.down", label: L10n.tr("回到最新消息"), id: "remote-scroll-latest") {
                            followBottom = true; proxy.scrollTo("remote-bottom", anchor: .bottom)
                        }.chatGlass(in: Circle(), interactive: true).padding(.bottom, 10)
                    }
                }
            }
            .chatBar(edge: .bottom) {
                VStack(spacing: 8) {
                    if stale {
                        conversationStatus.padding(12)
                            .background(Palette.canvas, in: RoundedRectangle(cornerRadius: 18))
                    }
                    if let issue = draft.storageError { Text(issue).font(.footnote).foregroundStyle(.red) }
                    if let issue = draft.legacyError { Text(issue).font(.footnote).foregroundStyle(Palette.secondary) }
                    if draft.legacy != nil { Button(L10n.tr("恢复旧版草稿")) { showLegacyRecovery = true }.frame(minHeight: 44).accessibilityIdentifier("remote-legacy-draft") }
                    if !draft.otherDrafts.isEmpty { Button(L10n.tr("查看另外保存的草稿")) { showOtherDrafts = true }.frame(minHeight: 44).accessibilityIdentifier("remote-other-drafts") }
                    if !draft.archived.isEmpty { Button(L10n.tr("未确认指令记录（\(draft.archived.count)）")) { showUnconfirmedRecords = true }.frame(minHeight: 44).accessibilityIdentifier("remote-unconfirmed-records") }
                    if let pending, !busy {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(L10n.tr("发送结果未确认。")).font(.footnote).foregroundStyle(Palette.secondary)
                            Text(pending.text).font(.footnote).lineLimit(2)
                            if let choice = pending.modelChoice { Text("\(choice.model) · \(remoteEffortName(choice.reasoning_effort))").font(.caption).foregroundStyle(Palette.secondary) }
                            Button(L10n.tr("重试确认发送结果")) { transmit(pending) }.frame(minHeight: 44)
                            Button(L10n.tr("核对并处理这条指令")) { promptFocused = false; reviewedPending = pending }.frame(minHeight: 44).accessibilityIdentifier("remote-review-pending")
                        }.padding(12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 16))
                    }
                    if let notice = dictation.notice { Text(notice).font(.footnote).foregroundStyle(Palette.secondary).accessibilityIdentifier("voice-notice") }
                    remoteComposer
                }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 16).padding(.bottom, 8).padding(.top, dictation.active ? 24 : 6)
            }.chatScrollEdges()
        }.foregroundStyle(Palette.ink).background(Palette.canvas.ignoresSafeArea())
            .navigationTitle(chat?.name ?? L10n.tr("新对话")).navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.hidden, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 3) {
                        Text(chat?.name ?? L10n.tr("新对话")).font(.headline).lineLimit(1)
                        Text(headerSubtitle).font(.footnote).foregroundStyle(Palette.secondary).lineLimit(1)
                            .accessibilityLabel(L10n.tr("执行电脑，\(device.name)"))
                    }.dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                        .accessibilityIdentifier("remote-conversation-title")
                }
                if let chat {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button(chat.pinned == true ? L10n.tr("取消置顶") : L10n.tr("置顶对话"), systemImage: chat.pinned == true ? "pin.slash" : "pin") { act("pin", ["pinned": !(chat.pinned ?? false)]) }.disabled(busy || !confirmed)
                            Button(L10n.tr("刷新对话"), systemImage: "arrow.clockwise") { Task { await refresh() } }.disabled(loading)
                        } label: { Image(systemName: "ellipsis").frame(width: 28, height: 28) }
                            .accessibilityLabel(L10n.tr("对话选项")).accessibilityIdentifier("remote-conversation-options")
                    }
                }
            }
            .confirmationDialog(L10n.tr("停止刚才查看的这一轮任务？"), isPresented: $stopConfirmation, titleVisibility: .visible) { Button(L10n.tr("停止任务"), role: .destructive) { stop() } }
            .sheet(item: $presentedApproval) { approval in
                RemoteApprovalSheet(approval: approval,
                    confirmed: confirmed, busy: approvalBusy, issue: approvalError, supportsScopes: snapshot?.approval_scope_protocol == 1,
                    available: snapshot?.approvals.contains { $0.id == approval.id } == true && !resolvedApprovals.contains(approval.id),
                    dismiss: { presentedApproval = nil }, respond: { scope in respondToApproval(approval, scope: scope) })
            }
            .sheet(isPresented: $showModels) {
                RemoteModelPicker(deviceName: device.name, catalog: modelCatalog, issue: modelIssue, loading: modelsLoading, selection: draft.modelChoice, choose: { choice in
                    do { try draft.chooseModel(choice); modelIssue = nil } catch { modelIssue = error.localizedDescription }
                }, reload: { Task { await reloadModels() } })
            }
            .sheet(isPresented: $showLegacyRecovery) {
                NavigationStack {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            Text(L10n.tr("旧版没有完整保存草稿的目标电脑。请先核对，这份内容是否属于“\(device.name)”。"))
                            if let legacy = draft.legacy {
                                Text(legacy.text.isEmpty ? legacy.pending?.text ?? "" : legacy.text).textSelection(.enabled)
                                if let pending = legacy.pending {
                                    Text(L10n.tr("待确认指令")).font(.headline)
                                    Text(pending.text).textSelection(.enabled)
                                    Text(L10n.tr("它可能已经执行，请先在电脑上确认。")).font(.footnote).foregroundStyle(.secondary)
                                }
                            }
                            Button(L10n.tr("确认属于这台电脑并恢复")) {
                                do {
                                    try draft.restoreLegacy()
                                    if draft.address.kind == "chat", chat?.id != draft.address.value {
                                        chat = RemoteChat(id: draft.address.value, session_id: "", name: L10n.tr("恢复的远程任务"), status: nil, pinned: nil, project_path: project?.path)
                                        Task { await refresh() }
                                    }
                                    showLegacyRecovery = false
                                } catch { self.error = error.localizedDescription; showLegacyRecovery = false }
                            }.frame(minHeight: 44).accessibilityIdentifier("remote-confirm-legacy-draft")
                        }.padding(20)
                    }.navigationTitle(L10n.tr("核对旧版草稿")).navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { showLegacyRecovery = false } } }
                }
            }
            .sheet(isPresented: $showOtherDrafts) {
                NavigationStack {
                    List(draft.otherDrafts, id: \.self) { value in
                        VStack(alignment: .leading, spacing: 12) {
                            Text(value).textSelection(.enabled)
                            Button(L10n.tr("恢复到输入框")) {
                                do { try draft.chooseOtherDraft(value); showOtherDrafts = false }
                                catch { self.error = error.localizedDescription; showOtherDrafts = false }
                            }.frame(minHeight: 44)
                        }
                    }.navigationTitle(L10n.tr("另外保存的草稿")).navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("关闭")) { showOtherDrafts = false } } }
                }
            }
            .sheet(item: $reviewedPending) { request in
                RemotePendingReview(request: request, deviceName: device.name) {
                    try draft.archiveUnconfirmed(request)
                    error = nil; reviewedPending = nil
                }
            }
            .sheet(isPresented: $showUnconfirmedRecords) {
                NavigationStack {
                    List {
                        Text(L10n.tr("以下指令可能已执行，请先在电脑上确认。")).font(.subheadline)
                        ForEach(draft.archived, id: \.id) { request in
                            RemoteUnconfirmedDetails(request: request, deviceName: device.name)
                        }
                    }.navigationTitle(L10n.tr("未确认指令记录")).navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("关闭")) { showUnconfirmedRecords = false } } }
                }
            }
            .fullScreenCover(isPresented: $inputExpanded, onDismiss: {
                if restoreInputAfterExpansion { promptFocused = true }
            }) {
                ExpandedComposer(text: $draft.text, selection: $promptSelection, canSend: canSend,
                                 collapse: { inputExpanded = false }, send: {
                    restoreInputAfterExpansion = false; inputExpanded = false; send()
                })
            }
            .onChange(of: dictation.editRevision) { _, _ in
                promptSelection = dictation.finalSelection
                if expandAfterVoice {
                    expandAfterVoice = false; restoreInputAfterExpansion = true; inputExpanded = true
                } else { promptFocused = true }
            }
            .onChange(of: dictation.active) { _, active in if !active { promptSelection = dictation.finalSelection } }
            .onAppear {
                guard !appeared else { return }; appeared = true
                draft.load()
                if startWithVoice && pending == nil && draft.storageError == nil { startDictation() }
            }
            .onDisappear { observation.suspend(); dictation.interrupt(); draft.flush() }
            .onChange(of: scenePhase) { _, phase in
                dictation.foreground = phase == .active
                if phase != .active { observation.suspend(); draft.flush() }
                if phase == .background || (phase == .inactive && dictation.phase != .connecting) { dictation.interrupt() }
            }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }; observation.resume()
                while !Task.isCancelled { await refresh(); try? await Task.sleep(for: .seconds(2)) }
            }
            .task(id: scenePhase) { guard scenePhase == .active else { return }; await reloadModels() }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                while !Task.isCancelled {
                    let now = Date()
                    // Only invalidate the view when freshness changes; long Markdown
                    // replies should not be reconstructed for an invisible clock tick.
                    if observation.isCurrent(at: observedNow) != observation.isCurrent(at: now) || observation.isStale(at: observedNow) != observation.isStale(at: now) { observedNow = now }
                    try? await Task.sleep(for: .seconds(1))
                }
            }
    }
    private func reloadModels() async {
        guard !modelsLoading else { return }; modelsLoading = true; defer { modelsLoading = false }
        do {
            let result: RemoteModelOverview = try await RemoteService.rpc(device, op: "overview")
            guard !Task.isCancelled else { return }
            modelCatalog = result.model_catalog; modelIssue = nil
        } catch { if !Task.isCancelled { modelIssue = error.localizedDescription } }
    }
    private func refresh() async {
        guard let chat, !loading, scenePhase == .active else { return }; loading = true; defer { loading = false }; let requestedAt = Date(); let revision = mutationRevision
        do { let value: RemoteSnapshot = try await RemoteService.rpc(device, op: "chat", args: ["chat_id": chat.id]); guard !Task.isCancelled, scenePhase == .active, revision == mutationRevision else {return}; snapshot = value; self.chat = value.chat; observation.received(requestedAt: requestedAt); observedNow = Date(); presentNextApproval() }
        catch { if !Task.isCancelled, revision == mutationRevision { observation.failed(ChatService.failureDescription(error)); observedNow = Date() } }
    }
    private func startDictation() {
        guard !busy, pending == nil, draft.storageError == nil else { return }
        let selection = promptSelection
        expandAfterVoice = false; promptFocused = false; dictation.foreground = scenePhase == .active
        dictation.start(settings: settings, original: text, selection: selection,
                        update: { draft.text = $0 }, persist: { draft.flush() }, send: { send() })
    }
    private func send() {
        guard canSend, scenePhase == .active, chat == nil || observation.isCurrent(at: Date()) else { return }
        if !usesQueue { promptFocused = false }
        followBottom = true
        if usesQueue {
            do { transmit(try draft.prepareSend(modelChoice: draft.modelChoice, deliveryMode: .queue)) }
            catch { self.error = error.localizedDescription }
            return
        }
        if running {
            do { transmit(try draft.prepareSend(expectedRunID: snapshot?.running_request_id)) }
            catch { self.error = error.localizedDescription }
            return
        }
        busy = true
        Task {
            do {
                // Resolve desktop defaults once, then persist the exact choice before dispatch.
                let overview: RemoteModelOverview = try await RemoteService.rpc(device, op: "overview")
                modelCatalog = overview.model_catalog
                let choice: RemoteModelChoice?
                if let catalog = overview.model_catalog { choice = try catalog.resolve(draft.modelChoice) }
                else {
                    guard draft.modelChoice == nil else { throw LocalFailure.message(L10n.tr("请更新电脑端，以使用所选模型与思考设置。")) }
                    choice = nil
                }
                let request = try draft.prepareSend(modelChoice: choice)
                busy = false; transmit(request)
            } catch { busy = false; self.error = error.localizedDescription }
        }
    }
    private func queueAction(_ action: String, _ itemID: String, _ text: String?) async -> Bool {
        guard let chat, !busy, pending == nil, observation.isCurrent(at: Date()), scenePhase == .active else { return false }
        var args: [String: Any] = ["chat_id": chat.id, "action": action, "item_id": itemID]
        if let text { args["text"] = text }
        if action == "promote" { args["expected_run_id"] = snapshot?.running_request_id as Any? ?? NSNull() }
        busy = true
        defer { busy = false }
        do {
            let queue: RemoteOutbox = try await RemoteService.rpc(device, op: "outbox", args: args)
            mutationRevision += 1; snapshot?.outbox = queue; error = nil
            await refresh()
            return true
        } catch {
            self.error = error.localizedDescription
            await refresh()
            return false
        }
    }
    private func transmit(_ request: RemotePendingSend) {
        guard !busy else { return }; busy = true
        Task {
            defer { busy = false }
            do {
                let result: RemoteSent = try await RemoteService.rpc(device, op: "send", args: request.arguments(for: device), id: request.id)
                mutationRevision += 1
                // Keep the just-accepted bubble visible until the next snapshot arrives.
                // A slow poll from before this receipt cannot overwrite the new item.
                if request.deliveryMode == .queue, result.delivery == "queued", snapshot?.outbox != nil,
                   snapshot?.outbox?.items.contains(where: { $0.id == request.id }) != true {
                    snapshot?.outbox?.items.append(.init(id: request.id, text: request.text, state: "pending", attachments: 0))
                }
                try draft.acknowledge(request, chatID: result.chat.id)
                chat = result.chat; error = nil; receiptNotice = result.recoveryNotice; await refresh()
            } catch {
                self.error = error.localizedDescription
                if let failure = error as? RemoteFailure, [400, 403, 404, 412, 422].contains(failure.status) {
                    do { try draft.reject(request) } catch { self.error = error.localizedDescription }
                }
            }
        }
    }
    private func stop() {
        guard let target = stopTarget, target.chatID == chat?.id, !busy,
              observation.isCurrent(at: Date()), scenePhase == .active else { return }
        stopTarget = nil; busy = true; error = nil
        Task {
            defer { busy = false }
            do {
                let _: RemoteAck = try await RemoteService.rpc(device, op: "stop", args: target.arguments)
                await refresh()
            } catch { self.error = error.localizedDescription; await refresh() }
        }
    }
    private func presentNextApproval() {
        guard confirmed, !busy, !approvalBusy, presentedApproval == nil,
              !showModels, !showLegacyRecovery, !showOtherDrafts, !showUnconfirmedRecords,
              reviewedPending == nil, !stopConfirmation, !dictation.active,
              let approval = snapshot?.approvals.first(where: { !seenApprovals.contains($0.id) && !resolvedApprovals.contains($0.id) }) else { return }
        promptFocused = false; approvalError = nil
        seenApprovals.insert(approval.id); presentedApproval = approval
    }

    private func respondToApproval(_ approval: RemoteApproval, scope: String) {
        guard let chat, !busy, !approvalBusy, observation.isCurrent(at: Date()), scenePhase == .active,
              snapshot?.approvals.contains(where: { $0.id == approval.id }) == true,
              !resolvedApprovals.contains(approval.id) else { return }
        approvalBusy = true; busy = true; approvalError = nil
        Task {
            defer { approvalBusy = false; busy = false }
            do {
                let _: RemoteAck = try await RemoteService.rpc(device, op: "approval", args: ["chat_id": chat.id, "request_id": approval.id, "allow": scope != "deny", "scope": scope == "deny" ? "exact" : scope])
                resolvedApprovals.insert(approval.id); presentedApproval = nil
                await refresh()
            } catch {
                approvalError = error.localizedDescription
                await refresh()
            }
        }
    }

    private func act(_ op: String, _ arguments: [String: Any]) {
        guard let chat, !busy, observation.isCurrent(at: Date()), scenePhase == .active else {return}; busy = true; error = nil; followBottom = true
        var args = arguments; args["chat_id"] = chat.id
        Task { defer {busy = false}; do { let _: RemoteAck = try await RemoteService.rpc(device, op: op, args: args); await refresh() } catch { self.error = error.localizedDescription } }
    }
}

private struct RemoteUnconfirmedDetails: View {
    let request: RemotePendingSend
    let deviceName: String
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(deviceName).font(.headline)
            Text(request.chatID.map { L10n.tr("原会话编号：\($0)") } ?? L10n.tr("原目标：新任务")).font(.footnote).foregroundStyle(.secondary)
            if let project = request.projectPath { Text(project).font(.footnote).textSelection(.enabled) }
            Text(request.text).textSelection(.enabled).accessibilityIdentifier("remote-unconfirmed-text").accessibilityValue(request.id)
            if let choice = request.modelChoice { Text("\(choice.model) · \(remoteEffortName(choice.reasoning_effort))").font(.footnote) }
        }.padding(.vertical, 8)
    }
}

private struct RemotePendingReview: View {
    let request: RemotePendingSend
    let deviceName: String
    let archive: () throws -> Void
    @State private var understood = false
    @State private var issue: String?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            Form {
                Section { RemoteUnconfirmedDetails(request: request, deviceName: deviceName) }
                Section {
                    Text(L10n.tr("这条指令可能已执行，请先在电脑上确认。"))
                    Toggle(L10n.tr("我已了解这条指令可能已执行"), isOn: $understood).accessibilityIdentifier("remote-understand-unconfirmed")
                    Button(L10n.tr("保留记录并结束等待")) { do { try archive() } catch { issue = error.localizedDescription } }
                        .disabled(!understood).accessibilityIdentifier("remote-archive-unconfirmed")
                    if let issue { Text(issue).foregroundStyle(.red) }
                }
            }.navigationTitle(L10n.tr("核对待确认指令")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { dismiss() } } }
        }
    }
}

private struct RemoteQuestionCard: View {
    let question: RemoteQuestion; let busy: Bool; let answer: ([String: Any]) -> Void
    @State private var selected: Set<String> = []; @State private var text = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(question.title).font(.headline)
            ForEach(question.options) { option in Button { if selected.contains(option.id) {selected.remove(option.id)} else if question.multiple == true {selected.insert(option.id)} else {selected = [option.id]} } label: { Label(option.label, systemImage: selected.contains(option.id) ? "checkmark.circle.fill" : "circle").frame(minHeight: 44) }.tint(Palette.ink) }
            TextField(L10n.tr("补充说明"), text: $text, axis: .vertical).textFieldStyle(.roundedBorder)
            HStack { Button(L10n.tr("跳过")) { submit(skip: true) }; Spacer(); Button(L10n.tr("提交回答")) { submit(skip: false) }.disabled(selected.isEmpty && text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) }
        }.padding(16).background(Palette.muted, in: RoundedRectangle(cornerRadius: 18)).disabled(busy)
    }
    private func submit(skip: Bool) { answer(["request_id":question.id,"selected":Array(selected).sorted(),"text":text,"skip":skip]) }
}

private struct RemoteApprovalSheet: View {
    let approval: RemoteApproval
    let confirmed: Bool
    let busy: Bool
    let issue: String?
    let supportsScopes: Bool
    let available: Bool
    let dismiss: () -> Void
    let respond: (String) -> Void
    @State private var scope = "exact"
    @State private var detent: PresentationDetent = .height(360)
    @State private var detailsExpanded = false
    @Environment(\.dynamicTypeSize) private var typeSize

    private var directory: String? { approval.supportsDirectoryGrant ? approval.suggested_directory : nil }
    private func directoryText(_ path: String) -> Text {
        guard let slash = path.lastIndex(of: "/"), slash < path.index(before: path.endIndex) else { return Text(path).bold() }
        let start = path.index(after: slash)
        return Text(String(path[..<start])).foregroundColor(Palette.secondary) + Text(String(path[start...])).bold()
    }
    private var scopeChoices: some View {
        let choices = [("exact", L10n.tr("仅本次")), ("session_directory", L10n.tr("本会话")), ("persistent_directory", L10n.tr("始终允许"))]
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 0) {
                ForEach(choices, id: \.0) { value, label in scopeButton(value, label: label) }
            }
            VStack(spacing: 4) {
                ForEach(choices, id: \.0) { value, label in scopeButton(value, label: label) }
            }
        }.padding(3).background(Palette.muted, in: RoundedRectangle(cornerRadius: 16))
            .disabled(busy || !confirmed || !available)
    }
    private func scopeButton(_ value: String, label: String) -> some View {
        Button { scope = value } label: {
            Text(label).font(.subheadline).fixedSize(horizontal: true, vertical: false)
                .frame(maxWidth: .infinity, minHeight: 44)
                .foregroundStyle(scope == value ? Palette.onInk : Palette.ink)
                .background(scope == value ? Palette.ink : Color.clear, in: RoundedRectangle(cornerRadius: 13))
        }.buttonStyle(.plain).accessibilityIdentifier("remote-approval-scope-" + value)
            .accessibilityAddTraits(scope == value ? .isSelected : [])
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 7) {
                    Text(directory == nil ? L10n.tr("需要你的批准") : L10n.tr("批准目录访问"))
                        .font(.title3.weight(.semibold)).accessibilityIdentifier("remote-approval-title")
                }
                Spacer(minLength: 0)
                Button(action: dismiss) {
                    Image(systemName: "xmark").font(.system(size: 20, weight: .regular)).frame(width: 44, height: 44)
                }.buttonStyle(.plain).accessibilityLabel(L10n.tr("稍后处理")).disabled(busy)
            }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 24).padding(.top, 16).padding(.bottom, 8)
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if let directory {
                        HStack(alignment: .top, spacing: 14) {
                            Image(systemName: "folder").font(.system(size: 26, weight: .light)).foregroundStyle(Palette.secondary)
                            directoryText(directory).font(.footnote.monospaced()).textSelection(.enabled)
                                .fixedSize(horizontal: false, vertical: true)
                        }.padding(.top, 4)
                        Text(approval.directory_recursive == false ? L10n.tr("只读 · 不含子目录") : L10n.tr("只读 · 含子目录"))
                            .font(.subheadline).foregroundStyle(Palette.secondary)
                    } else {
                        Text(approval.findings_summary ?? approval.tool_name ?? L10n.tr("电脑操作")).font(.headline)
                        if let command = approval.command { Text(command).font(.callout.monospaced()).textSelection(.enabled) }
                        if let directory = approval.workingDirectory { Label(directory, systemImage: "folder").font(.footnote).textSelection(.enabled) }
                        Text(approval.reviewExplanation).font(.subheadline).foregroundStyle(Palette.secondary)
                    }
                    if approval.unsandboxed { Label(L10n.tr("这一次操作将在系统沙箱外执行"), systemImage: "exclamationmark.shield").font(.footnote) }
                    if supportsScopes && approval.supportsDirectoryGrant {
                        scopeChoices
                        if scope == "persistent_directory" {
                            Text(L10n.tr("后续会话生效，可在电脑权限设置中撤销。"))
                                .font(.footnote).foregroundStyle(Palette.secondary).accessibilityIdentifier("remote-approval-scope-note")
                        }
                    }
                    Divider().overlay(Palette.line)
                    DisclosureGroup(L10n.tr("操作详情"), isExpanded: $detailsExpanded) {
                        VStack(alignment: .leading, spacing: 12) {
                            if directory != nil {
                                Text(L10n.tr("允许读取、列举和搜索此目录") + (approval.directory_recursive == false ? "。" : L10n.tr("及其子目录。") )).font(.subheadline)
                            }
                            Text(approval.reviewExplanation).font(.subheadline)
                            if let reason = approval.justification, !reason.isEmpty { Text(reason).font(.subheadline) }
                            if let summary = approval.findings_summary { Text(summary).font(.subheadline) }
                            if let target = approval.exact_target { Text(target).textSelection(.enabled) }
                            if let details = approval.action_detail { Text(details).font(.footnote.monospaced()).textSelection(.enabled) }
                            if let failure = approval.review_failure { Text(failure).textSelection(.enabled) }
                        }.frame(maxWidth: .infinity, alignment: .leading).padding(.top, 12)
                    }.font(.subheadline).tint(Palette.ink).frame(minHeight: 44)
                }.padding(.horizontal, 24).padding(.bottom, 12)
            }
            VStack(alignment: .leading, spacing: 8) {
                if !available { Text(L10n.tr("这项审批已处理或已过期。")).foregroundStyle(Palette.secondary) }
                else if !confirmed { Label(L10n.tr("连接尚未确认，恢复后可继续审批。"), systemImage: "wifi.exclamationmark").foregroundStyle(Palette.secondary) }
                if let issue { Text(issue).foregroundStyle(.red).accessibilityIdentifier("remote-approval-error") }
                if busy { ProgressView(L10n.tr("正在提交…")) }
                HStack(spacing: 20) {
                    Button(L10n.tr("拒绝"), role: .destructive) { respond("deny") }
                        .foregroundStyle(.red).frame(minWidth: 48, minHeight: 48).accessibilityIdentifier("remote-approval-deny")
                    Button { respond(scope) } label: {
                        Text(L10n.tr("允许"))
                            .font(.body.weight(.medium)).multilineTextAlignment(.center)
                            .frame(maxWidth: .infinity, minHeight: 48).padding(.horizontal, 8)
                            .foregroundStyle(Palette.onAccent).background(Palette.accent, in: Capsule())
                    }.buttonStyle(.plain).accessibilityIdentifier("remote-approval-allow")
                }.disabled(busy || !confirmed || !available)
            }.font(.footnote).dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 24).padding(.top, 16).padding(.bottom, 12)
                .background(Palette.canvas).overlay(alignment: .top) { Rectangle().fill(Palette.line).frame(height: 0.5) }
        }.foregroundStyle(Palette.ink).background(Palette.canvas)
            .presentationDetents(typeSize.isAccessibilitySize ? [.large] : [.height(360), .large], selection: $detent)
            .presentationDragIndicator(.visible).interactiveDismissDisabled(busy)
            .onChange(of: detailsExpanded) { _, expanded in if expanded { detent = .large } }
            .onAppear { if typeSize.isAccessibilitySize { detent = .large } }
    }
}
