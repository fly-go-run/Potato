import SwiftUI

@MainActor
final class WorkspaceStore: ObservableObject {
    @Published var conversations: [Conversation]
    @Published var selectedID: UUID
    @Published var settings: ConnectionSettings
    @Published var library: [LibraryItem] = []
    @Published var error: String?
    @Published var generatingID: UUID?
    @Published var recallNotice: String?
    @Published var memories: [PersonalMemory] = []
    /// The service rejected the saved sign-in (model list or a reply returned 401/403).
    @Published var authorizationExpired = false
    /// Set when someone tries to send before signing in; the workspace presents sign-in.
    @Published var signInRequested = false
    /// People must sign in to chat; automated tests keep the offline sample replies.
    var requiresSignIn: Bool { settings.demo && !allowsDemoReplies }
    private let allowsDemoReplies: Bool
    private var memoryConnection: String?
    var visibleMemories: [PersonalMemory] { memoryConnection == RecallService.digest((settings.serviceIdentity ?? settings.endpoint) + connectionToken) ? memories : [] }
    @Published var recallFocusID: UUID?
    @Published var recallSyncing = false
    private var recallSyncTask: Task<RecallStatus, Error>?
    let storage: LocalStorage
    let streamConfiguration: URLSessionConfiguration
    private let tokenProvider: (() -> String)?
    private var task: Task<Void, Never>?
    private var modelFetch: (id: UUID, endpoint: String?, token: String, task: Task<Void, Error>)?
    private var saveTask: Task<Void, Never>?
    private var loadFailed = false
    private var applyingCloudEvent = false
    var demoDelay: Duration = .milliseconds(16)
    var selected: Conversation { conversations.first { $0.id == selectedID } ?? conversations[0] }
    var isGenerating: Bool { generatingID == selectedID }
    var visibleConversations: [Conversation] { conversations.filter { $0.deletedAt == nil }.sorted { a, b in a.pinned == b.pinned ? a.updatedAt > b.updatedAt : a.pinned } }

