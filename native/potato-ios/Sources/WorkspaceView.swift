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
    @State private var remoteRootActive = true
    @State private var documentVisible = true
    @State private var expanded = false
    @State private var modal: Modal?
    @State private var showImporter = false
    @State private var showPhotos = false
    @State private var photos: [PhotosPickerItem] = []
    @State private var importing = false
    @State private var editingMessage: ChatMessage?
    @State private var editedText = ""
    @State private var toast: String?
    @State private var followBottom = true
    @State private var sandboxRun: SandboxRun?
    @StateObject private var speech = ReplySpeech()
    @StateObject private var voice = VoiceComposer()
    @State private var inputSelection: NSRange?
    @State private var inputFocused = false
    @State private var availableHeight: CGFloat = 600
    @State private var inputOverflowing = false
    @State private var inputExpanded = false
    @State private var restoreInputAfterExpansion = true
    @State private var expandAfterVoice = false
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    private var reduceMotion: Bool {
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--reduce-motion-preview") { return true }
        #endif
        return systemReduceMotion
    }
    @Environment(\.scenePhase) private var scenePhase
    private enum Modal: Identifiable {
        case history, settings, memories, models, share(URL), preview([Attachment], Int)
        var id: String { switch self { case .history: "history"; case .settings: "settings"; case .memories: "memories"; case .models: "models"; case .share(let url): url.absoluteString; case .preview(let attachments, let index): attachments[index].id.uuidString } }
    }
    private struct SandboxRun: Identifiable { let id = UUID(); let code: String; let messageID: UUID; let files: [Attachment] }
    init() {
        let testing = ProcessInfo.processInfo.arguments.contains("--ui-testing")
        let root = testing ? FileManager.default.temporaryDirectory.appendingPathComponent("PotatoUITests") : nil
        #if DEBUG
        let streamConfiguration = CodeExecutionPreview.configuration ?? LocalModelPreview.configuration ?? ReasoningPreview.configuration
        #else
        let streamConfiguration = URLSessionConfiguration.ephemeral
        #endif
        let value = WorkspaceStore(storage: LocalStorage(root: root), resetForTesting: ProcessInfo.processInfo.arguments.contains("--reset"), streamConfiguration: streamConfiguration)
        #if DEBUG
        DeveloperConnectionImport.apply(to: value)
        ReasoningPreview.prepare(value)
        LocalModelPreview.prepare(value)
        RecallPreview.prepare(value)
        CodeExecutionPreview.prepare(value)
        if testing && ProcessInfo.processInfo.arguments.contains("--voice-preview") {
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []; chat.title = "语音交互验证"
                chat.messages = [ChatMessage(role: "user", text: "帮我安排一下明天的工作"), ChatMessage(role: "assistant", text: "可以，先说说你想完成哪些事。")]
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
        if testing && ProcessInfo.processInfo.arguments.contains("--sidebar-long-chat-preview") {
            value.update { chat in
                chat.draft = nil; chat.input = ""; chat.pendingAttachments = []
                chat.messages = (1...30).map { ChatMessage(role: "assistant", text: "历史消息 \($0)\n\n这是一段用于检查回看与追底的合成内容。向上查看历史后，只有主动点击回到最新消息才恢复追底。") }
            }
        }
        #endif
        if testing && ProcessInfo.processInfo.arguments.contains("--slow-stream") { value.demoDelay = .milliseconds(80) }
        _store = StateObject(wrappedValue: value)
    }
    private var chat: Conversation { store.selected }
    private var prompt: Binding<String> { Binding(get: { store.selected.input }, set: { value in store.update { $0.input = value } }) }
    private var draftBinding: Binding<WorkingDraft> { Binding(get: { store.selected.draft ?? WorkingDraft() }, set: { value in store.update { $0.draft = value } }) }
    var body: some View {
        GeometryReader { geometry in
            let drawerWidth = min(360, geometry.size.width * 0.74)
            let drawerOffset = sidebarDrag?.active == true ? sidebarDrag!.offset : (sidebarVisible ? drawerWidth : 0)
            let drawerProgress = drawerOffset / drawerWidth
            ZStack(alignment: .leading) {
                Palette.canvas.ignoresSafeArea()
                Group {
                    WorkspaceSidebar(store: store, remoteSelected: remoteVisible, selectRemote: {
                        voice.interrupt(); remoteVisible = true; setSidebar(false)
                    }, selectChat: { id in
                        store.select(id); remoteVisible = false; setSidebar(false)
                    }, newChat: {
                        voice.interrupt(); store.newChat(); documentVisible = false; remoteVisible = false; setSidebar(false)
                    }, library: { modal = .history }, settings: { modal = .settings })
                    .frame(width: drawerWidth)
                    .accessibilityHidden(!sidebarVisible)
                    .allowsHitTesting(sidebarVisible)
                    .accessibilityAction(.escape) { setSidebar(false) }
                }
                Group {
                    if remoteVisible {
                        NavigationStack {
                            RemoteView(store: remoteStore, openSidebar: { setSidebar(true) }, settings: store.settings)
                                .onAppear { remoteRootActive = true }
                                .onDisappear { remoteRootActive = false }
                        }
                    } else { workspace }
                }
                .clipShape(RoundedRectangle(cornerRadius: drawerProgress * 30))
                .shadow(color: .black.opacity(drawerProgress * 0.10), radius: 18, x: -4)
                .accessibilityHidden(sidebarVisible)
                .allowsHitTesting(!sidebarVisible)
                .overlay {
                    if sidebarVisible {
                        Button { setSidebar(false) } label: { Color.black.opacity(0.025) }
                            .accessibilityLabel("关闭侧栏").accessibilityIdentifier("close-sidebar")
                    }
                }
                .offset(x: drawerOffset)
            }
            .contentShape(Rectangle())
            .background(SidebarPanBridge(enabled: canDragSidebar, open: sidebarVisible, registry: sidebarGestureRegistry,
                changed: { updateSidebarDrag($0, width: drawerWidth) }, ended: { predicted, cancelled in
                    guard !cancelled, let drag = sidebarDrag, drag.active else { cancelSidebarDrag(); return }
                    setSidebar(drag.destination(predicted: predicted))
                }))
            .environment(\.sidebarGestureRegistry, sidebarGestureRegistry)
            .onChange(of: scenePhase) { _, phase in if phase != .active { cancelSidebarDrag() } }
            .onChange(of: geometry.size.width) { _, _ in cancelSidebarDrag() }
            .onChange(of: remoteRootActive) { _, active in if remoteVisible && !active { cancelSidebarDrag() } }
            .onAppear {
                availableHeight = geometry.size.height
                #if DEBUG
                if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--remote-preview") { remoteVisible = true }
                #endif
            }
            .onChange(of: geometry.size.height) { _, height in availableHeight = height }
            .onChange(of: store.selectedID) { _, _ in remoteVisible = false }
            .sheet(item: $modal) { item in
                switch item {
                case .history: LibraryView(store: store)
                case .settings: SettingsView(store: store)
                case .memories: RecallView(store: store)
                case .models: LocalModelPicker(store: store, openConnection: { modal = .settings })
                case .share(let url): ActivitySheet(items: [url])
                case .preview(let attachments, let index): AttachmentPreview(attachments: attachments, storage: store.storage, index: index)
                }
            }
        }
    }
    private var canDragSidebar: Bool {
        (sidebarVisible || !remoteVisible || remoteRootActive) && modal == nil && !inputExpanded && !showImporter && !showPhotos && editingMessage == nil && sandboxRun == nil
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
        VStack(spacing: 0) {
            if !expanded {
                navigationBar
                if let generatingID = store.generatingID, generatingID != chat.id {
                    Button { store.select(generatingID) } label: { Label("另一段对话正在生成 · 点按查看", systemImage: "bubble.left.and.text.bubble.right").font(.footnote).frame(maxWidth: .infinity, minHeight: 44) }.background(Palette.muted)
                }
            }
            if documentVisible && chat.draft != nil {
                if !expanded && !inputFocused { contextPreview }
                DocumentPanel(draft: draftBinding, expanded: $expanded, keyboardVisible: inputFocused, close: { animate { documentVisible = false; expanded = false } }, share: shareText)
                composer.background(.white)
            } else {
                if chat.messages.isEmpty { if voice.active { Spacer(minLength: 0) } else { welcome } }
                else { conversation }
                composer
            }
        }.foregroundStyle(Palette.ink).background(Palette.canvas.ignoresSafeArea())
            .fullScreenCover(isPresented: $inputExpanded, onDismiss: { if restoreInputAfterExpansion { inputFocused = true } }) {
                ExpandedComposer(text: prompt, selection: $inputSelection, attachmentCount: chat.pendingAttachments.count, canSend: canSend, collapse: { inputExpanded = false }, send: {
                    restoreInputAfterExpansion = false; inputExpanded = false; sendInput()
                })
            }

            .sheet(item: $editingMessage) { message in
                TextEditingSheet(title: "编辑为新分支", text: $editedText, saveLabel: "重新发送") { store.editAndResend(message, text: editedText); editingMessage = nil }
            }
            .sheet(item: $sandboxRun) { run in SandboxRunSheet(code: run.code, attachments: run.files, store: store, messageID: run.messageID) }
            .fileImporter(isPresented: $showImporter, allowedContentTypes: [.text, .pdf, .image], allowsMultipleSelection: true) { result in
                switch result { case .success(let urls): importFiles(urls); case .failure(let error): store.error = error.localizedDescription }
            }
            .photosPicker(isPresented: $showPhotos, selection: $photos, maxSelectionCount: max(1, 4 - chat.pendingAttachments.count), selectionBehavior: .ordered, matching: .images)
            .onChange(of: photos) { _, value in if !value.isEmpty { importPhotos(value) } }
            .onChange(of: store.selectedID) { _, _ in voice.interrupt(); restoreInputAfterExpansion = false; inputExpanded = false; expandAfterVoice = false; inputOverflowing = false; inputSelection = nil; inputFocused = false; expanded = false; documentVisible = store.selected.draft != nil; followBottom = true; speech.stop() }
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
            .alert("Potato", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) { Button("知道了", role: .cancel) { store.error = nil } } message: { Text(store.error ?? "") }
            .overlay(alignment: .top) { if let toast { Text(toast).font(.subheadline).padding(12).background(.regularMaterial, in: Capsule()).padding(.top, 60).allowsHitTesting(false).accessibilityAddTraits(.updatesFrequently) } }
    }
    private var navigationBar: some View {
        HStack {
            IconButton(symbol: "line.3.horizontal", label: "历史会话", id: "history") { setSidebar(true) }.background(.white.opacity(0.8), in: Circle())
            Spacer()
            IconButton(symbol: "square.and.pencil", label: "新对话", id: "new-chat") { voice.interrupt(); store.newChat(); documentVisible = false }.background(.white.opacity(0.8), in: Circle())
            Menu {
                Button("记忆与历史", systemImage: "brain") { inputFocused = false; modal = .memories }
                Button("设置", systemImage: "gearshape") { inputFocused = false; modal = .settings }
                Button("打开示例文稿", systemImage: "doc.text") { store.addExample(); documentVisible = true }
                if !chat.messages.isEmpty { Button("分享对话", systemImage: "square.and.arrow.up") { shareText(chat.messages.map { "## \($0.role == "user" ? "我" : "Potato")\n\n\($0.displayText)" }.joined(separator: "\n\n")) } }
            } label: { Image(systemName: "ellipsis").font(.system(size: 21)).frame(width: 44, height: 44).background(.white.opacity(0.8), in: Circle()) }.accessibilityLabel("更多").accessibilityIdentifier("more")
        }.overlay {
            if !chat.messages.isEmpty {
                VStack(spacing: 2) { Text("Potato").font(.title3.bold()); if store.settings.demo { Text("本地体验").font(.caption2).foregroundStyle(Palette.secondary) } }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).allowsHitTesting(false)
            }
        }.padding(.horizontal, 16).padding(.vertical, 8)
    }
    private var contextPreview: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Spacer(minLength: 36); Text(chat.messages.first?.text ?? "").font(.body).padding(.horizontal, 17).padding(.vertical, 12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 22)).lineLimit(2) }
            Button { documentVisible = false } label: {
                HStack(alignment: .top) { Text(chat.messages.last?.text ?? "").font(.body).multilineTextAlignment(.leading).lineLimit(2); Spacer(minLength: 0); Image(systemName: "chevron.up.chevron.down").font(.caption) }
            }.buttonStyle(.plain).accessibilityLabel("查看完整对话").accessibilityIdentifier("show-conversation")
        }.padding(.horizontal, 20).padding(.top, 6).padding(.bottom, 16)
    }
    private var welcome: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack(spacing: 18) {
                    Image("PotatoMark").resizable().scaledToFit().frame(width: 48, height: 48).clipShape(Circle()).accessibilityHidden(true)
                    Text("今天，想聊什么？")
                        .font(.system(.title2, design: .rounded).weight(.medium))
                        .multilineTextAlignment(.center)
                }
                .padding(24)
                .frame(maxWidth: .infinity)
                .frame(minHeight: geometry.size.height)
            }.scrollBounceBehavior(.basedOnSize).scrollDismissesKeyboard(.interactively)
        }.accessibilityIdentifier("chat-welcome")
    }
    private var conversation: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 24) {
                    ForEach(chat.messages) { message in messageRow(message).id(message.id) }
                    if let draft = chat.draft {
                        Button { inputFocused = false; documentVisible = true } label: {
                            HStack(spacing: 14) { Image(systemName: "doc.text").font(.system(size: 24)); VStack(alignment: .leading, spacing: 6) { Text(draft.title).font(.headline).fixedSize(horizontal: false, vertical: true); Text("工作文稿 · 点按继续编辑").font(.footnote).foregroundStyle(Palette.secondary) }; Spacer(minLength: 0); Image(systemName: "chevron.right").font(.footnote) }.padding(18).background(.white, in: RoundedRectangle(cornerRadius: 18))
                        }.buttonStyle(.plain).accessibilityIdentifier("open-document")
                    }
                    Color.clear.frame(height: 1).id("bottom")
                }.padding(.horizontal, 20).padding(.vertical, 20)
                    .background(ScrollActivityObserver { followBottom = false })
            }.accessibilityIdentifier("conversation").scrollDismissesKeyboard(.interactively)
                .onAppear { if let focus = store.recallFocusID { proxy.scrollTo(focus, anchor: .top) } else { proxy.scrollTo("bottom", anchor: .bottom) } }
                .onChange(of: store.recallFocusID) { _, id in if let id { followBottom = false; proxy.scrollTo(id, anchor: .top) } }
                .onChange(of: chat.messages.last?.text) { _, _ in if followBottom { proxy.scrollTo("bottom", anchor: .bottom) } }
                .onChange(of: chat.messages.last?.reasoning?.text) { _, _ in if followBottom { proxy.scrollTo("bottom", anchor: .bottom) } }
                .onChange(of: chat.messages.last?.searches) { _, _ in if followBottom { proxy.scrollTo("bottom", anchor: .bottom) } }
                .onChange(of: chat.messages.count) { _, _ in followBottom = true; proxy.scrollTo("bottom", anchor: .bottom) }
                .overlay(alignment: .bottomTrailing) {
                    if !followBottom { IconButton(symbol: "arrow.down", label: "回到最新消息", id: "scroll-latest") { followBottom = true; animate { proxy.scrollTo("bottom", anchor: .bottom) } }.background(.regularMaterial, in: Circle()).padding(16) }
                }
        }
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
                    .contextMenu { Button("复制", systemImage: "doc.on.doc") { copy(message.displayText) }; Button("编辑并重新发送", systemImage: "pencil") { editedText = message.text; editingMessage = message }.disabled(store.generatingID != nil) }
            } else {
                if let trace = message.displayReasoning { ReasoningProcessView(trace: trace, reduceMotion: reduceMotion, onExpand: { followBottom = false }).id(message.selectedVersionID ?? message.id) }
                if message.text.isEmpty && message.state == .streaming && message.displayReasoning == nil && message.displaySearches.isEmpty && message.displayCodeRuns.isEmpty { HStack(spacing: 10) { ProgressView(); Text("正在准备回复…").font(.subheadline).foregroundStyle(Palette.secondary) }.accessibilityIdentifier("generating") }
                if !message.displayText.isEmpty { MarkdownContent(text: message.displayText, runPython: message.state == .streaming || message.selectedVersionID != nil ? nil : { code in
                    sandboxRun = SandboxRun(code: code, messageID: message.id, files: Array(chat.messages.prefix(while: { $0.id != message.id }).flatMap(\.attachments).suffix(4)))
                }).environment(\.openURL, OpenURLAction { url in
                    if url.scheme == "potato", url.host == "conversation", let id = UUID(uuidString: url.lastPathComponent), let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "message" })?.value,
                       let source = message.displayRecalls.flatMap(\.sources).first(where: { $0.conversation == id.uuidString.lowercased() && $0.id == query.lowercased() }) { store.openRecallSource(source); return .handled }
                    return url.scheme == "potato" ? .discarded : .systemAction
                }) }
                if message.state == .streaming && !message.text.isEmpty && message.displayReasoning?.state != .streaming && !message.displayCodeRuns.contains(where: { $0.state == "running" }) { HStack(spacing: 8) { ProgressView().controlSize(.mini); Text("正在回复").font(.caption) }.foregroundStyle(Palette.secondary).accessibilityIdentifier("streaming-status") }
                if let failure = message.displayFailure { Label(failure, systemImage: "exclamationmark.circle").font(.footnote).foregroundStyle(.red) }
                if message.displayState == .stopped { Text("已停止生成").font(.caption).foregroundStyle(Palette.secondary) }
                ForEach(message.displayCodeRuns) { run in CodeExecutionView(run: run) }
                if !message.displayRecalls.isEmpty { RecallSourcesView(runs: message.displayRecalls, store: store) }
                if !message.displaySearches.isEmpty { SearchSourcesButton(runs: message.displaySearches) }
                if message.state == .streaming && message.text.isEmpty && message.displayReasoning == nil && !message.displaySearches.isEmpty && message.displaySearches.last?.state != "searching" { HStack { ProgressView().controlSize(.small); Text("正在整理搜索结果…").font(.caption) }.foregroundStyle(Palette.secondary) }
                if let execution = message.displayExecution {
                    DisclosureGroup(execution.status == "complete" ? "计算已完成" : "计算出错") {
                        Text([execution.stdout, execution.text, execution.stderr, execution.error ?? ""].filter { !$0.isEmpty }.joined(separator: "\n")).font(.system(.footnote, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                    }.padding(12).background(Palette.muted, in: RoundedRectangle(cornerRadius: 12))
                }
                let outputs = message.displayAttachments
                if !outputs.filter(\.isImage).isEmpty { MessageImageGrid(attachments: outputs.filter(\.isImage), storage: store.storage) { showPreview($0, among: outputs) } }
                ForEach(outputs.filter { !$0.isImage }) { attachment in attachmentButton(attachment) }
                if message.state != .streaming {
                    ReplyActions(message: message, isLast: message.id == chat.messages.last?.id, busy: store.generatingID != nil, store: store, speech: speech, retry: { speech.stop(); store.retry(messageID: message.id) }, save: { store.saveReplyAsDraft(message); documentVisible = true }, share: { shareText(message.displayText) }).disabled(voice.active)
                    if message.versionCount > 1 {
                        HStack(spacing: 8) {
                            IconButton(symbol: "chevron.left", label: "上一个回复版本", id: "previous-reply") { store.chooseReplyVersion(message.id, offset: -1) }.disabled(message.versionIndex == 0 || store.generatingID != nil)
                            Text("\(message.versionIndex + 1) / \(message.versionCount)").font(.caption).monospacedDigit().accessibilityLabel("回复版本 \(message.versionIndex + 1)，共 \(message.versionCount) 个")
                            IconButton(symbol: "chevron.right", label: "下一个回复版本", id: "next-reply") { store.chooseReplyVersion(message.id, offset: 1) }.disabled(message.versionIndex == message.versionCount - 1 || store.generatingID != nil)
                            Spacer()
                        }.foregroundStyle(Palette.secondary)
                    }
                }
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
    private var composer: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !chat.pendingAttachments.isEmpty {
                Text("附件 \(chat.pendingAttachments.count) / 4 · 左右滑动查看").font(.caption2).foregroundStyle(Palette.secondary)
                ScrollView(.horizontal, showsIndicators: false) { HStack(spacing: 8) {
                    ForEach(chat.pendingAttachments) { attachment in
                        HStack(spacing: 0) { attachmentButton(attachment); IconButton(symbol: "xmark.circle.fill", label: "移除 \(attachment.name)") { store.removePendingAttachment(attachment.id) }.disabled(voice.active) }.background(Palette.canvas, in: RoundedRectangle(cornerRadius: 12))
                    }
                } }
            }
            if importing { Label("正在导入附件…", systemImage: "clock").font(.caption).foregroundStyle(Palette.secondary) }
            if voice.active {
                VoiceComposerPanel(voice: voice, text: chat.input, maximumHeight: inputMaximumHeight(voice: true)) {
                    expandAfterVoice = true; voice.finish(send: false)
                }
            } else {
            if inputOverflowing {
                HStack {
                    Text("\(chat.input.count) 字").foregroundStyle(Palette.secondary)
                    Spacer()
                    Button("展开", systemImage: "arrow.up.left.and.arrow.down.right", action: expandInput).accessibilityIdentifier("expand-input")
                }.font(.caption).frame(minHeight: 44).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            }
            ZStack(alignment: .topLeading) {
                if chat.input.isEmpty { Text(documentVisible && chat.draft != nil ? "想调整哪一部分？" : "问问 Potato").font(.body).foregroundStyle(Palette.secondary).allowsHitTesting(false).accessibilityHidden(true) }
                ComposerTextInput(text: prompt, selection: $inputSelection, focused: $inputFocused, placeholder: documentVisible && chat.draft != nil ? "想调整哪一部分？" : "问问 Potato", maximumHeight: inputMaximumHeight(), onOverflow: { inputOverflowing = $0 })
            }.padding(.horizontal, 6).padding(.top, 4)
            if let notice = voice.notice {
                HStack(alignment: .top) {
                    Text(notice).font(.caption).foregroundStyle(Palette.secondary).accessibilityIdentifier("voice-notice")
                    Spacer(minLength: 0)
                    Button { voice.dismissNotice() } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }.accessibilityLabel("关闭语音提示")
                }
            }
            HStack(spacing: 4) {
                Menu {
                    Button("照片图库", systemImage: "photo") { inputFocused = false; showPhotos = true }
                    Button("选择文件", systemImage: "folder") { inputFocused = false; showImporter = true }
                } label: { Image(systemName: "plus").font(.system(size: 21)).frame(width: 44, height: 44) }.disabled(importing || chat.pendingAttachments.count >= 4).accessibilityLabel("添加附件").accessibilityIdentifier("add-attachment")
                Button { inputFocused = false; modal = store.settings.validatedURL == nil ? .settings : .models } label: {
                    HStack(spacing: 6) {
                        Text(store.settings.demo ? "本地体验" : store.settings.modelEntry(store.localModelChoice.model).name).lineLimit(1)
                        if !store.settings.demo {
                            Text(store.localModelChoice.compactThinkingLabel).foregroundStyle(Palette.secondary).lineLimit(1).fixedSize()
                        }
                    }.font(.subheadline).dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 12).frame(minHeight: 44).background(Palette.canvas, in: Capsule())
                }.accessibilityLabel("模型与思考，\(store.settings.demo ? "本地体验" : store.localModelChoice.model)，\(store.localModelChoice.thinkingLabel)").accessibilityIdentifier("connection-settings")
                Spacer(minLength: 0)
                IconButton(symbol: "mic", label: "开始语音输入", id: "voice-input") { let selection = inputSelection; expandAfterVoice = false; inputFocused = false; speech.stop(); voice.start(store: store, selection: selection) }.disabled(importing || store.generatingID != nil)
                Button {
                    if store.isGenerating { store.stop() }
                    else { sendInput() }
                    if store.settings.haptics { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
                } label: {
                    Image(systemName: store.isGenerating ? "stop.fill" : "arrow.up").font(.system(size: store.isGenerating ? 17 : 23, weight: .medium)).foregroundStyle(.white).frame(width: 44, height: 44).background(canSend || store.isGenerating ? Palette.ink : Palette.secondary.opacity(0.35), in: Circle())
                }.disabled(!store.isGenerating && !canSend).accessibilityLabel(store.isGenerating ? "停止生成" : "发送").accessibilityIdentifier(store.isGenerating ? "stop-generation" : "send-message")
            }
            }
        }.padding(voice.active ? 8 : 12).background(voice.active ? Color(white: 0.99) : .white, in: RoundedRectangle(cornerRadius: voice.active ? 26 : 28))
            .overlay { RoundedRectangle(cornerRadius: 28).stroke(voice.active ? .clear : Palette.line, lineWidth: 0.8) }.shadow(color: .black.opacity(0.035), radius: 12, x: 0, y: 4)
            .overlay(alignment: .top) { if voice.active { Label("上滑发送", systemImage: "chevron.up").font(.system(size: 10)).foregroundStyle(.tertiary).offset(y: -19).accessibilityHidden(true) } }
            .padding(.horizontal, 10).padding(.bottom, 8).padding(.top, voice.active ? 24 : 6)
            .excludesSidebarGesture()
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
        }.buttonStyle(.plain).accessibilityLabel("预览 \(attachment.name)")
    }
    private func showPreview(_ attachment: Attachment, among attachments: [Attachment]) {
        inputFocused = false
        let images = attachments.filter(\.isImage)
        if attachment.isImage, let index = images.firstIndex(where: { $0.id == attachment.id }) { modal = .preview(images, index) }
        else { modal = .preview([attachment], 0) }
    }
    private func importFiles(_ urls: [URL]) {
        guard urls.count + chat.pendingAttachments.count <= 4 else { store.error = "每条消息最多 4 个附件，请重新选择。"; return }
        importing = true
        let id = chat.id
        Task {
            defer { importing = false }
            for url in urls {
                do { let storage = store.storage
                    let attachment = try await Task.detached { try storage.importFile(url) }.value; store.update(id) { $0.pendingAttachments.append(attachment) } }
                catch { store.error = error.localizedDescription }
            }
        }
    }
    private func importPhotos(_ items: [PhotosPickerItem]) {
        guard items.count + chat.pendingAttachments.count <= 4 else { photos = []; store.error = "每条消息最多 4 个附件，请重新选择。"; return }
        importing = true
        let id = chat.id
        Task {
            defer { importing = false; photos = [] }
            for item in items {
                do {
                    guard let data = try await item.loadTransferable(type: Data.self) else { throw LocalFailure.message("照片无法读取，请重新选择。") }
                    let storage = store.storage
                    let attachment = try await Task.detached { try storage.importData(data, name: "照片.jpg", type: .image) }.value
                    store.update(id) { $0.pendingAttachments.append(attachment) }
                } catch { store.error = error.localizedDescription }
            }
        }
    }
    private func copy(_ text: String) { UIPasteboard.general.string = text; toast = "已复制"; Task { try? await Task.sleep(for: .seconds(2)); toast = nil } }
    private func animate(_ action: () -> Void) { withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.22), action) }
    private func shareText(_ text: String) {
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("Exports")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent("Potato-\(UUID().uuidString.prefix(8)).md")
            try text.write(to: url, atomically: true, encoding: .utf8); modal = .share(url)
        } catch { store.error = "无法导出：\(error.localizedDescription)" }
    }
}
struct ActivitySheet: UIViewControllerRepresentable {
    var items: [Any]
    func makeUIViewController(context: Context) -> UIActivityViewController { UIActivityViewController(activityItems: items, applicationActivities: nil) }
    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
#Preview { WorkspaceView() }
