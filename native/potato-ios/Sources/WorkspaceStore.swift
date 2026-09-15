import SwiftUI

@MainActor
final class WorkspaceStore: ObservableObject {
    @Published var conversations: [Conversation]
    @Published var selectedID: UUID
    @Published var settings: ConnectionSettings
    @Published var error: String?
    @Published var generatingID: UUID?
    @Published var recallNotice: String?
    @Published var memories: [PersonalMemory] = []
    private var memoryConnection: String?
    var visibleMemories: [PersonalMemory] { memoryConnection == RecallService.digest((settings.serviceIdentity ?? settings.endpoint) + connectionToken) ? memories : [] }
    @Published var recallFocusID: UUID?
    @Published var recallSyncing = false
    private var recallSyncTask: Task<RecallStatus, Error>?
    let storage: LocalStorage
    private let streamConfiguration: URLSessionConfiguration
    private let tokenProvider: (() -> String)?
    private var task: Task<Void, Never>?
    private var modelFetch: (id: UUID, endpoint: String?, token: String, task: Task<Void, Error>)?
    private var saveTask: Task<Void, Never>?
    private var loadFailed = false
    var demoDelay: Duration = .milliseconds(16)
    var selected: Conversation { conversations.first { $0.id == selectedID } ?? conversations[0] }
    var isGenerating: Bool { generatingID == selectedID }
    var visibleConversations: [Conversation] { conversations.filter { $0.deletedAt == nil }.sorted { a, b in a.pinned == b.pinned ? a.updatedAt > b.updatedAt : a.pinned } }

