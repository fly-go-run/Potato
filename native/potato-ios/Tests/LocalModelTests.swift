import XCTest
@testable import PotatoMobile

final class LocalModelFixture: URLProtocol {
    static let models = #"{"data":[{"id":"known","name":"Known","reasoning_effort_options":["low","high"],"thinking_modes":["enabled","disabled"]},{"id":"deepseek-unknown","name":"Unknown"}]}"#
    private static let lock = NSLock()
    private static var requests: [URLRequest] = []
    private static var expectation: XCTestExpectation?
    static func reset(_ started: XCTestExpectation? = nil) { lock.lock(); defer { lock.unlock() }; requests = []; expectation = started }
    static var captured: [URLRequest] { lock.lock(); defer { lock.unlock() }; return requests }
    private var work: Task<Void, Never>?
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "model-tests.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var saved = request
        if let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; data.append(contentsOf: buffer.prefix(count)) }
            saved.httpBody = data
        }
        Self.lock.lock(); Self.requests.append(saved); let expectation = Self.expectation; Self.expectation = nil; Self.lock.unlock(); expectation?.fulfill()
        let path = request.url!.path
        let get = request.httpMethod == "GET"
        let body = (saved.httpBody.flatMap { try? JSONSerialization.jsonObject(with: $0) }) as? [String: Any] ?? [:]
        work = Task {
            if get { try? await Task.sleep(for: .milliseconds(150)) }
            guard !Task.isCancelled else { return }
            let status = path.contains("missing") ? 404 : 200
            let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": get ? "application/json" : "text/event-stream"])!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            let data: Data
            if get { data = Data((path.contains("large") ? String(repeating: "x", count: 1_000_001) : Self.models).utf8) }
            else {
                let content = "\(body["model"] as? String ?? "")|\((body["thinking"] as? [String: String])?["type"] ?? "default")|\(body["reasoning_effort"] as? String ?? "default")"
                let frame = try! JSONSerialization.data(withJSONObject: ["choices": [["delta": ["content": content]]]])
                data = Data("data: \(String(decoding: frame, as: UTF8.self))\n\ndata: [DONE]\n\n".utf8)
            }
            client?.urlProtocol(self, didLoad: data); client?.urlProtocolDidFinishLoading(self)
        }
    }
    override func stopLoading() { work?.cancel() }
}

