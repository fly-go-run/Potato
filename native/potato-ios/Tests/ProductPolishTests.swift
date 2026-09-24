import XCTest
@testable import PotatoMobile

@MainActor
final class ProductPolishTests: XCTestCase {
    private func storage() -> LocalStorage {
        LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
    }

    func testModelDisplayNames() {
        XCTAssertEqual(ModelNaming.displayName(id: "deepseek-v4.1-flash-expires-on-0910", name: "deepseek-v4.1-flash-expires-on-0910"), "DeepSeek V4.1 Flash")
        XCTAssertEqual(ModelNaming.displayName(id: "gpt-5.6-luna", name: "gpt-5.6-luna"), "GPT-5.6 Luna")
        XCTAssertEqual(ModelNaming.displayName(id: "openrouter/deepseek-v4-pro", name: ""), "DeepSeek V4 Pro")
        // A name chosen by the service wins; only the expiry suffix is removed.
        XCTAssertEqual(ModelNaming.displayName(id: "fixture", name: "Fixture Model"), "Fixture Model")
        XCTAssertEqual(ModelNaming.displayName(id: "x", name: "Flash expires-on-0910"), "Flash")
    }

    func testModelExpiryUsesTheEncodedDate() {
        var calendar = Calendar(identifier: .gregorian); calendar.timeZone = TimeZone(identifier: "Asia/Shanghai")!
        let september24 = calendar.date(from: DateComponents(year: 2026, month: 9, day: 24))!
        XCTAssertTrue(ModelNaming.isExpired("deepseek-v4.1-flash-expires-on-0910", now: september24, calendar: calendar))
        XCTAssertFalse(ModelNaming.isExpired("deepseek-v4.1-flash-expires-on-1010", now: september24, calendar: calendar))
        XCTAssertFalse(ModelNaming.isExpired("deepseek-v4.1-flash", now: september24, calendar: calendar))
        // The expiry day itself is still usable.
        let september10 = calendar.date(from: DateComponents(year: 2026, month: 9, day: 10, hour: 20))!
        XCTAssertFalse(ModelNaming.isExpired("m-expires-on-0910", now: september10, calendar: calendar))
        // December read in January belongs to the previous year.
        let january5 = calendar.date(from: DateComponents(year: 2027, month: 1, day: 5))!
        XCTAssertTrue(ModelNaming.isExpired("m-expires-on-1231", now: january5, calendar: calendar))
    }

    func testTitlesEndOnWordsAndStripMarkdown() {
        XCTAssertEqual(ConversationTitle.make(from: "## 帮我整理今天的工作计划。\n第二行"), "帮我整理今天的工作计划")
        let latin = ConversationTitle.make(from: "Please look up the current official Exa search documentation for me")
        XCTAssertTrue(latin.hasSuffix("…"))
        XCTAssertFalse(latin.dropLast().hasSuffix(" "))
        XCTAssertTrue("Please look up the current official Exa search documentation".hasPrefix(String(latin.dropLast())))
        XCTAssertGreaterThan(latin.count, 30, "Latin text gets roughly twice the characters of CJK text")
        XCTAssertEqual(ConversationTitle.make(from: "一二三四五六七八九十一二三四五六七八九十一二三四五"), "一二三四五六七八九十一二三四五六七八九十一二…")
        XCTAssertEqual(ConversationTitle.sanitizeGenerated("标题：「周末轻松计划」。\n多余"), "周末轻松计划")
        XCTAssertNil(ConversationTitle.sanitizeGenerated("  “”  "))
    }

    func testDisplayTitleForLegacyPlaceholdersAndDrafts() {
        var chat = Conversation()
        XCTAssertEqual(chat.displayTitle, "新对话")
        chat.input = "明天要带的东西"
        XCTAssertTrue(chat.displayTitle.contains("明天要带的东西"))
        chat.messages = [ChatMessage(role: "user", text: "写一封请假邮件"), ChatMessage(role: "assistant", text: "好的")]
        XCTAssertEqual(chat.displayTitle, "写一封请假邮件")
        chat.title = "我的标题"
        XCTAssertEqual(chat.displayTitle, "我的标题")
    }