    /// `productionDefaults`: a fresh install starts empty (no sample), empty leftover conversations are
    /// removed, and an edited legacy system prompt becomes the person's custom instructions.
    init(storage: LocalStorage = LocalStorage(), resetForTesting: Bool = false, streamConfiguration: URLSessionConfiguration = .ephemeral, tokenProvider: (() -> String)? = nil, productionDefaults: Bool = false) {
        allowsDemoReplies = AppEnvironment.isAutomatedTest && !productionDefaults
        self.storage = storage
        self.streamConfiguration = streamConfiguration
        self.tokenProvider = tokenProvider
        var loadError: Error?
        var saved: SavedWorkspace?
        do { if !resetForTesting { saved = try storage.load() } } catch { loadError = error }
        if let saved, !saved.conversations.isEmpty {
            conversations = saved.conversations
            selectedID = saved.selectedID
            settings = saved.settings
            if !conversations.contains(where: { $0.id == selectedID && $0.deletedAt == nil }) {
                let new = Conversation(); conversations.append(new); selectedID = new.id
            }
            for i in conversations.indices {
                for j in conversations[i].messages.indices where conversations[i].messages[j].state == .streaming {
                    if conversations[i].messages[j].cloudReply != nil {
                        let last = conversations[i].messages[j].reasoning?.lastReceivedAt ?? conversations[i].messages[j].createdAt
                        conversations[i].messages[j].reasoning?.finish(.complete, at: last)
                        conversations[i].messages[j].cloudReply?.notice = L10n.tr("正在恢复云端回复…")
                        continue
                    }
                    conversations[i].messages[j].recoverInterruptedReply()
                    conversations[i].messages[j].searches = conversations[i].messages[j].searches?.map { var run = $0; if run.state == "searching" { run.state = "stopped" }; return run }
                }
            }
        } else {
            let first = productionDefaults ? Conversation() : Conversation.example()
            conversations = [first]; selectedID = first.id; settings = ConnectionSettings()
        }
        if productionDefaults {
            conversations.removeAll { $0.isEmptyShell && $0.id != selectedID && $0.deletedAt == nil && !$0.pinned }
            // Earlier builds cut the first message at 28 characters, often inside a word.
            for i in conversations.indices where conversations[i].automaticTitle == nil {
                guard let first = conversations[i].messages.first(where: { $0.role == "user" })?.text.trimmingCharacters(in: .whitespacesAndNewlines),
                      first.count > 28, conversations[i].title == String(first.prefix(28)) else { continue }
                conversations[i].title = ConversationTitle.make(from: first); conversations[i].automaticTitle = false
            }
            // Someone who already set up a manual connection keeps access to it.
            if settings.developerMode == nil && !settings.demo && settings.cloudAccount == nil { settings.developerMode = true }
            if settings.systemPrompt != ConnectionSettings.defaultSystemPrompt && settings.customInstructions == nil {
                settings.customInstructions = settings.systemPrompt
                settings.systemPrompt = ConnectionSettings.defaultSystemPrompt
            }
        }
        if loadError != nil { loadFailed = true; error = L10n.tr("本地记录读取失败，原文件已保留。为避免覆盖，暂未保存新修改。") }
        else {
            library = saved?.library ?? []
            if saved != nil && saved?.library == nil { migrateLibrary() }
            persist()
            // Startup is the only time no attachment import can be in flight.
            if error == nil { try? storage.pruneUnreferencedAttachments(in: snapshot) }
        }
    }
    func update(_ id: UUID? = nil, _ change: (inout Conversation) -> Void) {
        guard let i = conversations.firstIndex(where: { $0.id == (id ?? selectedID) }) else { return }
        change(&conversations[i]); conversations[i].updatedAt = Date(); scheduleSave()
    }
    func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }; self?.persist()
        }
    }
    var connectionToken: String { tokenProvider?() ?? settings.connectionToken }
    var snapshot: SavedWorkspace { SavedWorkspace(conversations: conversations, selectedID: selectedID, settings: settings, library: library) }
    func commitLibrary(_ items: [LibraryItem]) throws {
        guard !loadFailed else { throw LocalFailure.message(L10n.tr("本地记录读取失败，暂时无法保存资料。原文件已保留。")) }
        if applyingCloudEvent { library = items; return }
        var saved = snapshot; saved.library = items
        try storage.save(saved)
        library = items
    }
    func persist() {
        guard !loadFailed, !applyingCloudEvent else { return }
        do { try storage.save(snapshot) }
        catch { self.error = L10n.tr("保存失败：\(error.localizedDescription)。请保留应用并检查设备可用空间。") }
    }
    func newChat() {
        if selected.messages.isEmpty && selected.input.isEmpty && selected.pendingAttachments.isEmpty && selected.deletedAt == nil { return }
        let chat = Conversation(); conversations.append(chat); selectedID = chat.id; persist()
    }
    func select(_ id: UUID) { selectedID = id; persist() }
    func addExample() { let chat = Conversation.example(); conversations.append(chat); selectedID = chat.id; persist() }
    func trash(_ id: UUID) {
        if generatingID == id { stop() }
        update(id) { $0.deletedAt = Date() }
        if selectedID == id {
            if let first = visibleConversations.first { selectedID = first.id }
            else { let chat = Conversation(); conversations.append(chat); selectedID = chat.id }
        }
        persist(); invalidateLocalRecall(id); syncRecallInBackground(cleanup: true)
    }
    /// Moves every conversation to Recently Deleted and starts a fresh one.
    func trashAll() {
        if generatingID != nil { stop() }
        let now = Date()
        for i in conversations.indices where conversations[i].deletedAt == nil && !conversations[i].isEmptyShell {
            conversations[i].deletedAt = now; invalidateLocalRecall(conversations[i].id)
        }
        if let empty = conversations.first(where: { $0.deletedAt == nil }) { selectedID = empty.id }
        else { let chat = Conversation(); conversations.append(chat); selectedID = chat.id }
        persist(); syncRecallInBackground(cleanup: true)
    }
    func rename(_ id: UUID, to title: String) {
        let value = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else { return }
        update(id) { $0.title = String(value.prefix(100)); $0.automaticTitle = false }
        persist()
    }
    func restore(_ id: UUID) { update(id) { $0.deletedAt = nil }; persist(); syncRecallInBackground() }
    func stop() {
        if let id = generatingID, let reply = conversations.first(where: { $0.id == id })?.messages.last(where: { $0.state == .streaming }), reply.cloudReply != nil {
            update(id) { chat in
                guard let i = chat.messages.firstIndex(where: { $0.id == reply.id }) else { return }
                chat.messages[i].cloudReply?.stopRequested = true
                chat.messages[i].cloudReply?.notice = L10n.tr("正在请求云端停止…")
            }
            persist()
            task?.cancel(); task = nil; generatingID = nil
            resumeCloudReplies()
            return
        }
        task?.cancel(); task = nil
        if let id = generatingID { update(id) { chat in
            if let i = chat.messages.lastIndex(where: { $0.state == .streaming }) {
                chat.messages[i].finishReply(.stopped)
                chat.messages[i].searches = chat.messages[i].searches?.map { var run = $0; if run.state == "searching" { run.state = "stopped" }; return run }
            }
        } }
        generatingID = nil; persist()
    }
    func send() {
        guard generatingID == nil else { error = L10n.tr("另一段对话仍在生成，请先停止或等待完成。"); return }
        let text = selected.input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !selected.pendingAttachments.isEmpty else { return }
        // Keep the draft and ask for sign-in instead of producing a sample reply.
        if requiresSignIn { signInRequested = true; return }
        let choice: LocalModelChoice?
        do { choice = try validatedLocalChoice() } catch { self.error = error.localizedDescription; return }
        let message = ChatMessage(role: "user", text: text, attachments: selected.pendingAttachments)
        do { try collectLibrary(message.attachments, conversationID: selectedID, messageID: message.id) }
        catch { self.error = L10n.tr("资料保存失败：\(error.localizedDescription)"); return }
        update { chat in
            if let choice { chat.modelChoice = choice }
            chat.messages.append(message)
            if ConversationTitle.defaults.contains(chat.title) {
                chat.title = text.isEmpty ? L10n.tr("附件对话") : ConversationTitle.make(from: text)
                chat.automaticTitle = true
            }
            chat.input = ""; chat.pendingAttachments = []
        }
        generate(choice: choice)
    }
    @discardableResult
    func retry(messageID: UUID? = nil, using override: LocalModelChoice? = nil) -> Bool {
        guard generatingID == nil, let old = selected.messages.last, old.role == "assistant",
              messageID == nil || messageID == old.id else { return false }
        // Upgrades can retain an old sample conversation. Keep its reply and draft
        // intact until sign-in, just as we do for a new message.
        if requiresSignIn { signInRequested = true; return false }
        let choice: LocalModelChoice?
        do {
            choice = settings.demo ? nil : (override ?? old.displayModelChoice ?? localModelChoice)
            try choice?.validate(settings: settings)
        } catch { self.error = error.localizedDescription; return false }
        var versions = old.versions ?? []
        versions.append(ReplyVersion(activityOrder: old.activityOrder, activityAnchors: old.activityAnchors, modelChoice: old.modelChoice, reasoning: old.reasoning, recalls: old.recalls, codeRuns: old.codeRuns, searches: old.searches, execution: old.execution, attachments: old.attachments, text: old.text, state: old.state, failure: old.failure, date: old.createdAt))
        update { $0.messages.removeLast() }
        generate(previousVersions: versions, choice: choice)
        return true
    }
    var localModelChoice: LocalModelChoice {
        selected.modelChoice ?? LocalModelChoice(endpoint: settings.serviceIdentity ?? "", model: settings.model)
    }
    private func validatedLocalChoice() throws -> LocalModelChoice? {
        if settings.demo { return nil }
        let choice = localModelChoice
        try choice.validate(settings: settings)
        return choice
    }
    func chooseLocalModel(_ choice: LocalModelChoice) throws {
        try choice.validate(settings: settings)
        update { $0.modelChoice = choice }; persist()
    }
    func reloadLocalModels(force: Bool = true) async throws {
        if !force, settings.currentCatalog?.isFresh() == true { return }
        let configuration = settings, token = connectionToken
        if let pending = modelFetch, pending.endpoint == configuration.serviceIdentity, pending.token == token {
            try await pending.task.value
            return
        }
        modelFetch?.task.cancel()
        let id = UUID()
        let request = Task { @MainActor in
            defer { if self.modelFetch?.id == id { self.modelFetch = nil } }
            let catalog: LocalModelCatalog
            do { catalog = try await LocalModelService.catalog(settings: configuration, token: token, configuration: self.streamConfiguration) }
            catch let failure as AuthorizationFailure { if token == self.connectionToken { self.authorizationExpired = true }; throw failure }
            try Task.checkCancellation()
            guard configuration.serviceIdentity == self.settings.serviceIdentity, token == self.connectionToken else { throw LocalFailure.message(L10n.tr("连接已改变，请重新读取模型列表。")) }
            self.installLocalModelCatalog(catalog); self.authorizationExpired = false; self.persist()
        }
        modelFetch = (id, configuration.serviceIdentity, token, request)
        try await request.value
    }
    func refreshLocalModelsInBackground() async {
        guard !settings.demo, settings.serviceIdentity != nil else { return }
        // Offline startup keeps the saved catalog without interrupting the conversation.
        try? await reloadLocalModels(force: false)
    }
    /// A request the service accepted proves the saved credential works again.
    private func credentialAccepted() { if authorizationExpired { authorizationExpired = false } }
    func installLocalModelCatalog(_ catalog: LocalModelCatalog) {
        settings.modelCatalog = catalog
        guard settings.cloudAccount != nil, catalog.endpoint == settings.serviceIdentity, !catalog.models.isEmpty else { return }
        let fallback = catalog.models.first(where: { $0.id == catalog.defaultModel }) ?? catalog.models[0]
        if !catalog.models.contains(where: { $0.id == settings.model }) { settings.model = fallback.id }
        for i in conversations.indices {
            guard let previous = conversations[i].modelChoice, previous.endpoint == catalog.endpoint else { continue }
            var next = previous
            if !catalog.models.contains(where: { $0.id == previous.model }) {
                let provider = previous.model.split(separator: "/").first.map(String.init)
                let replacement = catalog.models.first(where: { provider != nil && $0.id.hasPrefix(provider! + "/") }) ?? fallback
                next = LocalModelChoice(endpoint: catalog.endpoint, model: replacement.id)
            }
            if (try? next.validate(settings: settings)) == nil {
                next.thinkingMode = nil; next.reasoningEffort = nil
            }
            conversations[i].modelChoice = next
        }
    }
    func editAndResend(_ message: ChatMessage, text: String) {
        guard generatingID == nil, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              let index = selected.messages.firstIndex(where: { $0.id == message.id }) else { return }
        // Preserve the old branch in history before regenerating the edited turn.
        var branch = selected
        branch.id = UUID(); branch.title += L10n.tr(" · 编辑副本"); branch.isExample = false
        branch.messages = Array(branch.messages.prefix(index))
        branch.input = text; branch.pendingAttachments = message.attachments; branch.draft = nil
        conversations.append(branch); selectedID = branch.id
        send()
    }
    func removePendingAttachment(_ attachmentID: UUID) {
        update { $0.pendingAttachments.removeAll { $0.id == attachmentID } }
        // The file is reclaimed on the next successful startup, after persisted references are known.
        persist()
    }
    func addAttachment(_ attachment: Attachment) {
        guard selected.pendingAttachments.count < Attachment.maximumPerMessage else { error = L10n.tr("每条消息最多添加 \(Attachment.maximumPerMessage) 个附件。"); return }
        update { $0.pendingAttachments.append(attachment) }
    }
    func chooseReplyVersion(_ messageID: UUID, offset: Int) {
        guard generatingID == nil, let i = selected.messages.firstIndex(where: { $0.id == messageID }) else { return }
        let index = selected.messages[i].versionIndex + offset
        let versions = selected.messages[i].versions ?? []
        guard index >= 0 && index <= versions.count else { return }
        let chosen = index == versions.count ? nil : versions[index].id
        if i < selected.messages.count - 1 {
            // A past answer change starts a branch; later turns must not silently change their context.
            var branch = selected
            branch.id = UUID(); branch.title += L10n.tr(" · 回复分支"); branch.isExample = false
            branch.messages = Array(branch.messages.prefix(i + 1))
            branch.messages[i].selectedVersionID = chosen
            conversations.append(branch); selectedID = branch.id
            persist()
        } else {
            update { $0.messages[i].selectedVersionID = chosen }
        }
    }
    func saveReplyAsDraft(_ message: ChatMessage) {
        guard !message.displayText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        update { chat in
            if var draft = chat.draft {
                draft.replaceContent(message.displayText, reason: L10n.tr("替换文稿前"))
                chat.draft = draft
            } else {
                var draft = WorkingDraft(); draft.customMarkdown = message.displayText
                draft.title = String(message.displayText.components(separatedBy: .newlines).first?.trimmingCharacters(in: CharacterSet(charactersIn: "# ")).prefix(100) ?? Substring(L10n.tr("工作文稿")))
                chat.draft = draft
            }
        }
    }
    func importExecutionArtifacts(_ artifacts: [SandboxArtifact]) throws -> [Attachment] {
        var images = Set<String>()
        // Files follow rich previews in the response. Keep the named file when both match.
        let unique = artifacts.reversed().filter { artifact in
            guard artifact.name.lowercased().hasSuffix(".png"),
                  let data = Data(base64Encoded: artifact.base64),
                  let digest = ImageImport.pngPixelDigest(data) else { return true }
            return images.insert(digest).inserted
        }.reversed()
        return try unique.map { try storage.importArtifact($0) }
    }
    func saveExecution(_ execution: SandboxExecution, messageID: UUID, conversationID: UUID) throws {
        var saved = execution
        let files = try importExecutionArtifacts(execution.artifacts)
        try collectLibrary(files, conversationID: conversationID, messageID: messageID)
        saved.artifacts = [] // File bytes live in protected local storage, never duplicated in workspace JSON.
        update(conversationID) { chat in
            guard let index = chat.messages.firstIndex(where: { $0.id == messageID }) else { return }
            chat.messages[index].execution = saved
            chat.messages[index].attachments.append(contentsOf: files)
        }
        persist()
    }
    private func generate(previousVersions: [ReplyVersion] = [], choice: LocalModelChoice? = nil) {
        let id = selectedID
        let history = selected.messages
        let workingDraft = selected.draft
        let configuration = settings
        // Capture credentials with the endpoint before yielding; later settings edits cannot cross them.
        let credential = connectionToken
        var reply = ChatMessage(modelChoice: choice, role: "assistant", text: "", state: .streaming)
        reply.versions = previousVersions.isEmpty ? nil : previousVersions
        let replyID = reply.id
        update(id) { $0.messages.append(reply) }
        generatingID = id
        persist()
        task = Task { [weak self] in
            guard let self else { return }
            do {
                if configuration.demo {
                    let answer = L10n.tr("这是本地体验模式，尚未调用模型。你的消息和附件已保存在这台 iPhone。连接模型后，可以在这里获得真实回复；也可以先编辑、勾选和分享示例文稿。")
                    for chunk in answer.map(String.init) {
                        try await Task.sleep(for: self.demoDelay)
                        try Task.checkCancellation()
                        self.append(chunk, to: reply.id, in: id)
                    }
                } else {
                    if configuration.recallEnabled == true && self.conversations.first(where: { $0.id == id })?.recallExcluded != true {
                        try await self.synchronizeRecall()
                        try Task.checkCancellation()
                        guard configuration == self.settings, credential == self.connectionToken else { throw LocalFailure.message(L10n.tr("连接或设置已改变，请重新发送。")) }
                    }
                    var request = try ChatService.request(settings: configuration, token: credential, messages: history, storage: storage, draft: workingDraft, choice: choice)
                    if configuration.recallEnabled == true && self.conversations.first(where: { $0.id == id })?.recallExcluded != true {
                        var body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
                        body["recall"] = ["enabled": true, "auto_memory": configuration.automaticMemory == true, "timezone": TimeZone.current.identifier]
                        request.httpBody = try JSONSerialization.data(withJSONObject: body)
                    }
                    if let account = configuration.cloudAccount {
                        let job = CloudReply(id: replyID.uuidString.lowercased(), endpoint: configuration.serviceIdentity!, owner: account.owner, requestBody: request.httpBody)
                        self.update(id) { chat in if let i = chat.messages.firstIndex(where: { $0.id == replyID }) { chat.messages[i].cloudReply = job } }
                        // Refuse submission unless the recovery ID and exact request are durably saved.
                        try self.storage.save(self.snapshot)
                        await self.followCloudReply(messageID: replyID, conversationID: id)
                    } else {
                        for try await event in ChatService.events(request: request, configuration: self.streamConfiguration) {
                            try Task.checkCancellation()
                            switch event {
                            case .delta(let delta): self.receive(delta, messageID: replyID, conversationID: id)
                            case .search(let search): self.recordSearch(search, messageID: replyID, conversationID: id)
                            case .recall(let run): self.recordRecall(run, messageID: replyID, conversationID: id)
                            case .execution(let run): try self.recordCodeExecution(run, messageID: replyID, conversationID: id)
                            case .done: break
                            }
                        }
                        self.credentialAccepted()
                    }
                }
                try Task.checkCancellation()
                self.update(id) { chat in
                    if let i = chat.messages.firstIndex(where: { $0.id == reply.id }) {
                        if chat.messages[i].text.isEmpty {
                            let description = chat.messages[i].reasoning == nil ? L10n.tr("模型没有返回文字，请重新生成。") : L10n.tr("模型仅返回思考内容，未返回答复。已保留思考内容，可以重新生成。")
                            chat.messages[i].finishReply(.failed, failure: description)
                        } else { chat.messages[i].finishReply(.complete) }
                    }
                }
                self.generateTitleIfNeeded(conversationID: id, configuration: configuration, credential: credential)
            } catch {
                if error is AuthorizationFailure { self.authorizationExpired = true }
                if !Task.isCancelled {
                    self.update(id) { chat in
                        if let i = chat.messages.firstIndex(where: { $0.id == reply.id }) {
                            chat.messages[i].finishReply(.failed, failure: ChatService.failureDescription(error))
                        }
                    }
                }
            }
            if !Task.isCancelled { self.generatingID = nil; self.task = nil }
            self.persist(); self.syncRecallInBackground()
        }
    }
    private func append(_ text: String, to messageID: UUID, in id: UUID) {
        receive(ReplyDelta(text: text), messageID: messageID, conversationID: id)
    }
    private func receive(_ delta: ReplyDelta, messageID: UUID, conversationID id: UUID) {
        guard let i = conversations.firstIndex(where: { $0.id == id }), let j = conversations[i].messages.firstIndex(where: { $0.id == messageID }) else { return }
        let previous = conversations[i].messages[j].reasoning?.state
        conversations[i].messages[j].receive(delta)
        let message = conversations[i].messages[j]
        let count = message.text.count + (message.reasoning?.text.count ?? 0)
        // Checkpoint phase boundaries and larger additions, not every token.
        if previous != message.reasoning?.state || count % 200 < delta.text.count + delta.reasoning.count { persist() }
    }
    func recordSearch(_ search: WebSearchRun, messageID: UUID, conversationID: UUID) {
        update(conversationID) { chat in
            guard let index = chat.messages.firstIndex(where: { $0.id == messageID }) else { return }
            guard chat.messages[index].state != .stopped else { return }
            // Tool execution is a different phase; its waiting time is not reasoning time.
            if search.state == "searching" { chat.messages[index].reasoning?.finish(.complete, at: Date()) }
            var runs = chat.messages[index].searches ?? []
            if let existing = runs.firstIndex(where: { $0.id == search.id }) { runs[existing] = search }
            else if runs.count < 4 { runs.append(search) }
            chat.messages[index].recordActivity("search:\(search.id)")
            chat.messages[index].searches = runs
        }
    }
}

