import XCTest
@testable import PotatoMobile

@MainActor final class CloudAccountTests: XCTestCase {
    private var profile: RemoteAccountProfile { RemoteAccountProfile(owner: "cloud-test-" + UUID().uuidString, email: "fixture@example.test", relay: URL(string: "https://model-tests.invalid")!, scope: "cloud") }
    private var config: URLSessionConfiguration { let c = URLSessionConfiguration.ephemeral; c.protocolClasses = [LocalModelFixture.self]; return c }
    func testCloudConnectionLoadsCatalogPreservesDraftAndPersistsAccountWithoutCredential() async throws {
        let account = profile, root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try SecureToken.save("synthetic-cloud-secret", account: account.account)
        defer { try? SecureToken.save("", account: account.account); try? FileManager.default.removeItem(at: root) }
        let store = WorkspaceStore(storage: LocalStorage(root: root)); store.newChat(); store.update { $0.input = "保留草稿" }
        LocalModelFixture.reset()
        try await store.connectCloud(account, configuration: config)
        XCTAssertFalse(store.settings.demo); XCTAssertEqual(store.settings.model, "known")
        XCTAssertEqual(store.connectionToken, "synthetic-cloud-secret"); XCTAssertEqual(store.selected.input, "保留草稿")
        XCTAssertEqual(LocalModelFixture.captured.first?.url?.path, "/v1/models")
        XCTAssertEqual(LocalModelFixture.captured.first?.value(forHTTPHeaderField: "Authorization"), "Bearer synthetic-cloud-secret")
        let encoded = try JSONEncoder().encode(store.snapshot)
        XCTAssertFalse(String(decoding: encoded, as: UTF8.self).contains("synthetic-cloud-secret"))
        let restored = WorkspaceStore(storage: LocalStorage(root: root)); XCTAssertEqual(restored.settings.cloudAccount, account)
        XCTAssertEqual(restored.connectionToken, "synthetic-cloud-secret")
        try SecureToken.save("", account: account.account)
        XCTAssertEqual(restored.connectionToken, "")
    }
    func testCloudCredentialCannotFollowEditedEndpointOrFallbackAfterLogout() throws {
        let account = profile; try SecureToken.save("scoped-secret", account: account.account)
        defer { try? SecureToken.save("", account: account.account) }
        var s = ConnectionSettings(); s.cloudAccount = account; s.endpoint = account.cloudEndpoint!.absoluteString
        XCTAssertEqual(s.connectionToken, "scoped-secret")
        for endpoint in ["https://other.invalid/v1/chat/completions", "https://model-tests.invalid/other/chat/completions", "http://model-tests.invalid/v1/chat/completions"] {
            s.endpoint = endpoint; XCTAssertEqual(s.connectionToken, "")
        }
    }
    func testLogoutDuringCloudDiscoveryDoesNotInstallOldConnection() async throws {
        let account = profile; try SecureToken.save("temporary-session", account: account.account)
        defer { try? SecureToken.save("", account: account.account) }
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)))
        let previous = store.settings, started = expectation(description: "catalog started")
        LocalModelFixture.reset(started)
        let task = Task { try await store.connectCloud(account, configuration: config) }
        await fulfillment(of: [started], timeout: 2)
        try SecureToken.save("", account: account.account)
        do { try await task.value; XCTFail("Logged out session must not connect") } catch {}
        XCTAssertEqual(store.settings, previous)
    }
    func testLegacySettingsDecodeAndCloudDefaultModel() throws {
        let data = Data(#"{"demo":true,"endpoint":"","model":"","haptics":true,"systemPrompt":"test"}"#.utf8)
        XCTAssertNil(try JSONDecoder().decode(ConnectionSettings.self, from: data).cloudAccount)
        var s = ConnectionSettings(); s.endpoint = "https://model-tests.invalid/v1/chat/completions"
        let catalog = try LocalModelService.decode(Data(#"{"default_model":"sub2api/one","catalog_source":"configured","data":[{"id":"sub2api/one","name":"sub2api · One"}]}"#.utf8), settings: s)
        XCTAssertEqual(catalog.defaultModel, "sub2api/one"); XCTAssertEqual(catalog.models.first?.id, "sub2api/one")
    }
    func testNarrowedCloudCatalogMigratesFutureSelectionAndPreservesHistoryDraftAndAttachments() throws {
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)))
        store.settings.cloudAccount = profile
        store.settings.endpoint = "https://model-tests.invalid/v1/chat/completions"
        store.settings.model = "deepseek/deepseek-v4-pro"
        let endpoint = store.settings.serviceIdentity!
        let old = LocalModelChoice(endpoint: endpoint, model: "sub2api/gpt-5.5", reasoningEffort: "high")
        store.update { chat in
            chat.modelChoice = old; chat.input = "保留草稿"
            chat.messages = [ChatMessage(modelChoice: old, role: "assistant", text: "旧答案")]
        }
        let history = store.selected.messages, attachments = store.selected.pendingAttachments
        let catalog = LocalModelCatalog(endpoint: endpoint, models: [
            LocalModelEntry(id: "deepseek/deepseek-flash", name: "DeepSeek V4.1 Flash", reasoning_effort_options: ["low", "high", "max"], thinking_modes: ["enabled", "disabled"]),
            LocalModelEntry(id: "sub2api/gpt-5.6", name: "GPT-5.6", reasoning_effort_options: ["none", "low", "medium", "high", "xhigh", "max"], thinking_modes: [])
        ], defaultModel: "deepseek/deepseek-flash")
        store.installLocalModelCatalog(catalog)
        XCTAssertEqual(store.settings.model, "deepseek/deepseek-flash")
        XCTAssertEqual(store.localModelChoice.model, "sub2api/gpt-5.6")
        XCTAssertNil(store.localModelChoice.reasoningEffort)
        XCTAssertEqual(store.selected.messages, history)
        XCTAssertEqual(store.selected.input, "保留草稿")
        XCTAssertEqual(store.selected.pendingAttachments, attachments)
        XCTAssertThrowsError(try old.validate(settings: store.settings))
        let allowed = LocalModelChoice(endpoint: endpoint, model: "sub2api/gpt-5.6", reasoningEffort: "max")
        try store.chooseLocalModel(allowed); store.installLocalModelCatalog(catalog)
        XCTAssertEqual(store.localModelChoice, allowed)
        let invalid = LocalModelChoice(endpoint: endpoint, model: "deepseek/deepseek-flash", reasoningEffort: "medium")
        XCTAssertThrowsError(try invalid.validate(settings: store.settings))
    }
}
