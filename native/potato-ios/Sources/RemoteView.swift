import SwiftUI

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
                Button(action: openSidebar) { Image(systemName: "line.3.horizontal").font(.title3).frame(width: 44, height: 44).background(.white.opacity(0.9), in: Circle()) }.accessibilityLabel("打开侧栏").accessibilityIdentifier("remote-sidebar")
                Spacer()
                Text("远程").font(.headline).accessibilityAddTraits(.isHeader)
                Spacer()
                Menu {
                    if store.profile == nil { Button("登录 Cloudflare 账号", systemImage: "person.crop.circle") { showLogin = true } }
                    Button("通过配对码添加电脑", systemImage: "plus") { showPairing = true }
                    Button("管理电脑", systemImage: "desktopcomputer") { showDevices = true }
                    Button("刷新", systemImage: "arrow.clockwise") { Task { await store.refresh() } }
                } label: { Image(systemName: "ellipsis").font(.title3).frame(width: 44, height: 44).background(.white.opacity(0.9), in: Circle()) }.accessibilityLabel("远程选项").accessibilityIdentifier("remote-options")
            }.padding(.horizontal, 18).padding(.vertical, 8)
            if !store.devices.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        deviceChip("全部", id: nil, online: nil)
                        ForEach(store.devices) { device in deviceChip(device.name, id: device.id, online: store.online[device.id] == true) }
                    }.padding(.horizontal, 20).padding(.vertical, 6)
                }.excludesSidebarGesture().accessibilityIdentifier("remote-device-picker")
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if let error = store.error { Label(error, systemImage: "wifi.exclamationmark").font(.footnote).foregroundStyle(Palette.secondary).padding(.vertical, 14) }
                    if store.devices.isEmpty {
                        ContentUnavailableView {
                            Label("让电脑上的任务，随你继续", systemImage: "desktopcomputer")
                        } description: { Text(store.profile == nil ? "电脑和手机登录同一个 Cloudflare 账号，已开启远程访问的电脑会显示在这里。" : "在电脑 Potato 登录同一账号，再开启远程访问。") }
                        actions: {
                            if store.profile == nil { Button { showLogin = true } label: { Text("登录 Cloudflare 账号").font(.headline).foregroundStyle(.white).padding(.horizontal, 22).frame(minHeight: 48).background(Palette.ink, in: Capsule()) }.buttonStyle(.plain).accessibilityIdentifier("remote-sign-in") }
                            Button("通过配对码添加") { showPairing = true }.font(.subheadline).padding(.top, 8).accessibilityIdentifier("remote-pair-empty")
                        }
                        .padding(.top, 70)
                    } else {
                        ForEach(visibleDevices.filter { store.online[$0.id] != true }) { device in
                            VStack(alignment: .leading, spacing: 6) {
                                Label("\(device.name) · 离线", systemImage: "desktopcomputer").font(.subheadline.weight(.medium))
                                Text("保持电脑开机、联网并运行 Potato，连接恢复后即可继续任务。").font(.footnote).foregroundStyle(Palette.secondary)
                            }.padding(.vertical, 14)
                        }
                        if visibleDevices.contains(where: { chats($0).contains { $0.pinned == true } }) {
                            sectionTitle("置顶")
                            ForEach(visibleDevices) { device in ForEach(chats(device).filter { $0.pinned == true }) { chat in chatRow(chat, device: device) } }
                        }
                        if visibleDevices.contains(where: { !projects($0).isEmpty }) {
                            sectionTitle("项目")
                            ForEach(visibleDevices) { device in
                                ForEach(projects(device)) { project in
                                    HStack(spacing: 10) {
                                        NavigationLink { RemoteProjectView(device: device, project: project, chats: chats(device).filter { $0.project_path == project.path }, settings: settings) } label: {
                                            HStack(spacing: 14) { Image(systemName: "folder").font(.title3); rowTitle(project.name, device: device); Spacer(minLength: 0) }.frame(minHeight: 48).contentShape(Rectangle())
                                        }.buttonStyle(.plain).accessibilityIdentifier("remote-project-\(device.id)-\(project.path)")
                                        Button { newTask = RemoteDestination(device: device, project: project) } label: { Image(systemName: "square.and.pencil").font(.title3).foregroundStyle(Palette.secondary).frame(width: 44, height: 48) }.accessibilityLabel("在\(project.name)新建任务")
                                    }.disabled(store.online[device.id] != true)
                                }
                            }
                        }
                        if visibleDevices.contains(where: { !chats($0).filter { $0.pinned != true }.isEmpty }) {
                            sectionTitle("会话")
                            ForEach(visibleDevices) { device in ForEach(chats(device).filter { $0.pinned != true }) { chat in chatRow(chat, device: device) } }
                        }
                        if !hasResults && !connectedDevices.isEmpty {
                            if store.refreshing { ProgressView("正在读取电脑…").padding(.top, 60).frame(maxWidth: .infinity) }
                            else { ContentUnavailableView(search.isEmpty ? "开始第一个远程任务" : "没有找到相关内容", systemImage: search.isEmpty ? "square.and.pencil" : "magnifyingglass", description: Text(search.isEmpty ? "点击右下角，把任务交给电脑。" : "试试其他项目或会话名称。")).padding(.top, 50) }
                        }
                    }
                }.padding(.horizontal, 24).padding(.bottom, 24)
            }.refreshable { await store.refresh() }.scrollDismissesKeyboard(.interactively)
            HStack(spacing: 10) {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").font(.title3)
                    TextField("搜索会话", text: $search).submitLabel(.search).accessibilityIdentifier("remote-search")
                    if !search.isEmpty { Button { search = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Palette.secondary) }.accessibilityLabel("清除搜索") }
                }.padding(.horizontal, 16).frame(minHeight: 48).background(.white.opacity(0.95), in: Capsule())
                composeButton(voice: true)
                composeButton(voice: false)
            }.padding(.horizontal, 22).padding(.top, 8).padding(.bottom, 12)
                .excludesSidebarGesture()
        }.foregroundStyle(Palette.ink).background(Palette.canvas.ignoresSafeArea()).toolbar(.hidden, for: .navigationBar)
            .navigationDestination(item: $newTask) { destination in RemoteTaskView(device: destination.device, project: destination.project, settings: settings, startWithVoice: destination.voice) }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                while !Task.isCancelled { await store.refresh(); try? await Task.sleep(for: .seconds(5)) }
            }
            .sheet(isPresented: $showLogin) { loginSheet }
            .onChange(of: store.profile?.owner) { _, value in if value != nil { showLogin = false } }
            .sheet(isPresented: $showDevices) { deviceManagement }
            .sheet(isPresented: $showPairing) { pairingSheet }
    }
    private func sectionTitle(_ name: String) -> some View { Text(name).font(.headline).padding(.top, 26).padding(.bottom, 12).accessibilityAddTraits(.isHeader) }
    private func rowTitle(_ title: String, device: RemoteDevice) -> some View {
        VStack(alignment: .leading, spacing: 3) { Text(title).font(.body).lineLimit(1); if multiple { Text(device.name).font(.caption2).foregroundStyle(Palette.secondary).lineLimit(1) } }
    }
    private func deviceChip(_ name: String, id: String?, online: Bool?) -> some View {
        Button { selectedDevice = id } label: {
            HStack(spacing: 7) { if let online { Circle().fill(online ? Color(red: 0.12, green: 0.71, blue: 0.5) : Color.gray).frame(width: 7, height: 7); Image(systemName: "laptopcomputer") }; Text(name).lineLimit(1) }
                .font(.subheadline).padding(.horizontal, 15).frame(minHeight: 38)
                .foregroundStyle(selectedDevice == id ? Color.white : Palette.ink)
                .background(selectedDevice == id ? Palette.ink : Palette.muted, in: Capsule()).padding(.vertical, 3)
        }.buttonStyle(.plain).accessibilityAddTraits(selectedDevice == id ? [.isSelected] : [])
            .accessibilityIdentifier("remote-device-\(id ?? "all")")
    }
    private func chatRow(_ chat: RemoteChat, device: RemoteDevice) -> some View {
        NavigationLink { RemoteTaskView(device: device, chat: chat, settings: settings) } label: {
            HStack { rowTitle(chat.name, device: device); Spacer(minLength: 8); if chat.status == "running" { Text(store.online[device.id] == true ? "执行中" : "上次执行中").font(.caption).foregroundStyle(Palette.secondary) } }.frame(minHeight: 52).contentShape(Rectangle())
        }.buttonStyle(.plain).disabled(store.online[device.id] != true)
    }
    @ViewBuilder private func composeButton(voice: Bool) -> some View {
        if connectedDevices.count > 1 {
            Menu { ForEach(connectedDevices) { device in Button(device.name) { newTask = RemoteDestination(device: device, voice: voice) } } } label: { composeIcon(voice: voice) }.accessibilityLabel(voice ? "选择电脑并语音输入" : "选择电脑并新建任务")
        } else {
            Button { if let device = connectedDevices.first { newTask = RemoteDestination(device: device, voice: voice) } else if store.devices.isEmpty { showLogin = store.profile == nil } } label: { composeIcon(voice: voice) }
                .disabled(!store.devices.isEmpty && connectedDevices.isEmpty)
                .accessibilityLabel(voice ? "语音指令" : "新远程任务").accessibilityIdentifier(voice ? "remote-voice" : "remote-new-task")
        }
    }
    private func composeIcon(voice: Bool) -> some View { Image(systemName: voice ? "waveform" : "square.and.pencil").font(.system(size: 21)).foregroundStyle(.white).frame(width: 46, height: 46).background(Palette.ink, in: Circle()) }
    private var deviceManagement: some View {
        NavigationStack {
            List {
                if let profile = store.profile {
                    Section("Cloudflare 账号") {
                        Label(profile.email, systemImage: "person.crop.circle")
                        Button("退出这台 iPhone 的登录", role: .destructive) { Task { await store.logout() } }
                    }
                }
                ForEach(store.devices) { device in Section(device.name) {
                    Label(store.online[device.id] == true ? "在线" : "离线", systemImage: "desktopcomputer")
                    if device.owner != nil {
                        Text("撤销后，所有手机都无法再控制这台电脑。电脑需要重新登录才能再次关联。").font(.footnote).foregroundStyle(Palette.secondary)
                        Button("撤销此电脑的远程访问", role: .destructive) { Task { await store.revoke(device); if selectedDevice == device.id { selectedDevice = nil } } }
                    } else {
                        Text("移除只清除本机配对。若要撤销旧手机访问，请在电脑重新生成配对码。").font(.footnote).foregroundStyle(Palette.secondary)
                        Button("从这台 iPhone 移除", role: .destructive) { store.forget(device); if selectedDevice == device.id { selectedDevice = nil } }
                    }
                } }
                Button("添加电脑") { showDevices = false; showPairing = true }
            }.navigationTitle("管理电脑").navigationBarTitleDisplayMode(.inline).toolbar { ToolbarItem(placement: .confirmationAction) { Button("完成") { showDevices = false } } }
        }
    }
    private var loginSheet: some View {
        NavigationStack {
            Form {
                Section {
                    Label("使用 Cloudflare 账号关联", systemImage: "person.crop.circle").font(.headline)
                    Text("手机和电脑登录同一账号后，可以查看已关联的电脑并继续任务。").font(.subheadline).foregroundStyle(Palette.secondary)
                    TextField("服务地址", text: $loginRelay).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                }
                if let login = store.login {
                    Section("核对登录验证码") {
                        Text(login.code).font(.title.monospaced()).textSelection(.enabled)
                        Text("在浏览器中登录，并确认两处显示的验证码一致，然后返回 Potato。").font(.subheadline)
                        Button("打开登录页面") { UIApplication.shared.open(login.verification_url) }
                        Button("我已登录，刷新") { Task { await store.refresh() } }
                        Button("重新开始") { store.cancelLogin() }
                    }
                } else {
                    Button { Task { if let url = await store.beginLogin(relay: loginRelay) { await UIApplication.shared.open(url) } } } label: {
                        HStack { if store.signingIn { ProgressView() }; Text("继续使用 Cloudflare") }
                    }.disabled(store.signingIn)
                }
                if let error = store.error { Text(error).font(.footnote).foregroundStyle(.red) }
            }.navigationTitle("登录 Potato").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { store.cancelLogin(); showLogin = false }.disabled(store.signingIn) } }
        }
    }
    private var pairingSheet: some View {
        NavigationStack {
            Form {
                Section("连接你的电脑") {
                    Label("电脑 Potato → 设置 → 能力 → iPhone 远程控制", systemImage: "desktopcomputer")
                    Text("生成并复制一次性配对码，在这里粘贴。配对后可以查看该电脑的会话、执行任务和处理审批。").font(.subheadline).foregroundStyle(Palette.secondary)
                    SecureField("粘贴完整配对码", text: $pairing).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("remote-pairing-code")
                    Button { pairingBusy = true; store.error = nil; Task { await store.pair(pairing); pairingBusy = false; if store.error == nil { pairing = ""; showPairing = false } } } label: { HStack { if pairingBusy { ProgressView() }; Text("连接电脑") } }.disabled(pairing.isEmpty || pairingBusy).accessibilityIdentifier("remote-pair-submit")
                }
                if let error = store.error { Section { Text(error).foregroundStyle(.red) } }
            }.navigationTitle("添加电脑").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { showPairing = false }.disabled(pairingBusy) } }
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
                NavigationLink { RemoteTaskView(device: device, project: project, settings: settings) } label: { Label("在此项目新建任务", systemImage: "square.and.pencil").frame(minHeight: 48) }
                Text("项目会话").font(.headline).padding(.top, 12)
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
    @StateObject private var dictation = RemoteDictation()
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
    @State private var stopConfirmation = false
    @State private var stopTarget: RemoteStopRequest?
    @State private var receiptNotice: String?
    @State private var reviewedPending: RemotePendingSend?
    @State private var showUnconfirmedRecords = false
    @State private var followBottom = true
    @FocusState private var promptFocused: Bool
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
    private var running: Bool { snapshot?.status == "running" }
    private var canAct: Bool { chat == nil || confirmed }
    private var canSend: Bool { !busy && canAct && pending == nil && draft.storageError == nil && !dictation.active && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    private var activityVisible: Bool { confirmed && running && snapshot?.needsUserResponse != true }

    private var modelLabel: String {
        if running { return "沿用当前任务配置" }
        guard let choice = draft.modelChoice else { return "跟随电脑" }
        let name = modelCatalog?.models.first(where: { $0.matches(choice) })?.name ?? choice.model
        return "\(name) · \(remoteEffortName(choice.reasoning_effort))"
    }
    private var scrollRevision: String {
        guard let snapshot else { return "empty" }
        let messages = snapshot.displayMessages
        return "\(messages.count)|\(messages.last?.text ?? "")|\(snapshot.approvals.map(\.id))|\(snapshot.questions.filter { $0.status == "pending" }.map(\.id))|\(snapshot.status)|\(snapshot.outcome?.status ?? "")"
    }
    var body: some View {
        VStack(spacing: 0) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: 4) {
                        Label { Text(device.name) } icon: { Image(systemName: "desktopcomputer").font(.system(size: 18)) }
                        Text("远程任务")
                    }.frame(maxWidth: .infinity, alignment: .leading).fixedSize(horizontal: false, vertical: true)
                } else {
                    HStack { Label(device.name, systemImage: "desktopcomputer"); Spacer(); Text("远程任务") }
                }
            }.font(.caption).foregroundStyle(Palette.secondary).dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 20).padding(.vertical, 10)
            ScrollViewReader { proxy in
            ScrollView {
                // Snapshots are bounded to 120 messages. Eager layout avoids
                // lazy height estimation oscillating with keyboard resizing and scrollTo.
                VStack(alignment: .leading, spacing: 20) {
                    if let error { Label(error, systemImage: "exclamationmark.circle").font(.subheadline).foregroundStyle(.red); Button("刷新任务状态") { Task { await refresh() } } }
                    if let receiptNotice { Text(receiptNotice).font(.subheadline).foregroundStyle(Palette.secondary).accessibilityIdentifier("remote-recovered-send") }
                    if let failure = observation.failure { Text(failure).font(.footnote).foregroundStyle(Palette.secondary).accessibilityIdentifier("remote-connection-error") }
                    if let snapshot {
                        ForEach(RemoteConversationRow.make(snapshot.displayMessages)) { row in
                            RemoteConversationRowView(row: row, running: running, confirmed: confirmed, activeProcessID: snapshot.activeProcessID, onExpand: { followBottom = false })
                        }
                        ForEach(snapshot.approvals) { approval in
                            VStack(alignment: .leading, spacing: 12) {
                                Label("需要你的批准", systemImage: "hand.raised").font(.headline)
                                Text(approval.findings_summary ?? approval.tool_name ?? "电脑操作").font(.subheadline)
                                if let justification = approval.justification, !justification.isEmpty { Text(justification).font(.subheadline).foregroundStyle(Palette.secondary) }
                                if let command = approval.command { Text(command).font(.body.monospaced()).textSelection(.enabled) }
                                if let directory = approval.workingDirectory { Label(directory, systemImage: "folder").font(.caption).lineLimit(3).textSelection(.enabled) }
                                if approval.unsandboxed { Label("这一次操作将在系统沙箱外执行", systemImage: "exclamationmark.shield").font(.footnote) }
                                DisclosureGroup("查看完整操作详情") {
                                    if let target = approval.exact_target { Text(target).font(.caption).textSelection(.enabled) }
                                    if let details = approval.action_detail { Text(details).font(.caption.monospaced()).textSelection(.enabled) }
                                }.font(.footnote).foregroundStyle(Palette.secondary)
                                HStack { Button("拒绝", role: .destructive) { act("approval", ["request_id": approval.id, "allow": false]) }; Spacer(); Button("允许这一次") { act("approval", ["request_id": approval.id, "allow": true]) }.buttonStyle(.borderedProminent).tint(Palette.ink) }
                            }.padding(16).background(Palette.muted, in: RoundedRectangle(cornerRadius: 18)).disabled(busy || !confirmed)
                        }
                        ForEach(snapshot.questions.filter { $0.status == "pending" }) { question in RemoteQuestionCard(question: question, busy: busy || !confirmed) { args in act("answer", args) } }
                        if !running && snapshot.outcome?.status == "failed" { Label(snapshot.outcome?.error?.message ?? "本轮任务失败，请检查后重试。", systemImage: "exclamationmark.circle").font(.subheadline).foregroundStyle(.red) }
                    } else if chat == nil { ContentUnavailableView("交给这台电脑", systemImage: "desktopcomputer", description: Text(project.map { "任务将在“\($0.name)”项目中执行。" } ?? "可以读取电脑上的项目、执行命令，并使用桌面已启用的能力。")) }
                    else { Text("连接后会显示电脑上的会话内容。").font(.footnote).foregroundStyle(Palette.secondary) }
                    Color.clear.frame(height: 1).id("remote-bottom")
                }.padding(20).background(ScrollActivityObserver { followBottom = false })
            }.accessibilityIdentifier("remote-conversation")
                .refreshable { await refresh() }
                .scrollDismissesKeyboard(.interactively)
                .onAppear { if chat != nil { proxy.scrollTo("remote-bottom", anchor: .bottom) } }
                .onChange(of: scrollRevision) { _, _ in if followBottom { proxy.scrollTo("remote-bottom", anchor: .bottom) } }
                .overlay(alignment: .bottomTrailing) {
                    if !followBottom {
                        IconButton(symbol: "arrow.down", label: "回到最新消息", id: "remote-scroll-latest") { followBottom = true; proxy.scrollTo("remote-bottom", anchor: .bottom) }.background(.regularMaterial, in: Circle()).padding(16)
                    }
                }
            }
            VStack(spacing: 8) {
                if chat != nil {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            if activityVisible {
                                if reduceMotion { Image(systemName: "ellipsis").accessibilityIdentifier("remote-static-activity") }
                                else { ProgressView().controlSize(.small).accessibilityIdentifier("remote-activity-spinner") }
                            } else if !confirmed { Image(systemName: observation.failure == nil ? "arrow.triangle.2.circlepath" : "wifi.exclamationmark").font(.system(size: 16)) }
                            Text(observation.title(snapshot: snapshot, at: observedNow)).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("remote-current-status")
                            Spacer(minLength: 0)
                            if !confirmed { Button("刷新") { Task { await refresh() } }.fixedSize().frame(minHeight: 44).disabled(loading).accessibilityIdentifier("remote-status-refresh") }
                        }
                        if !confirmed, let snapshot { Text("上次确认：\(snapshot.activityTitle)").fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("remote-last-status") }
                    }.font(.footnote).foregroundStyle(Palette.secondary).frame(maxWidth: .infinity, alignment: .leading).accessibilityElement(children: .contain)
                }
                if let issue = draft.storageError { Text(issue).font(.footnote).foregroundStyle(.red) }
                if let issue = draft.legacyError { Text(issue).font(.footnote).foregroundStyle(Palette.secondary) }
                if draft.legacy != nil { Button("恢复旧版草稿") { showLegacyRecovery = true }.frame(minHeight: 44).accessibilityIdentifier("remote-legacy-draft") }
                if !draft.otherDrafts.isEmpty { Button("查看另外保存的草稿") { showOtherDrafts = true }.frame(minHeight: 44).accessibilityIdentifier("remote-other-drafts") }
                if !draft.archived.isEmpty { Button("未确认指令记录（\(draft.archived.count)）") { showUnconfirmedRecords = true }.frame(minHeight: 44).accessibilityIdentifier("remote-unconfirmed-records") }
                if let pending, !busy {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("这条指令的发送结果尚未确认，重试会沿用同一操作编号。").font(.footnote).foregroundStyle(Palette.secondary)
                        Text(pending.text).font(.footnote).lineLimit(2)
                        if let choice = pending.modelChoice { Text("\(choice.model) · \(remoteEffortName(choice.reasoning_effort))").font(.caption).foregroundStyle(Palette.secondary) }
                        Button("重试确认发送结果") { transmit(pending) }.frame(minHeight: 44)
                        Button("核对并处理这条指令") { promptFocused = false; reviewedPending = pending }.frame(minHeight: 44).accessibilityIdentifier("remote-review-pending")
                    }.padding(12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 16))
                }
                if let notice = dictation.notice { Text(notice).font(.footnote).foregroundStyle(Palette.secondary) }
                if dictation.active {
                    HStack { Button("取消") { dictation.cancel(restore: true) }; Spacer(); Text(dictation.ready ? "正在听…" : "正在连接…").font(.subheadline); Spacer(); Button("完成") { dictation.finish() }.disabled(!dictation.ready) }.frame(minHeight: 44)
                }
                if running {
                    if let target = snapshot.flatMap(RemoteStopRequest.init(snapshot:)) {
                        Button("停止任务", role: .destructive) { stopTarget = target; stopConfirmation = true }.frame(minHeight: 44).disabled(busy || !confirmed).accessibilityIdentifier("remote-stop")
                    } else {
                        Text("请更新电脑端后使用远程停止；也可在电脑上停止任务。").font(.footnote).foregroundStyle(Palette.secondary).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("remote-stop-update-required")
                    }
                }
                HStack {
                    Button { promptFocused = false; showModels = true; Task { await reloadModels() } } label: {
                        HStack(spacing: 6) { Text(modelLabel).lineLimit(2).fixedSize(horizontal: false, vertical: true); if !running { Image(systemName: "chevron.down").font(.system(size: 11)) } }.font(.subheadline).frame(minHeight: 44)
                    }.accessibilityIdentifier("remote-model-settings").accessibilityLabel("模型与思考，\(modelLabel)").disabled(busy || pending != nil || running || !canAct)
                    Spacer(minLength: 0)
                }
                HStack(alignment: .bottom) {
                    TextField("给电脑发指令…", text: $draft.text, axis: .vertical).dynamicTypeSize(dynamicTypeSize).lineLimit(1...(dynamicTypeSize.isAccessibilitySize ? 3 : 6)).padding(12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 20)).accessibilityIdentifier("remote-prompt").focused($promptFocused).disabled(busy || dictation.active)
                    if !dictation.active { Button { startDictation() } label: { Image(systemName: "waveform").font(.system(size: 20)).frame(width: 44, height: 44) }.accessibilityLabel("语音输入").disabled(busy) }
                    Button { send() } label: { if busy { ProgressView().frame(width: 44, height: 44) } else { Image(systemName: "arrow.up").font(.system(size: 18, weight: .semibold)).foregroundStyle(.white).frame(width: 44, height: 44).background(Palette.ink, in: Circle()) } }.disabled(!canSend).opacity(canSend || busy ? 1 : 0.35).accessibilityLabel("发送到电脑").accessibilityIdentifier("remote-send")
                }
            }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 16).padding(.bottom, 12).padding(.top, 8).background(Color.white)
        }.background(Color.white).navigationTitle(chat?.name ?? "新远程任务").navigationBarTitleDisplayMode(.inline)
            .toolbar { if let chat { ToolbarItem(placement: .topBarTrailing) { Button { act("pin", ["pinned": !(chat.pinned ?? false)]) } label: { Image(systemName: chat.pinned == true ? "pin.fill" : "pin") }.accessibilityLabel(chat.pinned == true ? "取消置顶" : "置顶任务").disabled(busy || !confirmed) } } }
            .confirmationDialog("停止刚才查看的这一轮任务？", isPresented: $stopConfirmation, titleVisibility: .visible) { Button("停止任务", role: .destructive) { stop() } }
            .sheet(isPresented: $showModels) {
                RemoteModelPicker(deviceName: device.name, catalog: modelCatalog, issue: modelIssue, loading: modelsLoading, selection: draft.modelChoice, choose: { choice in
                    do { try draft.chooseModel(choice); modelIssue = nil } catch { modelIssue = error.localizedDescription }
                }, reload: { Task { await reloadModels() } })
            }
            .sheet(isPresented: $showLegacyRecovery) {
                NavigationStack {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            Text("旧版没有完整保存草稿的目标电脑。请先核对，这份内容是否属于“\(device.name)”。")
                            if let legacy = draft.legacy {
                                Text(legacy.text.isEmpty ? legacy.pending?.text ?? "" : legacy.text).textSelection(.enabled)
                                if let pending = legacy.pending {
                                    Text("待确认指令").font(.headline)
                                    Text(pending.text).textSelection(.enabled)
                                    Text("它可能已经执行。恢复会保留原操作编号，不会自动重发；请核对电脑后再确认发送结果。").font(.footnote).foregroundStyle(.secondary)
                                }
                            }
                            Button("确认属于这台电脑并恢复") {
                                do {
                                    try draft.restoreLegacy()
                                    if draft.address.kind == "chat", chat?.id != draft.address.value {
                                        chat = RemoteChat(id: draft.address.value, session_id: "", name: "恢复的远程任务", status: nil, pinned: nil, project_path: project?.path)
                                        Task { await refresh() }
                                    }
                                    showLegacyRecovery = false
                                } catch { self.error = error.localizedDescription; showLegacyRecovery = false }
                            }.frame(minHeight: 44).accessibilityIdentifier("remote-confirm-legacy-draft")
                        }.padding(20)
                    }.navigationTitle("核对旧版草稿").navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { showLegacyRecovery = false } } }
                }
            }
            .sheet(isPresented: $showOtherDrafts) {
                NavigationStack {
                    List(draft.otherDrafts, id: \.self) { value in
                        VStack(alignment: .leading, spacing: 12) {
                            Text(value).textSelection(.enabled)
                            Button("恢复到输入框") {
                                do { try draft.chooseOtherDraft(value); showOtherDrafts = false }
                                catch { self.error = error.localizedDescription; showOtherDrafts = false }
                            }.frame(minHeight: 44)
                        }
                    }.navigationTitle("另外保存的草稿").navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { showOtherDrafts = false } } }
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
                        Text("以下指令的结果仍未确认。结束等待没有取消电脑任务，也没有重新发送。请在电脑核对后再决定后续操作。").font(.subheadline)
                        ForEach(draft.archived, id: \.id) { request in
                            RemoteUnconfirmedDetails(request: request, deviceName: device.name)
                        }
                    }.navigationTitle("未确认指令记录").navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { showUnconfirmedRecords = false } } }
                }
            }
            .onAppear {
                guard !appeared else { return }; appeared = true
                draft.load()
                if startWithVoice && pending == nil && draft.storageError == nil { startDictation() }
            }
            .onDisappear { observation.suspend(); dictation.cancel(restore: false); draft.flush() }
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { observation.suspend(); dictation.cancel(restore: false); draft.flush() }
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
                    if observation.isCurrent(at: observedNow) != observation.isCurrent(at: now) { observedNow = now }
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
        guard let chat, !loading, scenePhase == .active else { return }; loading = true; defer { loading = false }; let requestedAt = Date()
        do { let value: RemoteSnapshot = try await RemoteService.rpc(device, op: "chat", args: ["chat_id": chat.id]); guard !Task.isCancelled, scenePhase == .active else {return}; snapshot = value; self.chat = value.chat; observation.received(requestedAt: requestedAt); observedNow = Date() }
        catch { if !Task.isCancelled { observation.failed(ChatService.failureDescription(error)); observedNow = Date() } }
    }
    private func startDictation() {
        dictation.start(settings: settings, original: text) { value in draft.text = value }
    }
    private func send() {
        guard !busy, pending == nil, chat == nil || observation.isCurrent(at: Date()) else { return }
        promptFocused = false
        followBottom = true
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
                    guard draft.modelChoice == nil else { throw LocalFailure.message("请更新电脑端，以使用所选模型与思考设置。") }
                    choice = nil
                }
                let request = try draft.prepareSend(modelChoice: choice)
                busy = false; transmit(request)
            } catch { busy = false; self.error = error.localizedDescription }
        }
    }
    private func transmit(_ request: RemotePendingSend) {
        guard !busy else { return }; busy = true
        Task {
            defer { busy = false }
            do {
                let result: RemoteSent = try await RemoteService.rpc(device, op: "send", args: request.arguments(for: device), id: request.id)
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
            Text(request.chatID.map { "原会话编号：\($0)" } ?? "原目标：新任务").font(.footnote).foregroundStyle(.secondary)
            if let project = request.projectPath { Text(project).font(.footnote).textSelection(.enabled) }
            Text(request.text).textSelection(.enabled).accessibilityIdentifier("remote-unconfirmed-text")
            if let choice = request.modelChoice { Text("\(choice.model) · \(remoteEffortName(choice.reasoning_effort))").font(.footnote) }
            Text("原操作编号：\(request.id)").font(.footnote).textSelection(.enabled).accessibilityIdentifier("remote-unconfirmed-id")
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
                    Text("这条指令可能已经执行，请先在电脑检查原任务。结束等待只会把原编号和内容保存在本机记录中，不会停止电脑任务，也不会再次发送。")
                    Toggle("我已了解这条指令可能已执行", isOn: $understood).accessibilityIdentifier("remote-understand-unconfirmed")
                    Button("保留记录并结束等待") { do { try archive() } catch { issue = error.localizedDescription } }
                        .disabled(!understood).accessibilityIdentifier("remote-archive-unconfirmed")
                    if let issue { Text(issue).foregroundStyle(.red) }
                }
            }.navigationTitle("核对待确认指令").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } } }
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
            TextField("补充说明", text: $text, axis: .vertical).textFieldStyle(.roundedBorder)
            HStack { Button("跳过") { submit(skip: true) }; Spacer(); Button("提交回答") { submit(skip: false) }.disabled(selected.isEmpty && text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) }
        }.padding(16).background(Palette.muted, in: RoundedRectangle(cornerRadius: 18)).disabled(busy)
    }
    private func submit(skip: Bool) { answer(["request_id":question.id,"selected":Array(selected).sorted(),"text":text,"skip":skip]) }
}

