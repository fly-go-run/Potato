import SwiftUI
import UniformTypeIdentifiers
import PhotosUI
import QuickLook
import AVFoundation

struct WorkspaceView: View {
    @StateObject private var store: WorkspaceStore
    @StateObject private var remoteStore = RemoteStore()
    @State private var sidebarVisible = false
    @State private var sidebarDrag: SidebarDrag?
    @StateObject private var sidebarGestureRegistry = SidebarGestureRegistry()
    @State private var remoteVisible = false
    @State private var libraryVisible = false
    @State private var remoteRootActive = true
    @State private var documentVisible = true
    @State private var expanded = false
    @State private var modal: Modal?
    @State private var appearancePreview: AppAppearance?
    @State private var showAttachments = false
    @State private var attachmentLibraryRequested = false
    @State private var attachmentDestination: AttachmentDestination?
    @State private var nextAttachmentDestination: AttachmentDestination?
    @StateObject private var attachmentImports = ComposerAttachments()
    private var importing: Bool { attachmentImports.count(in: store.selectedID) > 0 }
    @State private var editingMessage: ChatMessage?
    @State private var editedText = ""
    @State private var toast: String?
    @State private var followBottom = true
    /// Whether the newest content is below the visible area; stays true where it cannot be measured (iOS 17).
    @State private var latestOffscreen = true
    @StateObject private var speech = ReplySpeech()
    @StateObject private var voice = VoiceComposer()
    @State private var inputSelection: NSRange?
    @State private var inputFocused = false
    @State private var availableHeight: CGFloat = 600
    @State private var inputOverflowing = false
    @State private var inputExpanded = false
    @State private var restoreInputAfterExpansion = true
    @State private var expandAfterVoice = false
    @State private var renaming = false
    @State private var renameText = ""
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    private var reduceMotion: Bool {
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--reduce-motion-preview") { return true }
        #endif
        return systemReduceMotion
    }
    @Environment(\.scenePhase) private var scenePhase
    private enum Modal: Identifiable {
        case history, settings, models, retryModels(ChatMessage), signIn, share(URL), preview([Attachment], Int)
        var id: String { switch self { case .history: "history"; case .settings: "settings"; case .models: "models"; case .retryModels(let message): "retry-models-\(message.id)"; case .signIn: "sign-in"; case .share(let url): url.absoluteString; case .preview(let attachments, let index): attachments[index].id.uuidString } }
    }
    init() {
        // Keep startup and persistence inside StateObject's lazy initialization.
        // Locale changes must not reload the workspace or reset live drafts.
        _store = StateObject(wrappedValue: Self.makeStore())
    }
    private static func makeStore() -> WorkspaceStore {
        let testing = ProcessInfo.processInfo.arguments.contains("--ui-testing")
        let root = testing ? FileManager.default.temporaryDirectory.appendingPathComponent("PotatoUITests") : nil
        #if DEBUG
        let streamConfiguration = CodeExecutionPreview.configuration ?? LocalModelPreview.configuration ?? ReasoningPreview.configuration
        #else
        let streamConfiguration = URLSessionConfiguration.ephemeral
        #endif
        let value = WorkspaceStore(storage: LocalStorage(root: root), resetForTesting: ProcessInfo.processInfo.arguments.contains("--reset"), streamConfiguration: streamConfiguration, productionDefaults: !testing || AppEnvironment.isFirstRunPreview)
        AppLocalization.shared.selection = value.settings.languageMode
        #if DEBUG
        DeveloperConnectionImport.apply(to: value)
        ReasoningPreview.prepare(value)
        LocalModelPreview.prepare(value)
        RecallPreview.prepare(value)
        CodeExecutionPreview.prepare(value)
        ActivityPreview.prepare(value)
        LibraryPreviewFixtures.prepare(value)
        if testing && ProcessInfo.processInfo.arguments.contains("--voice-preview") {
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []; chat.title = "语音交互验证"
                chat.messages = [ChatMessage(role: "user", text: "帮我安排一下明天的工作"), ChatMessage(role: "assistant", text: "可以，先说说你想完成哪些事。")]
            }
        }
        if testing && ProcessInfo.processInfo.arguments.contains("--syntax-preview") {
            let python = "from collections import Counter\n\ndef top_words(text: str, n: int = 10):\n    # 统计出现最多的单词\n    words = text.lower().split()\n    return Counter(words).most_common(n)\n\nprint(top_words(\"hello potato hello\"))"
            let markdown = "# 项目说明\n\n- **重点**：保留原始格式\n- 使用 `python` 处理数据\n\n[查看文档](https://example.com)\n\n> 这是一段引用"
            let language = ProcessInfo.processInfo.arguments.contains("--syntax-markdown") ? "markdown" : "python"
            let source = language == "python" ? python : markdown
            value.settings.appearanceMode = ProcessInfo.processInfo.arguments.contains("--syntax-dark") ? .dark : .light
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []; chat.title = "代码阅读"
                chat.messages = [ChatMessage(role: "user", text: language == "python" ? "写一段 Python 代码" : "展示 Markdown 源码"), ChatMessage(role: "assistant", text: "下面是一个例子。\n\n```\(language)\n\(source)\n```\n\n需要执行时，可以继续对话，让我在沙箱中运行并返回结果。")]
            }
        }
        if testing && ProcessInfo.processInfo.arguments.contains("--sidebar-code-preview") {
            let columns = (1...40).map { "column_\($0)" }.joined(separator: ", ")
            let headings = (1...10).map { "Column \($0)" }.joined(separator: "|")
            let separators = Array(repeating: "---", count: 10).joined(separator: "|")
            let samples = Array(repeating: "sample", count: 10).joined(separator: "|")
            let markdown = "横向内容手势测试\n\n```swift\nlet sample = [\(columns)]\n```\n\n|\(headings)|\n|\(separators)|\n|\(samples)|"
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []
                chat.messages = [ChatMessage(role: "assistant", text: markdown)]
            }
        }
        if testing && ProcessInfo.processInfo.arguments.contains("--markdown-preview") {
            let markdown = """
            三种部署方式的对比：

            | | 方案 A：本地部署 | 方案 B：云端托管 | 方案 C：混合 |
            |---|:---|:---:|---:|
            | 成本 | 一次性投入较高，需要自备服务器和运维人员 | 按量付费 | 中等 |
            | 延迟 | 低 | 取决于网络，跨区域访问可能超过 200ms | 低 |
            | 数据安全 | 数据完全留在内网<br>适合合规要求高的场景 | 依赖云厂商 | 敏感数据留本地 |
            | 命令 | `grep a \\| wc -l` | `npm i` | |

            | 项目 | 状态 |
            |---|---|
            | 计划 | 完成 |

            ## **建议**
            1. 先上云端托管
               验证需求后再评估
               - 关注月度账单
               - 预留迁移脚本
            2. 数据量大时切换混合方案

            > 成本估算基于公开报价
            > 实际以合同为准
            """
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []
                chat.messages = [ChatMessage(role: "user", text: "对比一下部署方案"), ChatMessage(role: "assistant", text: markdown)]
            }
        }
        if testing && ProcessInfo.processInfo.arguments.contains("--sidebar-long-chat-preview") {
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []
                chat.messages = (1...30).map { ChatMessage(role: "assistant", text: "历史消息 \($0)\n\n这是一段用于检查回看与追底的合成内容。向上查看历史后，只有主动点击回到最新消息才恢复追底。") }
            }
        }
        if testing && ProcessInfo.processInfo.arguments.contains("--glass-reading-preview") {
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []
                chat.title = "周末，给自己留一点空白"
                chat.messages = [
                    ChatMessage(role: "user", text: "这个周末想出去走走，但不想把行程排得太满。帮我安排一个轻松一点的两日计划吧。"),
                    ChatMessage(role: "assistant", text: """
                    可以把周末分成两种节奏：**周六去户外，周日留在附近。** 每天只安排一件最想做的事，剩下的时间随心走。

                    ### 周六 · 一段不赶路的散步

                    上午睡到自然醒，吃过早餐再出门。选一条交通方便、随时可以折返的河边步道或公园路线，不必追求走多远。

                    - **上午：** 带一瓶水，慢慢走四十分钟。遇到喜欢的地方就坐一会儿。
                    - **中午：** 找一家附近的小店吃饭，给午餐留足时间。
                    - **下午：** 看体力决定继续散步，或者到书店翻一会儿书。

                    比起打卡几个地点，这一天更适合观察一些平时忽略的小事：树影、街角的花，或者一条没走过的小路。
                    """),
                    ChatMessage(role: "user", text: "这个节奏挺好。如果周日下雨，就不想再跑很远了。想找个安静的地方读书，再留点时间整理照片。"),
                    ChatMessage(role: "assistant", text: """
                    ### 周日 · 把时间留给自己

                    下雨天可以把活动范围收在家附近。选择一间安静、采光舒服的咖啡馆，带一本一直想读的书。手机调成静音，给自己一段不被打断的时间。

                    **不必给读书设进度。** 读几页、看看窗外，或者把想到的事情记下来，都算是很好的休息。

                    下午回家整理周六拍的照片，只挑最喜欢的五张，给每一张写一句简短的说明。留下当时的感受，比把所有照片分类更有意思。

                    ### 出门前的小清单

                    1. 水、纸巾和一把轻便的伞。
                    2. 舒适的鞋，以及一本能随手翻开的书。
                    3. 提前看一下天气，给返程留一点余地。

                    如果临时不想出门，也可以把计划缩成家附近的一小段散步。**舒服的节奏，比完整执行行程更重要。**
                    """),
                    ChatMessage(role: "user", text: "那就按这个安排。周六走累了就提前回来，周日只保留读书和整理照片两件事。"),
                    ChatMessage(role: "assistant", text: """
                    ### 给周末留一点空白

                    可以。周六把散步当成唯一的安排，午饭后再决定下一步；周日留一个安静的上午读书，下午挑几张喜欢的照片。

                    不用把空出来的时间补满。临时想坐一会儿、换一条路，或者早点回家，都可以。
                    """)
                ]
            }
        }
        #endif
        if testing && ProcessInfo.processInfo.arguments.contains("--slow-stream") { value.demoDelay = .milliseconds(80) }
        return value
    }
    private var chat: Conversation { store.selected }
    private var prompt: Binding<String> { Binding(get: { store.selected.input }, set: { value in store.update { $0.input = value } }) }
    private var draftBinding: Binding<WorkingDraft> { Binding(get: { store.selected.draft ?? WorkingDraft() }, set: { value in store.update { $0.draft = value } }) }
    var body: some View {
        GeometryReader { geometry in
            let drawerWidth = min(360, geometry.size.width * 0.74)
            let drawerOffset = sidebarDrag?.active == true ? sidebarDrag!.offset : (sidebarVisible ? drawerWidth : 0)
            let drawerProgress = drawerOffset / drawerWidth
            ZStack(alignment: .topLeading) {
                Palette.canvas.ignoresSafeArea()
                Group {
                    WorkspaceSidebar(store: store, remoteSelected: remoteVisible, librarySelected: libraryVisible, selectRemote: {
                        voice.interrupt(); libraryVisible = false; remoteVisible = true; setSidebar(false)
                    }, selectChat: { id in
                        store.select(id); libraryVisible = false; remoteVisible = false; setSidebar(false)
                    }, newChat: {
                        voice.interrupt(); store.newChat(); libraryVisible = false; documentVisible = false; remoteVisible = false; setSidebar(false)
                    }, library: { voice.interrupt(); libraryVisible = true; remoteVisible = false; setSidebar(false) }, history: { libraryVisible = false; setSidebar(false); modal = .history }, settings: { modal = .settings })
                    .frame(width: drawerWidth, height: geometry.size.height)
                    .accessibilityElement(children: .contain)
                    .accessibilityHidden(!sidebarVisible)
                    .allowsHitTesting(sidebarVisible)
                    .accessibilityAction(.escape) { setSidebar(false) }
                }
                // One concrete page container keeps the moving surface and its
                // accessibility/hit regions together through drawer transitions.
                ZStack {
                    if libraryVisible {
                        LibraryView(store: store, openSidebar: { setSidebar(true) }, used: {
                            libraryVisible = false; remoteVisible = false; documentVisible = false; inputFocused = false
                        })
                    } else if remoteVisible {
                        NavigationStack {
                            RemoteView(store: remoteStore, openSidebar: { setSidebar(true) }, settings: store.settings)
                                .onAppear { remoteRootActive = true }
                                .onDisappear { remoteRootActive = false }
                        }
                    } else { workspace }
                }
                .padding(.top, geometry.safeAreaInsets.top)
                .padding(.bottom, geometry.safeAreaInsets.bottom)
                .frame(width: geometry.size.width, height: geometry.size.height + geometry.safeAreaInsets.top + geometry.safeAreaInsets.bottom)
                .background(Palette.canvas)
                .overlay(alignment: .top) {
                    if !libraryVisible && !remoteVisible && !chat.messages.isEmpty && !(documentVisible && chat.draft != nil) {
                        ChatStatusBackdrop().frame(height: geometry.safeAreaInsets.top + 8)
                    }
                }
                .accessibilityElement(children: .contain)
                .accessibilityHidden(sidebarVisible)
                .allowsHitTesting(!sidebarVisible)
                // The dimming surface and close target share the page's final clipping path.
                .overlay {
                    Palette.canvas.opacity(drawerProgress * 0.58).allowsHitTesting(false)
                    if sidebarVisible {
                        Button { setSidebar(false) } label: { Color.clear.contentShape(Rectangle()) }
                            .buttonStyle(.plain)
                            .accessibilityLabel(L10n.tr("关闭侧栏")).accessibilityIdentifier("close-sidebar")
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: drawerProgress * 38, style: .continuous))
                .shadow(color: .black.opacity(drawerProgress * 0.09), radius: 10, x: -3, y: 0)
                .offset(x: drawerOffset, y: -geometry.safeAreaInsets.top)
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
            .contentShape(Rectangle())
            .background(SidebarPanBridge(enabled: canDragSidebar, open: sidebarVisible, registry: sidebarGestureRegistry,
                changed: { updateSidebarDrag($0, width: drawerWidth) }, ended: { predicted, cancelled in
                    guard !cancelled, let drag = sidebarDrag, drag.active else { cancelSidebarDrag(); return }
                    setSidebar(drag.destination(predicted: predicted))
                }))
            .environment(\.sidebarGestureRegistry, sidebarGestureRegistry)
            .onChange(of: scenePhase) { _, phase in
                if phase != .active { cancelSidebarDrag() }
                if phase == .background { remoteStore.suspendConnections() }
            }
            .task(id: scenePhase) {
                guard scenePhase == .active else { return }
                AppLocalization.shared.refreshSystemLanguage()
                store.resumeCloudReplies()
                await store.refreshLocalModelsInBackground()
            }
            .onChange(of: geometry.size.width) { _, _ in cancelSidebarDrag() }
            .onChange(of: remoteRootActive) { _, active in if remoteVisible && !active { cancelSidebarDrag() } }
            .onAppear {
                availableHeight = geometry.size.height
                #if DEBUG
                if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--remote-preview") { remoteVisible = true }
                if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--library-open") { libraryVisible = true }
                #endif
            }
            .onChange(of: geometry.size.height) { _, height in availableHeight = height }
            .onChange(of: store.selectedID) { _, _ in remoteVisible = false; libraryVisible = false }
            .onChange(of: store.signInRequested) { _, requested in
                guard requested else { return }
                store.signInRequested = false; inputFocused = false; modal = .signIn
            }
            .sheet(item: $modal) { item in
                switch item {
                case .history: ConversationHistoryView(store: store)
                case .settings: SettingsView(store: store, appearancePreview: $appearancePreview)
                case .models: LocalModelPicker(store: store, openConnection: { modal = .settings }, signIn: { modal = .signIn })
                case .retryModels(let message):
                    LocalModelPicker(store: store, openConnection: {}, signIn: { modal = .signIn }, retryMessage: message, regenerate: { choice in
                        speech.stop(); return store.retry(messageID: message.id, using: choice)
                    })
                case .signIn: CloudAccountView(store: store, connected: {})
                case .share(let url): ActivitySheet(items: [url])
                case .preview(let attachments, let index): AttachmentPreview(attachments: attachments, storage: store.storage, index: index)
                }
            }
        }
        .background(WindowAppearance(mode: appearancePreview ?? store.settings.appearanceMode).allowsHitTesting(false))
    }
    private var canDragSidebar: Bool {
        (sidebarVisible || !remoteVisible || remoteRootActive) && modal == nil && !inputExpanded && !showAttachments && attachmentDestination == nil && editingMessage == nil
    }
    private func prepareSidebarInteraction() {
        inputFocused = false
        voice.interrupt()
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }
    private func setSidebar(_ visible: Bool) {
        prepareSidebarInteraction()
        withAnimation(reduceMotion ? nil : .smooth(duration: 0.24)) {
            sidebarDrag = nil; sidebarVisible = visible
        }
    }
    private func cancelSidebarDrag() {
        guard sidebarDrag != nil else { return }
        withAnimation(reduceMotion ? nil : .smooth(duration: 0.20)) { sidebarDrag = nil }
    }
    private func updateSidebarDrag(_ translation: CGSize, width: CGFloat) {
        var drag = sidebarDrag ?? SidebarDrag(open: sidebarVisible, width: width, allowed: canDragSidebar)
        let wasActive = drag.active
        drag.update(translation)
        if drag.active && !wasActive { prepareSidebarInteraction() }
        withTransaction(Transaction(animation: nil)) { sidebarDrag = drag }
    }
    private var workspace: some View {
        Group {
            if documentVisible && chat.draft != nil {
                VStack(spacing: 0) {
                    if !expanded { workspaceHeader }
                    if !expanded && !inputFocused { contextPreview }
                    DocumentPanel(draft: draftBinding, expanded: $expanded, keyboardVisible: inputFocused, close: { animate { documentVisible = false; expanded = false } }, share: shareText)
                    composerStack
                }
            } else {
                Group {
                    if chat.messages.isEmpty { if voice.active { Color.clear } else { welcome } }
                    else { conversation }
                }
                // Keep dismissal on the reading surface, outside the composer and its gestures.
                .contentShape(Rectangle())
                .simultaneousGesture(TapGesture().onEnded { inputFocused = false })
                .chatBar(edge: .top) { workspaceHeader }
                .chatBar(edge: .bottom) { composerStack }
                .chatScrollEdges()
            }
        }.foregroundStyle(Palette.ink).background(Palette.canvas.ignoresSafeArea())
            .fullScreenCover(isPresented: $inputExpanded, onDismiss: { if restoreInputAfterExpansion { inputFocused = true } }) {
                ExpandedComposer(text: prompt, selection: $inputSelection, canSend: canSend, collapse: { inputExpanded = false }, send: {
                    restoreInputAfterExpansion = false; inputExpanded = false; sendInput()
                })
            }

            .sheet(item: $editingMessage) { message in
                TextEditingSheet(title: L10n.tr("编辑为新分支"), text: $editedText, saveLabel: L10n.tr("重新发送")) { store.editAndResend(message, text: editedText); editingMessage = nil }
            }
            .sheet(isPresented: $showAttachments, onDismiss: {
                if attachmentLibraryRequested { attachmentLibraryRequested = false; libraryVisible = true }
                else if let destination = nextAttachmentDestination { nextAttachmentDestination = nil; attachmentDestination = destination }
            }) {
                AttachmentSheet(store: store, imports: attachmentImports, chatID: store.selectedID,
                    openLibrary: { attachmentLibraryRequested = true },
                    openPhotos: { nextAttachmentDestination = .photos(store.selectedID) },
                    openFiles: { nextAttachmentDestination = .files(store.selectedID) })
            }
            .sheet(item: $attachmentDestination, onDismiss: { showAttachments = true }) { destination in
                switch destination {
                case .photos(let id):
                    AttachmentPhotosPicker(limit: max(1, attachmentImports.remaining(in: store.conversations.first { $0.id == id } ?? chat))) { results in
                        attachmentImports.importPickerResults(results, to: id, store: store); attachmentDestination = nil
                    }
                case .files(let id):
                    AttachmentDocumentPicker { urls in
                        if let urls { attachmentImports.importFiles(urls, to: id, store: store) }
                        attachmentDestination = nil
                    }
                }
            }
            .onChange(of: store.selectedID) { _, _ in showAttachments = false; voice.interrupt(); restoreInputAfterExpansion = false; inputExpanded = false; expandAfterVoice = false; inputOverflowing = false; inputSelection = nil; inputFocused = false; expanded = false; documentVisible = store.selected.draft != nil; followBottom = true; speech.stop() }
            .onChange(of: scenePhase) { _, value in
                voice.foreground = value == .active
                if value == .background || (value == .inactive && voice.phase != .connecting) { voice.interrupt() }
                if value != .active { store.persist(); speech.stop() }
                else { store.syncRecallInBackground(cleanup: true) }
            }
            .onChange(of: voice.editRevision) { _, _ in
                inputSelection = voice.finalSelection
                if expandAfterVoice { expandAfterVoice = false; expandInput() }
                else { inputFocused = true }
            }
            .onChange(of: voice.sentRevision) { _, _ in inputSelection = nil; inputFocused = false; documentVisible = false; expanded = false; followBottom = true }
            .onChange(of: voice.active) { _, active in if !active { inputSelection = voice.finalSelection } }
            .onChange(of: modal?.id) { _, value in if value != nil { voice.interrupt() } }
            .onDisappear { voice.interrupt() }
            .alert("Potato", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) { Button(L10n.tr("知道了"), role: .cancel) { store.error = nil } } message: { Text(store.error ?? "") }
            .alert(L10n.tr("重命名对话"), isPresented: $renaming) {
                TextField(L10n.tr("对话标题"), text: $renameText)
                Button(L10n.tr("取消"), role: .cancel) {}
                Button(L10n.tr("保存")) { store.rename(chat.id, to: renameText) }
            }
            .overlay(alignment: .top) { if let toast { Text(toast).font(.subheadline).padding(12).background(.regularMaterial, in: Capsule()).padding(.top, 60).allowsHitTesting(false).accessibilityAddTraits(.updatesFrequently) } }
    }
    private var workspaceHeader: some View {
        VStack(spacing: 0) {
            navigationBar
            if let generatingID = store.generatingID, generatingID != chat.id {
                Button { store.select(generatingID) } label: {
                    Label(L10n.tr("另一段对话正在生成 · 点按查看"), systemImage: "bubble.left.and.text.bubble.right")
                        .font(.footnote).frame(maxWidth: .infinity, minHeight: 44)
                }.background(.regularMaterial, in: Capsule()).padding(.horizontal, 16)
            }
        }
    }
    private var navigationBar: some View {
        HStack {
            IconButton(symbol: "line.3.horizontal", label: L10n.tr("历史会话"), id: "history") { setSidebar(true) }.chatGlass(in: Circle(), interactive: true)
            Spacer()
            HStack(spacing: 2) {
                IconButton(symbol: "square.and.pencil", label: L10n.tr("新对话"), id: "new-chat") { voice.interrupt(); store.newChat(); documentVisible = false }
                Menu {
                    // Actions for this conversation first; settings live in the sidebar.
                    if !chat.messages.isEmpty {
                        Button(L10n.tr("重命名"), systemImage: "pencil") { inputFocused = false; renameText = chat.displayTitle; renaming = true }
                        Button(chat.pinned ? L10n.tr("取消置顶") : L10n.tr("置顶"), systemImage: "pin") { store.update { $0.pinned.toggle() }; store.persist() }
                        Button(L10n.tr("分享对话"), systemImage: "square.and.arrow.up") { shareText(chat.messages.map { "## \($0.role == "user" ? L10n.tr("我") : "Potato")\n\n\($0.displayText)" }.joined(separator: "\n\n")) }
                    }
                    if AppEnvironment.isUITesting { Button(L10n.tr("打开示例文稿"), systemImage: "doc.text") { store.addExample(); documentVisible = true } }
                    if !chat.messages.isEmpty {
                        Divider()
                        Button(L10n.tr("移到最近删除"), systemImage: "trash", role: .destructive) { voice.interrupt(); store.trash(chat.id) }
                    }
                } label: { Image(systemName: "ellipsis").font(.system(size: 19, weight: .medium)).frame(width: 44, height: 44).contentShape(Circle()) }.accessibilityLabel(L10n.tr("更多")).accessibilityIdentifier("more")
            }.padding(.horizontal, 3).chatGlass(in: Capsule())
        }.padding(.horizontal, 16).padding(.vertical, 8)
    }
    private var contextPreview: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Spacer(minLength: 36); Text(chat.messages.first?.text ?? "").font(.body).padding(.horizontal, 17).padding(.vertical, 12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 22)).lineLimit(2) }
            Button { documentVisible = false } label: {
                HStack(alignment: .top) { Text(chat.messages.last?.text ?? "").font(.body).multilineTextAlignment(.leading).lineLimit(2); Spacer(minLength: 0); Image(systemName: "chevron.up.chevron.down").font(.caption) }
            }.buttonStyle(.plain).accessibilityLabel(L10n.tr("查看完整对话")).accessibilityIdentifier("show-conversation")
        }.padding(.horizontal, 20).padding(.top, 6).padding(.bottom, 16)
    }
    private var welcome: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack(spacing: 18) {
                    Image("PotatoMark").resizable().scaledToFit().frame(width: 48, height: 48).clipShape(Circle()).accessibilityHidden(true)
                    Text(L10n.tr("今天，想聊什么？"))
                        .font(.system(.title2, design: .rounded).weight(.medium))
                        .multilineTextAlignment(.center)
                    if store.requiresSignIn {
                        Button { inputFocused = false; modal = .signIn } label: {
                            Text(L10n.tr("登录 Potato 账号")).font(.body.weight(.semibold)).padding(.horizontal, 22).frame(minHeight: 46)
                                .foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule())
                        }.buttonStyle(.plain).accessibilityIdentifier("welcome-sign-in").padding(.top, 4)
                    }
                }
                .padding(24)
                .frame(maxWidth: .infinity)
                .frame(minHeight: geometry.size.height)
                .background(ScrollActivityObserver { inputFocused = false })
            // Let the native scroll pan begin even when the welcome content fits.
            }.scrollBounceBehavior(.always).scrollDismissesKeyboard(.interactively)
        }.accessibilityIdentifier("chat-welcome")
    }
    private var conversation: some View {
        ScrollViewReader { proxy in
            ScrollView {
                // Streaming rows change height continuously. Keep their measured
                // layout instead of revising lazy estimates while following the end.
                VStack(alignment: .leading, spacing: 24) {
                    ForEach(chat.messages) { message in messageRow(message).id(message.id) }
                    if let draft = chat.draft {
                        Button { inputFocused = false; documentVisible = true } label: {
                            HStack(spacing: 14) { Image(systemName: "doc.text").font(.system(size: 24)); Text(draft.title).font(.headline).fixedSize(horizontal: false, vertical: true); Spacer(minLength: 0); Image(systemName: "chevron.right").font(.footnote) }.padding(18).background(Palette.surface, in: RoundedRectangle(cornerRadius: 18))
                        }.buttonStyle(.plain).accessibilityIdentifier("open-document")
                    }
                    Color.clear.frame(height: 1).id("bottom")
                }.padding(.horizontal, 20).padding(.vertical, 20)
                    .background(GeometryReader { geometry in
                        Color.clear.preference(key: ConversationHeightKey.self, value: geometry.size.height)
                    })
                    .background(ScrollActivityObserver { followBottom = false; inputFocused = false })
            }.accessibilityIdentifier("conversation").scrollDismissesKeyboard(.interactively).scrollClipDisabled()
                .onLatestOffscreenChange { offscreen in
                    latestOffscreen = offscreen
                    // Scrolling back down to the end resumes following new text.
                    if !offscreen { followBottom = true }
                }
                .onAppear { if let focus = store.recallFocusID { proxy.scrollTo(focus, anchor: .top) } else { proxy.scrollTo("bottom", anchor: .bottom) } }
                .onChange(of: store.recallFocusID) { _, id in if let id { followBottom = false; proxy.scrollTo(id, anchor: .top) } }
                // Follow after layout and only when its height actually changes,
                // not on every character (or before the new line is measured).
                .onPreferenceChange(ConversationHeightKey.self) { _ in
                    // Preferences arrive before UIScrollView commits its new
                    // contentSize. Scroll on the next main-loop turn, then check
                    // again that the user has not started reading history.
                    DispatchQueue.main.async {
                        guard followBottom else { return }
                        var transaction = Transaction(animation: nil)
                        transaction.disablesAnimations = true
                        withTransaction(transaction) { proxy.scrollTo("bottom", anchor: .bottom) }
                    }
                }
                .onChange(of: chat.messages.count) { _, _ in followBottom = true; proxy.scrollTo("bottom", anchor: .bottom) }
                .overlay(alignment: .bottom) {
                    if !followBottom && latestOffscreen {
                        IconButton(symbol: "arrow.down", label: L10n.tr("回到最新消息"), id: "scroll-latest") {
                            followBottom = true; animate { proxy.scrollTo("bottom", anchor: .bottom) }
                        }.chatGlass(in: Circle(), interactive: true).padding(.bottom, 10)
                    }
                }
        }
    }
    /// The slide image a code step rendered for this deck; those pages stay in the tool details, not the chat.
    private func deckCover(_ attachment: Attachment, in message: ChatMessage) -> Attachment? {
        guard (attachment.name as NSString).pathExtension.lowercased() == "pptx" else { return nil }
        let previewIDs = Set(message.displayCodeRuns.flatMap { $0.attachmentIDs ?? [] })
        return message.displayAttachments.first { $0.isImage && previewIDs.contains($0.id) }
    }
    private func messageRow(_ message: ChatMessage) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if message.role == "user" {
                HStack { Spacer(minLength: 36); VStack(alignment: .leading, spacing: 10) {
                    if !message.text.isEmpty { Text(message.text).textSelection(.enabled) }
                    let images = message.attachments.filter(\.isImage)
                    if !images.isEmpty { MessageImageGrid(attachments: images, storage: store.storage) { showPreview($0, among: images) } }
                    ForEach(message.attachments.filter { !$0.isImage }) { attachment in attachmentButton(attachment) }
                }.padding(16).background(Palette.muted, in: RoundedRectangle(cornerRadius: 22)) }
                    .contextMenu { Button(L10n.tr("复制"), systemImage: "doc.on.doc") { copy(message.displayText) }; Button(L10n.tr("编辑并重新发送"), systemImage: "pencil") { editedText = message.text; editingMessage = message }.disabled(store.generatingID != nil) }
            } else {
                replyTimeline(message).id(message.selectedVersionID ?? message.id)
                let outputs = message.displayAttachments
                ForEach(outputs.filter { !$0.isImage }) { attachment in DeliverableCard(attachment: attachment, storage: store.storage, cover: deckCover(attachment, in: message)) { showPreview(attachment, among: outputs) } }
                if message.state == .streaming {
                    // Recall runs before the model and has no row of its own, so the dot names it.
                    let recalling = message.displayRecalls.last?.state == "searching" ? L10n.tr("正在检索历史与记忆") : ""
                    TurnStatusLine(title: message.cloudReply?.notice ?? recalling, label: message.phaseTitle, identifier: message.cloudReply?.notice == nil ? "generating" : "cloud-reply-status")
                }
                if let failure = message.displayFailure {
                    if isBareFailure(message) { failureRow(message, failure: failure) }
                    else { Label(failure, systemImage: "exclamationmark.circle").font(.footnote).foregroundStyle(.red) }
                }
                if message.displayState == .stopped && message.displayFailure == nil { Text(L10n.tr("已停止生成")).font(.caption).foregroundStyle(Palette.secondary) }
                if !message.displayRecalls.isEmpty { RecallSourcesView(runs: message.displayRecalls, store: store) }
                if !message.displaySearches.isEmpty { SearchSourcesButton(runs: message.displaySearches, showsActivity: false) }
                // A deck's rendered pages belong to the tool details. Keep unrelated
                // images and legacy images visible when no producing call is known.
                let hasDeck = outputs.contains { ($0.name as NSString).pathExtension.lowercased() == "pptx" }
                let previewIDs = Set(message.displayCodeRuns.flatMap { $0.attachmentIDs ?? [] })
                let visibleImages = outputs.filter { $0.isImage && (!hasDeck || !previewIDs.contains($0.id)) }
                if !visibleImages.isEmpty { MessageImageGrid(attachments: visibleImages, storage: store.storage) { showPreview($0, among: outputs) } }
                if message.state != .streaming, !isBareFailure(message) {
                    ReplyActions(message: message, isLast: message.id == chat.messages.last?.id, busy: store.generatingID != nil, store: store, speech: speech, retry: { speech.stop(); store.retry(messageID: message.id) }, save: { store.saveReplyAsDraft(message); documentVisible = true }, share: { shareText(message.displayText) }).disabled(voice.active)
                }
                if message.state != .streaming, message.versionCount > 1 {
                        HStack(spacing: 8) {
                            IconButton(symbol: "chevron.left", label: L10n.tr("上一个回复版本"), id: "previous-reply") { store.chooseReplyVersion(message.id, offset: -1) }.disabled(message.versionIndex == 0 || store.generatingID != nil)
                            Text("\(message.versionIndex + 1) / \(message.versionCount)").font(.caption).monospacedDigit().accessibilityLabel(L10n.tr("回复版本 \(message.versionIndex + 1)，共 \(message.versionCount) 个"))
                            IconButton(symbol: "chevron.right", label: L10n.tr("下一个回复版本"), id: "next-reply") { store.chooseReplyVersion(message.id, offset: 1) }.disabled(message.versionIndex == message.versionCount - 1 || store.generatingID != nil)
                            Spacer()
                        }.foregroundStyle(Palette.secondary)
                }
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
    /// Commentary and the steps it led to, in the order they happened.
    @ViewBuilder private func replyTimeline(_ message: ChatMessage) -> some View {
        let segments = message.turnSegments
        let streaming = message.state == .streaming
        let lastActivity = segments.lastIndex { if case .activity = $0 { true } else { false } }
        if !segments.isEmpty { VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(segments.enumerated()), id: \.element.id) { index, segment in
                let last = index == segments.count - 1
                switch segment {
                case .activity(_, let steps):
                    ActivitySummaryView(steps: steps, running: streaming && last, storage: store.storage, thinking: message.displayReasoning?.state == .streaming,
                                        identifier: index == lastActivity ? "activity-summary" : "activity-summary-\(index)",
                                        onExpand: { followBottom = false; inputFocused = false })
                        .transition(.opacity)
                case .text(_, let text):
                    StreamingMarkdown(text: text, streaming: streaming && last).environment(\.openURL, OpenURLAction { url in
                        if url.scheme == "potato", url.host == "conversation", let id = UUID(uuidString: url.lastPathComponent), let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "message" })?.value,
                           let source = message.displayRecalls.flatMap(\.sources).first(where: { $0.conversation == id.uuidString.lowercased() && $0.id == query.lowercased() }) { store.openRecallSource(source); return .handled }
                        return url.scheme == "potato" ? .discarded : .systemAction
                    }).transition(.opacity)
                }
            }
        // A new step or paragraph fades in rather than popping into place.
        }.animation(reduceMotion ? nil : .easeOut(duration: 0.25), value: segments.map(\.id)) }
    }
    private var composer: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !chat.pendingAttachments.isEmpty {
                if chat.pendingAttachments.count >= Attachment.maximumPerMessage { Text(L10n.tr("每条消息最多添加 \(Attachment.maximumPerMessage) 个附件。")).font(.caption2).foregroundStyle(Palette.secondary) }
                ScrollView(.horizontal, showsIndicators: false) { HStack(spacing: 8) {
                    ForEach(chat.pendingAttachments) { attachment in
                        HStack(spacing: 0) { attachmentButton(attachment); IconButton(symbol: "xmark.circle.fill", label: L10n.tr("移除 \(attachment.name)")) { store.removePendingAttachment(attachment.id) }.disabled(voice.active).composerControl() }.background(Palette.canvas, in: RoundedRectangle(cornerRadius: 12))
                    }
                } }
            }
            if importing { Label(L10n.tr("正在导入附件…"), systemImage: "clock").font(.caption).foregroundStyle(Palette.secondary) }
            if let failure = attachmentImports.failure(in: chat.id) {
                HStack {
                    Text(failure).font(.caption).foregroundStyle(Palette.secondary)
                    Spacer()
                    Button { attachmentImports.clearFailures(in: chat.id) } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }.accessibilityLabel(L10n.tr("关闭"))
                }
            }
            if voice.active {
                VoiceComposerPanel(voice: voice, text: chat.input, maximumHeight: inputMaximumHeight(voice: true)) {
                    expandAfterVoice = true; voice.finish(send: false)
                }
            } else {
            if inputOverflowing {
                HStack {
                    Spacer()
                    Button(L10n.tr("展开"), systemImage: "arrow.up.left.and.arrow.down.right", action: expandInput).accessibilityIdentifier("expand-input").composerControl()
                }.font(.caption).frame(minHeight: 44).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            }
            ZStack(alignment: .topLeading) {
                if chat.input.isEmpty { Text(documentVisible && chat.draft != nil ? L10n.tr("想调整哪一部分？") : L10n.tr("问问 Potato")).font(.body).foregroundStyle(Palette.secondary).allowsHitTesting(false).accessibilityHidden(true) }
                ComposerTextInput(text: prompt, selection: $inputSelection, focused: $inputFocused, placeholder: documentVisible && chat.draft != nil ? L10n.tr("想调整哪一部分？") : L10n.tr("问问 Potato"), minimumHeight: 40, maximumHeight: inputMaximumHeight(), onOverflow: { inputOverflowing = $0 })
            }.padding(.horizontal, 6).padding(.top, 4)
            if let notice = voice.notice {
                HStack(alignment: .top) {
                    Text(notice).font(.caption).foregroundStyle(Palette.secondary).accessibilityIdentifier("voice-notice")
                    Spacer(minLength: 0)
                    Button { voice.dismissNotice() } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }.accessibilityLabel(L10n.tr("关闭语音提示")).composerControl()
                }
            }
            HStack(spacing: 4) {
                Button { inputFocused = false; showAttachments = true } label: {
                    Image(systemName: "plus").font(.system(size: 19)).frame(width: 36, height: 36).background(Palette.ink.opacity(0.045), in: Circle()).frame(width: 44, height: 44).contentShape(Circle())
                }.accessibilityLabel(L10n.tr("添加附件")).accessibilityIdentifier("add-attachment").composerControl()
                // Before sign-in the welcome screen owns the call to action; no model to pick yet.
                if !store.requiresSignIn { Button { inputFocused = false; modal = store.settings.validatedURL == nil ? .settings : .models } label: {
                    HStack(spacing: 6) {
                        Text(store.settings.demo ? L10n.tr("本地体验") : store.settings.modelEntry(store.localModelChoice.model).displayName).lineLimit(1)
                        if !store.settings.demo && (store.localModelChoice.thinkingMode != nil || store.localModelChoice.reasoningEffort != nil) {
                            Text(store.localModelChoice.compactThinkingLabel).foregroundStyle(Palette.secondary).lineLimit(1).fixedSize()
                        }
                    }.font(.subheadline).dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 12).frame(minHeight: 36).background(Palette.ink.opacity(0.045), in: Capsule()).frame(minHeight: 44).contentShape(Capsule())
                }.accessibilityLabel(L10n.tr("模型与思考，\(store.settings.demo ? L10n.tr("本地体验") : store.localModelChoice.model)，\(store.localModelChoice.thinkingLabel)")).accessibilityIdentifier("connection-settings").composerControl() }
                Spacer(minLength: 0)
                IconButton(symbol: "mic", label: L10n.tr("开始语音输入"), id: "voice-input") { if store.requiresSignIn { inputFocused = false; modal = .signIn; return }; let selection = inputSelection; expandAfterVoice = false; inputFocused = false; speech.stop(); voice.start(store: store, selection: selection) }.background { Circle().fill(Palette.ink.opacity(0.045)).frame(width: 36, height: 36) }.disabled(importing || store.generatingID != nil).composerControl()
                Button {
                    if store.isGenerating { store.stop() }
                    else { sendInput() }
                    if store.settings.haptics { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
                } label: {
                    Image(systemName: store.isGenerating ? "stop.fill" : "arrow.up").font(.system(size: store.isGenerating ? 16 : 21, weight: .medium)).foregroundStyle(Palette.onInk).frame(width: 38, height: 38).background(canSend || store.isGenerating ? Palette.ink : Palette.secondary.opacity(0.35), in: Circle()).frame(width: 44, height: 44).contentShape(Circle())
                }.disabled(!store.isGenerating && !canSend).accessibilityLabel(store.isGenerating ? L10n.tr("停止生成") : L10n.tr("发送")).accessibilityIdentifier(store.isGenerating ? "stop-generation" : "send-message").composerControl()
            }
            }
        }.padding(8)
            .chatGlass(in: RoundedRectangle(cornerRadius: 26, style: .continuous))
            .composerWhitespaceFocus(enabled: !voice.active) { inputFocused = true }
            .overlay(alignment: .top) { if voice.active { Label(L10n.tr("上滑发送"), systemImage: "chevron.up").font(.system(size: 10)).foregroundStyle(.tertiary).offset(y: -19).accessibilityHidden(true) } }
            .padding(.horizontal, 16).padding(.bottom, 8).padding(.top, voice.active ? 24 : 6)
            .excludesSidebarGesture()
    }
    private var composerStack: some View {
        VStack(spacing: 8) {
            if store.authorizationExpired && !store.settings.demo {
                AuthorizationBanner(manualConnection: store.settings.cloudAccount == nil) { inputFocused = false; modal = store.settings.cloudAccount != nil ? .signIn : .settings }
                    .transition(.opacity)
            }
            composer
        }
    }
    private func isBareFailure(_ message: ChatMessage) -> Bool {
        message.displayState == .failed && message.displayText.isEmpty && message.displayReasoning == nil && message.displaySearches.isEmpty
            && message.displayCodeRuns.isEmpty && message.displayAttachments.isEmpty && message.displayRecalls.isEmpty
    }
    /// A reply that failed before producing anything: say why and offer the next step, without copy/share actions.
    private func failureRow(_ message: ChatMessage, failure: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: "exclamationmark.circle").foregroundStyle(.red).accessibilityHidden(true)
                Text(failure).font(.subheadline).fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: 10) {
                if failure == AuthorizationFailure.message {
                    Button { inputFocused = false; modal = store.settings.cloudAccount != nil ? .signIn : .settings } label: {
                        Text(store.settings.cloudAccount != nil ? L10n.tr("重新登录") : L10n.tr("检查设置")).font(.subheadline.weight(.semibold)).padding(.horizontal, 16).frame(minHeight: 36).foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule())
                    }.buttonStyle(.plain).frame(minHeight: 44).accessibilityIdentifier("failure-sign-in")
                }
                if message.id == chat.messages.last?.id {
                    Button { speech.stop(); store.retry(messageID: message.id) } label: {
                        Label(L10n.tr("重试"), systemImage: "arrow.clockwise").font(.subheadline.weight(.medium)).padding(.horizontal, 14).frame(minHeight: 36)
                            .overlay { Capsule().stroke(Palette.line, lineWidth: 1) }
                    }.buttonStyle(.plain).frame(minHeight: 44).disabled(store.generatingID != nil).accessibilityIdentifier("failure-retry")
                }
            }
            if message.id == chat.messages.last?.id {
                Button {
                    inputFocused = false; modal = .retryModels(message)
                } label: {
                    Label(L10n.tr("换模型重新回答"), systemImage: "arrow.triangle.2.circlepath").font(.subheadline).frame(minHeight: 44)
                }.buttonStyle(.plain).disabled(store.generatingID != nil || voice.active).accessibilityIdentifier("failure-change-model")
            }
        }.accessibilityElement(children: .contain).accessibilityIdentifier("reply-failure")
    }
    private var canSend: Bool { !importing && store.generatingID == nil && (!chat.input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !chat.pendingAttachments.isEmpty) }
    private func inputMaximumHeight(voice: Bool = false) -> CGFloat {
        let attachments: CGFloat = chat.pendingAttachments.isEmpty ? 0 : 100
        return min(voice ? 220 : 180, max(72, (availableHeight - 190 - attachments) * 0.55))
    }
    private func expandInput() {
        inputFocused = false; restoreInputAfterExpansion = true; inputExpanded = true
    }
    private func sendInput() {
        inputFocused = false; documentVisible = false; expanded = false; followBottom = true
        store.send(); inputSelection = nil; inputOverflowing = false
    }
    private func attachmentButton(_ attachment: Attachment) -> some View {
        Button { showPreview(attachment, among: chat.pendingAttachments) } label: {
            if attachment.isImage {
                AttachmentThumbnail(url: store.storage.url(for: attachment)).frame(width: 72, height: 72).clipShape(RoundedRectangle(cornerRadius: 12))
            } else {
            HStack(spacing: 8) { Image(systemName: attachment.isImage ? "photo" : "doc.text"); VStack(alignment: .leading, spacing: 3) { Text(attachment.name).lineLimit(1); Text(ByteCountFormatter.string(fromByteCount: Int64(attachment.size), countStyle: .file)).font(.caption2).foregroundStyle(Palette.secondary) } }.font(.caption).frame(maxWidth: 200, minHeight: 44).padding(.horizontal, 10)
            }
        }.buttonStyle(.plain).accessibilityLabel(L10n.tr("预览 \(attachment.name)")).composerControl()
    }
    private func showPreview(_ attachment: Attachment, among attachments: [Attachment]) {
        inputFocused = false
        let images = attachments.filter(\.isImage)
        if attachment.isImage, let index = images.firstIndex(where: { $0.id == attachment.id }) { modal = .preview(images, index) }
        else { modal = .preview([attachment], 0) }
    }
    private func copy(_ text: String) { UIPasteboard.general.string = text; toast = L10n.tr("已复制"); Task { try? await Task.sleep(for: .seconds(2)); toast = nil } }
    private func animate(_ action: () -> Void) { withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.22), action) }
    private func shareText(_ text: String) {
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("Exports")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent("Potato-\(UUID().uuidString.prefix(8)).md")
            try text.write(to: url, atomically: true, encoding: .utf8); modal = .share(url)
        } catch { store.error = L10n.tr("无法导出：\(error.localizedDescription)") }
    }
}
private struct ConversationHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

struct ActivitySheet: UIViewControllerRepresentable {
    var items: [Any]
    func makeUIViewController(context: Context) -> UIActivityViewController { UIActivityViewController(activityItems: items, applicationActivities: nil) }
    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
#Preview { WorkspaceView() }

extension View {
    /// Reports whether the end of the scroll content sits more than a line below the visible area.
    @ViewBuilder func onLatestOffscreenChange(_ action: @escaping (Bool) -> Void) -> some View {
        if #available(iOS 18.0, *) {
            onScrollGeometryChange(for: Bool.self) { geometry in
                let maxOffset = geometry.contentSize.height + geometry.contentInsets.bottom - geometry.containerSize.height
                return maxOffset - geometry.contentOffset.y > 40
            } action: { _, offscreen in action(offscreen) }
        } else {
            self
        }
    }
}