@MainActor final class LocalModelTests: XCTestCase {
    private func settings(_ path: String = "/v1/chat/completions") -> ConnectionSettings {
        var value = ConnectionSettings(); value.demo = false; value.endpoint = "https://model-tests.invalid" + path; value.model = "known"; return value
    }
    private var config: URLSessionConfiguration { let value = URLSessionConfiguration.ephemeral; value.protocolClasses = [LocalModelFixture.self]; return value }
    private func storage() -> LocalStorage { LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)) }
    private func store(token: @escaping () -> String = { "synthetic" }) throws -> WorkspaceStore {
        let value = WorkspaceStore(storage: storage(), streamConfiguration: config, tokenProvider: token)
        value.settings = settings(); value.settings.modelCatalog = try LocalModelService.decode(Data(LocalModelFixture.models.utf8), settings: value.settings)
        value.newChat(); return value
    }
    private func finish(_ store: WorkspaceStore) async throws {
        for _ in 0..<100 { if store.generatingID == nil { return }; try await Task.sleep(for: .milliseconds(20)) }
        XCTFail("Generation did not finish")
    }
    func testCatalogUsesSameOriginAuthenticationAndPreservesUnknownCapabilities() async throws {
        LocalModelFixture.reset()
        let settings = settings()
        let value = try await LocalModelService.catalog(settings: settings, token: "catalog-token", configuration: config)
        let request = try XCTUnwrap(LocalModelFixture.captured.first)
        XCTAssertEqual(request.url?.absoluteString, "https://model-tests.invalid/v1/models")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer catalog-token")
        XCTAssertNil(request.httpBody); XCTAssertEqual(value.models.count, 2)
        XCTAssertEqual(value.models[0].efforts, ["low", "high"]); XCTAssertTrue(value.models[1].modes.isEmpty)
        XCTAssertEqual(try JSONDecoder().decode(LocalModelCatalog.self, from: JSONEncoder().encode(value)), value)
    }
    func testCatalogBoundsAndUnsupportedEndpointDoNotProduceInventedModels() async {
        for path in ["/missing/chat/completions", "/large/chat/completions"] {
            do { _ = try await LocalModelService.catalog(settings: settings(path), token: "", configuration: config); XCTFail("Invalid catalog must fail") } catch {}
        }
        XCTAssertThrowsError(try LocalModelService.catalogRequest(settings: settings("/custom-reply"), token: "private"))
        XCTAssertThrowsError(try LocalModelService.decode(Data(#"{"data":{}}"#.utf8), settings: settings()))
    }
    func testOfficialCapabilitiesUseExactOriginAndIDsAndRespectExplicitUnsupported() throws {
        let entry = LocalModelEntry(id: "deepseek-v4-pro", name: "Pro")
        XCTAssertEqual(entry.documented(for: URL(string: "https://api.deepseek.com/chat/completions")).efforts, ["low", "high", "max"])
        XCTAssertTrue(entry.documented(for: URL(string: "https://proxy.invalid/chat/completions")).modes.isEmpty)
        XCTAssertTrue(LocalModelEntry(id: "deepseek-v4-pro-private", name: "Private").documented(for: URL(string: "https://api.deepseek.com/chat/completions")).efforts.isEmpty)
        var blocked = entry; blocked.reasoning_effort_options = []; blocked.thinking_modes = []
        XCTAssertTrue(blocked.documented(for: URL(string: "https://api.deepseek.com/chat/completions")).modes.isEmpty)
        var budget = entry; budget.thinking_param_style = "budget"; budget.reasoning_effort_options = ["high"]
        XCTAssertTrue(budget.efforts.isEmpty)
    }
    func testWireParametersAndServiceDefaultCannotLeakBetweenModels() throws {
        var settings = settings(); settings.modelCatalog = try LocalModelService.decode(Data(LocalModelFixture.models.utf8), settings: settings)
        let choice = LocalModelChoice(endpoint: settings.serviceIdentity!, model: "known", thinkingMode: "enabled", reasoningEffort: "high")
        func body(_ choice: LocalModelChoice) throws -> [String: Any] { let request = try ChatService.request(settings: settings, token: "", messages: [ChatMessage(role: "user", text: "fixture")], storage: storage(), choice: choice); return try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any] }
        XCTAssertEqual(try body(choice)["reasoning_effort"] as? String, "high")
        var disabled = choice; disabled.thinkingMode = "disabled"; disabled.reasoningEffort = nil
        XCTAssertNil(try body(disabled)["reasoning_effort"])
        let unknown = LocalModelChoice(endpoint: settings.serviceIdentity!, model: "deepseek-unknown")
        XCTAssertNil(try body(unknown)["thinking"]); XCTAssertNil(try body(unknown)["reasoning_effort"])
        var stale = unknown; stale.reasoningEffort = "high"; XCTAssertThrowsError(try body(stale))
        disabled.reasoningEffort = "high"; XCTAssertThrowsError(try body(disabled))
    }
    func testInFlightRequestCapturesModelEndpointAndCredentialTogether() async throws {
        LocalModelFixture.reset(); var token = "token-before"
        let value = try store(token: { token })
        let choice = LocalModelChoice(endpoint: value.settings.serviceIdentity!, model: "known", thinkingMode: "enabled", reasoningEffort: "low")
        try value.chooseLocalModel(choice); value.update { $0.input = "Synthetic race check" }; value.send()
        token = "token-after"; value.settings.endpoint = "https://another.invalid/v1/chat/completions"
        try await finish(value)
        let request = try XCTUnwrap(LocalModelFixture.captured.first)
        XCTAssertEqual(request.url?.host, "model-tests.invalid"); XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer token-before")
        XCTAssertEqual(value.selected.messages.last?.text, "known|enabled|low")
        XCTAssertEqual(value.selected.messages.last?.modelChoice, choice)
    }
    func testEndpointChangePreservesDraftAndAnswerUntilExplicitChoice() async throws {
        let value = try store(); value.update { $0.input = "First" }; value.send(); try await finish(value)
        let before = value.selected.messages
        value.settings.endpoint = "https://other.invalid/v1/chat/completions"; value.update { $0.input = "Keep this draft" }
        value.send(); XCTAssertEqual(value.selected.messages, before); XCTAssertEqual(value.selected.input, "Keep this draft")
        value.retry(); XCTAssertEqual(value.selected.messages, before); XCTAssertNotNil(value.error)
    }
    func testRetryKeepsDisplayedModelAndExplicitRegenerationPreservesComposer() async throws {
        let value = try store(); let firstID = value.selectedID
        let first = LocalModelChoice(endpoint: value.settings.serviceIdentity!, model: "known", thinkingMode: "enabled", reasoningEffort: "high")
        try value.chooseLocalModel(first); value.update { $0.input = "First" }; value.send(); try await finish(value)
        let other = LocalModelChoice(endpoint: value.settings.serviceIdentity!, model: "deepseek-unknown")
        try value.chooseLocalModel(other); value.retry(); try await finish(value)
        XCTAssertEqual(value.selected.messages.last?.modelChoice, first)
        XCTAssertEqual(value.selected.messages.last?.text, "known|enabled|high")
        try value.chooseLocalModel(first); value.update { $0.input = "Unsent draft" }
        value.retry(messageID: value.selected.messages.last!.id, using: other); try await finish(value)
        XCTAssertEqual(value.selected.messages.last?.modelChoice, other)
        XCTAssertEqual(value.selected.messages.last?.versions?.last?.modelChoice, first)
        XCTAssertEqual(value.localModelChoice, first)
        XCTAssertEqual(value.selected.input, "Unsent draft")
        value.chooseReplyVersion(value.selected.messages.last!.id, offset: -1); value.persist()
        let restored = WorkspaceStore(storage: value.storage)
        XCTAssertEqual(restored.selected.messages.last?.displayModelChoice, first)
        value.newChat(); XCTAssertNil(value.selected.modelChoice)
        value.select(firstID); XCTAssertEqual(value.selected.modelChoice, first)
        value.retry(); try await finish(value)
        XCTAssertEqual(value.selected.messages.last?.modelChoice, first)
    }
    func testRetryRejectsStaleMessageAndInvalidOverrideWithoutChangingHistory() async throws {
        let value = try store(); value.update { $0.input = "First" }; value.send(); try await finish(value)
        let original = value.selected.messages
        XCTAssertFalse(value.retry(messageID: UUID()))
        XCTAssertEqual(value.selected.messages, original)
        let invalid = LocalModelChoice(endpoint: "https://other.invalid", model: "known")
        XCTAssertFalse(value.retry(messageID: original.last!.id, using: invalid))
        XCTAssertEqual(value.selected.messages, original)
        XCTAssertNil(value.generatingID)
    }
    func testStaleCatalogResponseCannotAttachToChangedEndpointOrCredential() async throws {
        for credentialChange in [false, true] {
            var token = "before"; let value = try store(token: { token })
            let initial = value.settings.modelCatalog
            let started = expectation(description: "request started"); LocalModelFixture.reset(started)
            let request = Task { try await value.reloadLocalModels() }
            await fulfillment(of: [started], timeout: 3)
            if credentialChange { token = "after" } else { value.settings.endpoint = "https://changed.invalid/v1/chat/completions" }
            do { try await request.value; XCTFail("Stale catalog must not replace current settings") } catch {}
            XCTAssertEqual(value.settings.modelCatalog, initial)
        }
    }
    func testOldWorkspaceSettingsAndMessagesDecodeWithoutModelFields() throws {
        var settingsJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(settings())) as! [String: Any]
        settingsJSON.removeValue(forKey: "modelCatalog")
        XCTAssertNil(try JSONDecoder().decode(ConnectionSettings.self, from: JSONSerialization.data(withJSONObject: settingsJSON)).modelCatalog)
        let message = ChatMessage(role: "assistant", text: "Old reply")
        XCTAssertNil(try JSONDecoder().decode(ChatMessage.self, from: JSONEncoder().encode(message)).modelChoice)
    }
}