    func testConversationPeriods() {
        var calendar = Calendar(identifier: .gregorian); calendar.timeZone = TimeZone(identifier: "Asia/Shanghai")!
        let now = calendar.date(from: DateComponents(year: 2026, month: 9, day: 24, hour: 9))!
        func day(_ offset: Int) -> Date { calendar.date(byAdding: .day, value: -offset, to: now)! }
        XCTAssertEqual(ConversationPeriod.of(day(0), now: now, calendar: calendar), .today)
        XCTAssertEqual(ConversationPeriod.of(day(1), now: now, calendar: calendar), .yesterday)
        XCTAssertEqual(ConversationPeriod.of(day(5), now: now, calendar: calendar), .week)
        XCTAssertEqual(ConversationPeriod.of(day(20), now: now, calendar: calendar), .month)
        XCTAssertEqual(ConversationPeriod.of(day(90), now: now, calendar: calendar), .older)
    }

    func testCustomInstructionsAreAppendedAndNotSentWithProbes() throws {
        var settings = ConnectionSettings(); settings.demo = false
        settings.endpoint = "https://polish.invalid/v1/chat/completions"; settings.model = "fixture"
        XCTAssertEqual(settings.requestInstructions, ConnectionSettings.defaultSystemPrompt)
        settings.customInstructions = "我是产品经理，回答请简短。"
        let request = try ChatService.request(settings: settings, token: "", messages: [ChatMessage(role: "user", text: "hi")], storage: storage())
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(request.httpBody)) as? [String: Any])
        let system = try XCTUnwrap((body["messages"] as? [[String: Any]])?.first?["content"] as? String)
        XCTAssertTrue(system.hasPrefix(ConnectionSettings.defaultSystemPrompt))
        XCTAssertTrue(system.contains("我是产品经理"))
    }

    func testProductionFirstRunStartsEmptyWithoutSample() {
        let store = WorkspaceStore(storage: storage(), productionDefaults: true)
        XCTAssertEqual(store.conversations.count, 1)
        XCTAssertTrue(store.selected.isEmptyShell)
        XCTAssertFalse(store.selected.isExample)
        // Tests keep the sample conversation by default.
        XCTAssertTrue(WorkspaceStore(storage: storage()).selected.isExample)
    }

    func testProductionRetryRequiresSignInAndPreservesExistingReply() throws {
        let disk = storage()
        var sample = Conversation.example(); sample.input = "保留未发送草稿"
        try disk.save(SavedWorkspace(conversations: [sample], selectedID: sample.id))
        let store = WorkspaceStore(storage: disk, productionDefaults: true)
        XCTAssertTrue(store.requiresSignIn)
        XCTAssertFalse(store.retry(messageID: sample.messages.last!.id))
        XCTAssertTrue(store.signInRequested)
        XCTAssertNil(store.generatingID)
        XCTAssertEqual(store.selected.messages, sample.messages)
        XCTAssertEqual(store.selected.input, sample.input)
        XCTAssertEqual(try disk.load()?.conversations.first?.messages, sample.messages)
    }

    func testTitleRequestUsesValidThinkingParametersAndOnlyTitleInstructions() throws {
        var settings = ConnectionSettings(); settings.demo = false
        settings.endpoint = "https://polish.invalid/v1/chat/completions"; settings.model = "fixture"
        settings.customInstructions = "PRIVATE_CUSTOM_INSTRUCTIONS"
        settings.systemPrompt = "ORIGINAL_SYSTEM_PROMPT"
        let cases: [(modes: [String], efforts: [String], mode: String?, effort: String?)] = [
            (["enabled", "disabled"], ["low", "high", "max"], "disabled", nil),
            (["enabled", "disabled"], ["none", "low"], "disabled", nil),
            ([], ["none", "low", "high"], nil, "none"),
            ([], ["low"], nil, "low"),
            ([], [], nil, nil)
        ]
        for item in cases {
            settings.modelCatalog = LocalModelCatalog(endpoint: settings.serviceIdentity!, models: [LocalModelEntry(id: "fixture", name: "Fixture", reasoning_effort_options: item.efforts, thinking_modes: item.modes)])
            let request = try ChatService.titleRequest(settings: settings, token: "synthetic", model: "fixture", userText: "写一份计划", replyText: "计划内容")
            let body = try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(request.httpBody)) as? [String: Any])
            XCTAssertEqual((body["thinking"] as? [String: String])?["type"], item.mode)
            XCTAssertEqual(body["reasoning_effort"] as? String, item.effort)
            XCTAssertEqual(body["max_tokens"] as? Int, 24)
            let messages = try XCTUnwrap(body["messages"] as? [[String: String]])
            XCTAssertTrue(messages[0]["content"]!.contains("标题"))
            XCTAssertFalse(messages[0]["content"]!.contains("PRIVATE_CUSTOM_INSTRUCTIONS"))
            XCTAssertFalse(messages[0]["content"]!.contains("ORIGINAL_SYSTEM_PROMPT"))
            XCTAssertTrue(messages[1]["content"]!.contains("计划内容"))
        }
    }

    func testProductionLaunchCleansLegacyData() throws {
        let disk = storage()
        let empty = Conversation(), draft: Conversation = { var value = Conversation(); value.input = "未发送"; return value }()
        var legacy = Conversation()
        let long = "Please look up the current official Exa search documentation for me"
        legacy.messages = [ChatMessage(role: "user", text: long), ChatMessage(role: "assistant", text: "Done")]
        legacy.title = String(long.prefix(28))
        var current = Conversation(); current.messages = [ChatMessage(role: "user", text: "当前")]
        var settings = ConnectionSettings(); settings.demo = false; settings.endpoint = "https://polish.invalid/v1/chat/completions"; settings.model = "m"
        settings.systemPrompt = "自己写过的回复偏好"
        try disk.save(SavedWorkspace(conversations: [empty, draft, legacy, current], selectedID: current.id, settings: settings))

        let store = WorkspaceStore(storage: disk, productionDefaults: true)
        XCTAssertFalse(store.conversations.contains { $0.id == empty.id }, "empty leftovers are removed")
        XCTAssertTrue(store.conversations.contains { $0.id == draft.id }, "unsent drafts are kept")
        let migrated = try XCTUnwrap(store.conversations.first { $0.id == legacy.id })
        XCTAssertNotEqual(migrated.title, String(long.prefix(28)))
        XCTAssertTrue(migrated.title.hasSuffix("…"))
        XCTAssertEqual(store.settings.customInstructions, "自己写过的回复偏好")
        XCTAssertEqual(store.settings.systemPrompt, ConnectionSettings.defaultSystemPrompt)
        XCTAssertEqual(store.settings.developerMode, true, "a manual connection keeps its developer options")
    }

    func testRenameStopsAutomaticTitles() {
        let store = WorkspaceStore(storage: storage())
        store.update { $0.automaticTitle = true }
        store.rename(store.selectedID, to: "  自己起的名字  ")
        XCTAssertEqual(store.selected.title, "自己起的名字")
        XCTAssertEqual(store.selected.automaticTitle, false)
        store.rename(store.selectedID, to: "   ")
        XCTAssertEqual(store.selected.title, "自己起的名字")
    }

    func testFileTitlesAndKinds() {
        XCTAssertEqual(FileKind.readableTitle("llm_inference-overview.pptx"), "LLM inference overview")
        XCTAssertEqual(FileKind.readableTitle("report.pdf"), "Report")
        XCTAssertEqual(FileKind(filename: "a.xlsx").symbol, "tablecells")
        XCTAssertEqual(FileKind(filename: "a.pdf").symbol, "doc.richtext")
        XCTAssertEqual(FileKind(filename: "photo", type: "image/jpeg").symbol, "photo")
    }
}