@MainActor
extension WorkspaceStore {
    func synchronizeRecall(cleanup: Bool = false) async throws {
        if let running = recallSyncTask { _ = try await running.value; await Task.yield(); return try await synchronizeRecall(cleanup: cleanup) }
        guard !settings.demo, settings.recallEnabled == true || (cleanup && settings.recallEnabled != nil) else { return }
        let configuration = settings, credential = connectionToken, chats = conversations
        let service = RecallService(settings: configuration, token: credential, configuration: streamConfiguration)
        recallSyncing = true
        let running = Task { try await service.sync(chats, storage: storage, deletionsOnly: configuration.recallEnabled != true) }
        recallSyncTask = running
        defer { recallSyncTask = nil; recallSyncing = false }
        do {
            let result = try await running.value
            guard configuration.serviceIdentity == settings.serviceIdentity, credential == connectionToken else { return }
            memoryConnection = RecallService.digest((configuration.serviceIdentity ?? configuration.endpoint) + credential)
            memories = result.memories
            recallNotice = L10n.tr("已同步 \(result.entries.values.filter { !$0.excluded }.count) 段对话 · \(memories.count) 条记忆")
        } catch { recallNotice = ChatService.failureDescription(error); throw error }
    }
    func syncRecallInBackground(cleanup: Bool = false) {
        guard !settings.demo, settings.recallEnabled == true || (cleanup && settings.recallEnabled != nil) else { return }
        Task { try? await synchronizeRecall(cleanup: cleanup) }
    }
    func refreshMemories() async {
        do {
            let configuration = settings, credential = connectionToken
            let result = try await RecallService(settings: configuration, token: credential, configuration: streamConfiguration).status()
            guard configuration.serviceIdentity == settings.serviceIdentity, credential == connectionToken else { return }
            memoryConnection = RecallService.digest((configuration.serviceIdentity ?? configuration.endpoint) + credential)
            memories = result.memories
        } catch { recallNotice = ChatService.failureDescription(error) }
    }
    func saveMemory(_ memory: PersonalMemory?, text: String, forget: Bool = false) async throws {
        let service = RecallService(settings: settings, token: connectionToken, configuration: streamConfiguration)
        try await service.memory(id: memory?.id ?? UUID().uuidString.lowercased(), text: text, base: memory?.revision, forget: forget)
        await refreshMemories()
    }
    func setRecallExcluded(_ excluded: Bool, conversation: UUID) {
        update(conversation) { $0.recallExcluded = excluded }; persist()
        if excluded { invalidateLocalRecall(conversation) }
        syncRecallInBackground(cleanup: true)
    }
    func invalidateLocalRecall(_ id: UUID) {
        memories.removeAll { $0.sources.contains { $0.conversation == id.uuidString.lowercased() } }
        for i in conversations.indices { for j in conversations[i].messages.indices {
            conversations[i].messages[j].recalls = conversations[i].messages[j].recalls?.map { var run = $0; run.sources.removeAll { $0.conversation == id.uuidString.lowercased() }; return run }
            if let versions = conversations[i].messages[j].versions {
                conversations[i].messages[j].versions = versions.map { var version = $0; version.recalls = version.recalls?.map { var run = $0; run.sources.removeAll { $0.conversation == id.uuidString.lowercased() }; return run }; return version }
            }
        } }
        persist()
    }
    func recordRecall(_ run: RecallRun, messageID: UUID, conversationID: UUID) {
        update(conversationID) { chat in
            guard let i = chat.messages.firstIndex(where: { $0.id == messageID }), chat.messages[i].state == .streaming else { return }
            var runs = chat.messages[i].recalls ?? []
            if let index = runs.firstIndex(where: { $0.id == run.id }) { runs[index] = run }
            else if runs.count < 12 { runs.append(run) }
            chat.messages[i].recordActivity("recall:\(run.id)")
            chat.messages[i].recalls = runs
        }
    }
    func openRecallSource(_ source: RecallSource) {
        guard let id = UUID(uuidString: source.conversation), let messageID = UUID(uuidString: source.id),
              let i = conversations.firstIndex(where: { $0.id == id && $0.deletedAt == nil && $0.recallExcluded != true }),
              let j = conversations[i].messages.firstIndex(where: { $0.id == messageID }) else { error = L10n.tr("原对话不在此设备，或已删除、排除。"); return }
        let message = conversations[i].messages[j]
        guard message.selectedVersionID?.uuidString.lowercased() == source.version else { error = L10n.tr("原对话已切换回复版本，请重新检索。"); return }
        let original = source.version.flatMap { version in message.versions?.first(where: { $0.id.uuidString.lowercased() == version })?.text } ?? message.text
        guard original.hasPrefix(source.text) else { error = L10n.tr("原消息内容已改变，请重新检索。"); return }
        selectedID = id; recallFocusID = nil
        Task { await Task.yield(); recallFocusID = messageID }
    }
}