/// Remote dictation shares the real speech transport, but never dispatches an
/// instruction automatically: the recognized text stays in the remote draft.
@MainActor final class RemoteDictation: ObservableObject {
    @Published private(set) var active = false
    @Published private(set) var ready = false
    @Published private(set) var notice: String?
    private let recorder: VoiceCapture
    private var generation = UUID()
    private var task: Task<Void, Never>?
    private var original = ""
    private var update: ((String) -> Void)?
    init(recorder: VoiceCapture? = nil) { self.recorder = recorder ?? VoiceRecorder() }
    func start(settings: ConnectionSettings, original: String, update: @escaping (String) -> Void) {
        guard !active else { return }
        self.original = original; self.update = update; active = true; ready = false; notice = nil
        let id = UUID(); generation = id
        task = Task { [weak self] in
            guard let self else { return }
            await recorder.start(settings: settings) { [weak self] event in
                guard let self, generation == id else { return }
                switch event {
                case .ready: ready = true
                case .text(let value, let final):
                    if !value.isEmpty { self.update?(original + (original.isEmpty ? "" : "\n") + value) }
                    if final { cancel(restore: false) }
                case .failed(let error): notice = error + "，已有文字已保留。"; cancel(restore: false)
                case .interrupted: notice = "录音已中断，已有文字已保留。"; cancel(restore: false)
                case .limit: finish()
                case .level: break
                }
            }
        }
    }
    func finish() { guard active, ready else { return }; ready = false; recorder.finish() }
    func cancel(restore: Bool) {
        if restore { update?(original) }
        generation = UUID(); recorder.cancel(); task?.cancel(); task = nil; active = false; ready = false; update = nil
    }
}