    init(storage: LocalStorage = LocalStorage(), resetForTesting: Bool = false, streamConfiguration: URLSessionConfiguration = .ephemeral, tokenProvider: (() -> String)? = nil) {
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
                    conversations[i].messages[j].recoverInterruptedReply()
                    conversations[i].messages[j].searches = conversations[i].messages[j].searches?.map { var run = $0; if run.state == "searching" { run.state = "stopped" }; return run }
                }
            }
        } else {
            let example = Conversation.example()
            conversations = [example]; selectedID = example.id; settings = ConnectionSettings()
        }
        if loadError != nil { loadFailed = true; error = "本地记录读取失败，原文件已保留。为避免覆盖，暂未保存新修改。" }
        else {
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
    var snapshot: SavedWorkspace { SavedWorkspace(conversations: conversations, selectedID: selectedID, settings: settings) }
    func persist() {
        guard !loadFailed else { return }
        do { try storage.save(snapshot) }
        catch { self.error = "保存失败：\(error.localizedDescription)。请保留应用并检查设备可用空间。" }
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
    func restore(_ id: UUID) { update(id) { $0.deletedAt = nil }; persist(); syncRecallInBackground() }
    func stop() {
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
        guard generatingID == nil else { error = "另一段对话仍在生成，请先停止或等待完成。"; return }
        let text = selected.input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !selected.pendingAttachments.isEmpty else { return }
        let choice: LocalModelChoice?
        do { choice = try validatedLocalChoice() } catch { self.error = error.localizedDescription; return }
        let message = ChatMessage(role: "user", text: text, attachments: selected.pendingAttachments)
        update { chat in
            if let choice { chat.modelChoice = choice }
            chat.messages.append(message)
            if chat.title == "新对话" { chat.title = String((text.isEmpty ? "附件对话" : text).prefix(28)) }
            chat.input = ""; chat.pendingAttachments = []
        }
        generate(choice: choice)
    }
    @discardableResult
    func retry(messageID: UUID? = nil, using override: LocalModelChoice? = nil) -> Bool {
        guard generatingID == nil, let old = selected.messages.last, old.role == "assistant",
              messageID == nil || messageID == old.id else { return false }
        let choice: LocalModelChoice?
        do {
            choice = settings.demo ? nil : (override ?? old.displayModelChoice ?? localModelChoice)
            try choice?.validate(settings: settings)
        } catch { self.error = error.localizedDescription; return false }
        var versions = old.versions ?? []
        versions.append(ReplyVersion(modelChoice: old.modelChoice, reasoning: old.reasoning, recalls: old.recalls, codeRuns: old.codeRuns, searches: old.searches, execution: old.execution, attachments: old.attachments, text: old.text, state: old.state, failure: old.failure, date: old.createdAt))
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
            let catalog = try await LocalModelService.catalog(settings: configuration, token: token, configuration: self.streamConfiguration)
            try Task.checkCancellation()
            guard configuration.serviceIdentity == self.settings.serviceIdentity, token == self.connectionToken else { throw LocalFailure.message("连接已改变，请重新读取模型列表。") }
            self.installLocalModelCatalog(catalog); self.persist()
        }
        modelFetch = (id, configuration.serviceIdentity, token, request)
        try await request.value
    }
    func refreshLocalModelsInBackground() async {
        guard !settings.demo, settings.serviceIdentity != nil else { return }
        // Offline startup keeps the saved catalog without interrupting the conversation.
        try? await reloadLocalModels(force: false)
    }
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
        branch.id = UUID(); branch.title += " · 编辑副本"; branch.isExample = false
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
        guard selected.pendingAttachments.count < 4 else { error = "每条消息最多添加 4 个附件。"; return }
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
            branch.id = UUID(); branch.title += " · 回复分支"; branch.isExample = false
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
                draft.replaceContent(message.displayText, reason: "替换文稿前")
                chat.draft = draft
            } else {
                var draft = WorkingDraft(); draft.customMarkdown = message.displayText
                draft.title = String(message.displayText.components(separatedBy: .newlines).first?.trimmingCharacters(in: CharacterSet(charactersIn: "# ")).prefix(100) ?? "工作文稿")
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
                    let answer = "这是本地体验模式，尚未调用模型。你的消息和附件已保存在这台 iPhone。连接模型后，可以在这里获得真实回复；也可以先编辑、勾选和分享示例文稿。"
                    for chunk in answer.map(String.init) {
                        try await Task.sleep(for: self.demoDelay)
                        try Task.checkCancellation()
                        self.append(chunk, to: reply.id, in: id)
                    }
                } else {
                    if configuration.recallEnabled == true && self.conversations.first(where: { $0.id == id })?.recallExcluded != true {
                        try await self.synchronizeRecall()
                        try Task.checkCancellation()
                        guard configuration == self.settings, credential == self.connectionToken else { throw LocalFailure.message("连接或设置已改变，请重新发送。") }
                    }
                    var request = try ChatService.request(settings: configuration, token: credential, messages: history, storage: storage, draft: workingDraft, choice: choice)
                    if configuration.recallEnabled == true && self.conversations.first(where: { $0.id == id })?.recallExcluded != true {
                        var body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
                        body["recall"] = ["enabled": true, "auto_memory": configuration.automaticMemory == true, "timezone": TimeZone.current.identifier]
                        request.httpBody = try JSONSerialization.data(withJSONObject: body)
                    }
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
                }
                try Task.checkCancellation()
                self.update(id) { chat in
                    if let i = chat.messages.firstIndex(where: { $0.id == reply.id }) {
                        if chat.messages[i].text.isEmpty {
                            let description = chat.messages[i].reasoning == nil ? "模型没有返回文字，请重新生成。" : "模型仅返回思考内容，未返回答复。已保留思考内容，可以重新生成。"
                            chat.messages[i].finishReply(.failed, failure: description)
                        } else { chat.messages[i].finishReply(.complete) }
                    }
                }
            } catch {
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
            recallNotice = "已同步 \(result.entries.values.filter { !$0.excluded }.count) 段对话 · \(memories.count) 条记忆"
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
            chat.messages[i].recalls = runs
        }
    }
    func openRecallSource(_ source: RecallSource) {
        guard let id = UUID(uuidString: source.conversation), let messageID = UUID(uuidString: source.id),
              let i = conversations.firstIndex(where: { $0.id == id && $0.deletedAt == nil && $0.recallExcluded != true }),
              let j = conversations[i].messages.firstIndex(where: { $0.id == messageID }) else { error = "原对话不在此设备，或已删除、排除。"; return }
        let message = conversations[i].messages[j]
        guard message.selectedVersionID?.uuidString.lowercased() == source.version else { error = "原对话已切换回复版本，请重新检索。"; return }
        let original = source.version.flatMap { version in message.versions?.first(where: { $0.id.uuidString.lowercased() == version })?.text } ?? message.text
        guard original.hasPrefix(source.text) else { error = "原消息内容已改变，请重新检索。"; return }
        selectedID = id; recallFocusID = nil
        Task { await Task.yield(); recallFocusID = messageID }
    }
}