extension WorkspaceStore {
    func resumeCloudReplies() {
        guard task == nil, !loadFailed else { return }
        for chat in conversations {
            guard let message = chat.messages.first(where: { $0.state == .streaming && $0.cloudReply != nil }) else { continue }
            generatingID = chat.id
            task = Task { [weak self] in
                guard let self else { return }
                await self.followCloudReply(messageID: message.id, conversationID: chat.id)
                guard !Task.isCancelled else { return }
                self.generatingID = nil; self.task = nil
                self.persist(); self.syncRecallInBackground()
                self.resumeCloudReplies()
            }
            return
        }
    }
    private func cloudMessage(_ messageID: UUID, in conversationID: UUID) -> ChatMessage? {
        conversations.first(where: { $0.id == conversationID })?.messages.first(where: { $0.id == messageID })
    }
    private func cloudNotice(_ text: String?, messageID: UUID, conversationID: UUID) {
        guard cloudMessage(messageID, in: conversationID)?.cloudReply?.notice != text else { return }
        update(conversationID) { chat in
            if let i = chat.messages.firstIndex(where: { $0.id == messageID }) { chat.messages[i].cloudReply?.notice = text }
        }
    }
    private func followCloudReply(messageID: UUID, conversationID: UUID) async {
        let service = CloudReplyService(configuration: streamConfiguration)
        var subscriptionAvailable = true
        var lastCheckpoint = ContinuousClock.now
        while !Task.isCancelled, let message = cloudMessage(messageID, in: conversationID), message.state == .streaming, let job = message.cloudReply {
            do {
                let credential = connectionToken
                guard job.createdAt > Date().addingTimeInterval(-7 * 86400) else { throw CloudReplyHTTPError(status: 410) }
                guard settings.cloudAccount?.owner == job.owner, settings.serviceIdentity == job.endpoint, !credential.isEmpty else {
                    cloudNotice(L10n.tr("请恢复原云端账号连接，以同步这条回复。"), messageID: messageID, conversationID: conversationID)
                    try await Task.sleep(for: .seconds(3)); continue
                }
                if job.stopRequested {
                    _ = try await service.request(job, token: credential, method: "POST")
                } else if job.requestBody != nil {
                    _ = try await service.request(job, token: credential, method: "PUT")
                    try Task.checkCancellation()
                    update(conversationID) { chat in
                        if let i = chat.messages.firstIndex(where: { $0.id == messageID }) { chat.messages[i].cloudReply?.requestBody = nil }
                    }
                    try storage.save(snapshot)
                }
                try Task.checkCancellation()
                guard let current = cloudMessage(messageID, in: conversationID)?.cloudReply else { return }
                if subscriptionAvailable && !current.stopRequested {
                    subscriptionAvailable = try await service.follow(current, token: credential) { page in
                        let terminal = !["queued", "running"].contains(page.state)
                        guard !page.events.isEmpty || terminal else { return }
                        self.credentialAccepted()
                        let checkpoint = terminal || lastCheckpoint.duration(to: .now) >= .milliseconds(250)
                        try self.applyCloudPage(page, messageID: messageID, conversationID: conversationID, checkpoint: checkpoint)
                        if checkpoint { lastCheckpoint = .now }
                    }
                    if subscriptionAvailable { continue }
                }
                let page = try await service.request(current, token: credential, method: "GET")!
                try Task.checkCancellation()
                credentialAccepted()
                try applyCloudPage(page, messageID: messageID, conversationID: conversationID)
                if cloudMessage(messageID, in: conversationID)?.state != .streaming { return }
                if (page.events.last?.seq ?? current.cursor) < page.last { continue }
                try await Task.sleep(for: .milliseconds(750))
            } catch {
                if Task.isCancelled { return }
                let cocoa = error as NSError
                let storageFailure = cocoa.domain == NSCocoaErrorDomain && (512...1023).contains(cocoa.code)
                if let http = error as? CloudReplyHTTPError, http.status == 401 || http.status == 403 { authorizationExpired = true }
                if error is URLError || storageFailure || (error as? CloudReplyHTTPError)?.retryable == true {
                    let notice = storageFailure ? L10n.tr("本机保存失败，请检查可用空间；云端回复仍可恢复。") : ((error as? CloudReplyHTTPError)?.notice ?? (job.requestBody == nil ? L10n.tr("连接暂时中断，云端仍在继续，联网后自动同步。") : L10n.tr("正在确认云端是否已接收，联网后自动恢复。")))
                    cloudNotice(notice, messageID: messageID, conversationID: conversationID)
                    do { try await Task.sleep(for: .seconds(3)) } catch { return }
                    continue
                }
                let description: String
                if let http = error as? CloudReplyHTTPError {
                    description = http.status == 410 ? L10n.tr("云端回复保存期已过（7 天），已保留本机内容。") : L10n.tr("无法恢复云端回复（\(http.status)），已保留内容。请检查服务版本或重新生成。")
                } else { description = ChatService.failureDescription(error) }
                update(conversationID) { chat in
                    if let i = chat.messages.firstIndex(where: { $0.id == messageID }) { chat.messages[i].finishReply(.failed, failure: description) }
                }
                persist(); return
            }
        }
    }
    /// Save the cursor, text, tool results and file references together. A restart can never
    /// observe appended text with an old cursor and append that same text a second time.
    func applyCloudPage(_ page: CloudReplyPage, messageID: UUID, conversationID: UUID, checkpoint: Bool = true) throws {
        guard let message = cloudMessage(messageID, in: conversationID), message.state == .streaming, let job = message.cloudReply else { return }
        let before = conversations, oldLibrary = library
        applyingCloudEvent = true
        defer { applyingCloudEvent = false }
        do {
            var cursor = job.cursor
            for item in page.events {
                if item.seq <= cursor { continue }
                guard item.seq == cursor + 1 else { throw LocalFailure.message(L10n.tr("云端回复顺序无效。")) }
                if let event = try SSEDecoder.decode(item.data) {
                    switch event {
                    case .delta(let delta): receive(delta, messageID: messageID, conversationID: conversationID)
                    case .search(let run): recordSearch(run, messageID: messageID, conversationID: conversationID)
                    case .recall(let run): recordRecall(run, messageID: messageID, conversationID: conversationID)
                    case .execution(let run): try recordCodeExecution(run, messageID: messageID, conversationID: conversationID)
                    case .done: break
                    }
                }
                cursor = item.seq
            }
            update(conversationID) { chat in
                guard let i = chat.messages.firstIndex(where: { $0.id == messageID }) else { return }
                chat.messages[i].cloudReply?.cursor = cursor
                chat.messages[i].cloudReply?.notice = job.stopRequested ? L10n.tr("正在请求云端停止…") : nil
                if cursor == page.last {
                    switch page.state {
                    case "complete":
                        if chat.messages[i].text.isEmpty { chat.messages[i].finishReply(.failed, failure: L10n.tr("模型未返回答复，已保留过程内容。")) }
                        else { chat.messages[i].finishReply(.complete) }
                    case "failed": chat.messages[i].finishReply(.failed, failure: page.failure ?? L10n.tr("云端生成失败，已保留收到的内容。"))
                    case "stopped": chat.messages[i].finishReply(.stopped)
                    default: break
                    }
                    if chat.messages[i].state != .streaming { chat.messages[i].cloudReply?.requestBody = nil; chat.messages[i].cloudReply?.notice = nil }
                }
            }
            if checkpoint { try storage.save(snapshot) }
        } catch {
            conversations = before; library = oldLibrary
            throw error
        }
    }
}

extension WorkspaceStore {
    /// After the first exchange, ask the model for a short title. Failures keep the derived title.
    func generateTitleIfNeeded(conversationID id: UUID, configuration: ConnectionSettings, credential: String) {
        guard !AppEnvironment.isAutomatedTest, !configuration.demo,
              let chat = conversations.first(where: { $0.id == id }), chat.automaticTitle == true,
              chat.messages.filter({ $0.role == "user" }).count == 1,
              let user = chat.messages.first(where: { $0.role == "user" }),
              let reply = chat.messages.last(where: { $0.role == "assistant" && $0.displayState == .complete && !$0.displayText.isEmpty }) else { return }
        let model = chat.modelChoice?.model ?? configuration.model
        let session = streamConfiguration
        Task { [weak self] in
            guard let request = try? ChatService.titleRequest(settings: configuration, token: credential, model: model, userText: user.displayText, replyText: reply.displayText) else { return }
            var text = ""
            do { for try await chunk in ChatService.stream(request: request, configuration: session) { text += chunk; if text.count > 80 { break } } }
            catch {} // A cut-off title is still usable; nothing usable keeps the derived one.
            guard let self, let title = ConversationTitle.sanitizeGenerated(text),
                  let i = self.conversations.firstIndex(where: { $0.id == id }), self.conversations[i].automaticTitle == true else { return }
            self.conversations[i].title = title
            self.conversations[i].automaticTitle = false
            self.scheduleSave()
        }
    }
}
