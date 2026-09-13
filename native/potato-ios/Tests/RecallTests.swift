import XCTest
@testable import PotatoMobile

final class RecallTests: XCTestCase {
    func testPayloadExcludesAttachmentsDraftsExamplesAndUnfinishedReplies() throws {
        var chat = Conversation(); chat.input = "尚未发送"; chat.title = "相机"; chat.draft = WorkingDraft()
        chat.messages = [ChatMessage(role: "user", text: "我选了 X100", attachments: [Attachment(name: "secret.txt", filename: "a", type: "text/plain", size: 10, extractedText: "不能上传的附件")]), ChatMessage(role: "assistant", text: "未完成", state: .streaming)]
        let raw = try RecallPayload(chat).content()
        XCTAssertTrue(raw.contains("我选了 X100")); XCTAssertFalse(raw.contains("不能上传")); XCTAssertFalse(raw.contains("尚未发送")); XCTAssertFalse(raw.contains("未完成")); XCTAssertFalse(raw.contains("secret.txt"))
        XCTAssertEqual(raw, try RecallPayload(chat).content())
        chat.recallExcluded = true; XCTAssertTrue(RecallPayload(chat).messages.isEmpty)
        chat.recallExcluded = false; chat.deletedAt = Date(); XCTAssertTrue(RecallPayload(chat).messages.isEmpty)
        chat.deletedAt = nil; chat.isExample = true; XCTAssertTrue(RecallPayload(chat).messages.isEmpty)
    }
    func testSelectedReplyVersionAndSourceRoundTrip() throws {
        let version = ReplyVersion(text: "旧版选择", state: .complete, failure: nil)
        var message = ChatMessage(role: "assistant", text: "新版选择"); message.versions = [version]; message.selectedVersionID = version.id
        var chat = Conversation(); chat.messages = [message]
        let wire = RecallPayload(chat).messages[0]; XCTAssertEqual(wire.text, version.text); XCTAssertEqual(wire.version, version.id.uuidString.lowercased())
        let source = RecallSource(id: message.id.uuidString, role: "assistant", text: version.text, date: "2026-09-12", version: version.id.uuidString, conversation: chat.id.uuidString, title: chat.title, revision: String(repeating: "a", count: 64))
        let run = RecallRun(id: "sources", state: "complete", sources: [source])
        message.recalls = [run]
        XCTAssertEqual(try JSONDecoder().decode(ChatMessage.self, from: JSONEncoder().encode(message)).recalls, [run])
        let eventData = try JSONSerialization.data(withJSONObject: ["potato_recall": JSONSerialization.jsonObject(with: JSONEncoder().encode(run))])
        guard case .recall(let decoded) = try SSEDecoder.decode(String(decoding: eventData, as: UTF8.self)) else { return XCTFail("Missing recall event") }
        XCTAssertEqual(decoded, run)
    }
    func testCredentialsRemainBoundToCapturedEndpoint() throws {
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/v1/chat/completions"
        let service = RecallService(settings: settings, token: "captured-token")
        settings.endpoint = "https://other.invalid/v1/chat/completions"
        let request = try service.request("sync", body: ["content": "example", "base": NSNull()])
        XCTAssertEqual(request.url?.absoluteString, "https://fixture.invalid/v1/recall/sync")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer captured-token")
        XCTAssertEqual(request.httpMethod, "POST")
    }
    @MainActor func testExcludingSourceRemovesCachedProvenanceAndMemories() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = WorkspaceStore(storage: LocalStorage(root: root), resetForTesting: true)
        var original = Conversation(); original.messages = [ChatMessage(role: "user", text: "偏好轻便")]
        let source = RecallSource(id: original.messages[0].id.uuidString.lowercased(), role: "user", text: "偏好轻便", date: "2026-09-12", conversation: original.id.uuidString.lowercased(), title: "偏好", revision: "x")
        var answer = Conversation(); var message = ChatMessage(role: "assistant", text: "记得你偏好轻便")
        message.recalls = [RecallRun(id: "sources", state: "complete", sources: [source])]; answer.messages = [message]
        store.conversations = [original, answer]; store.selectedID = answer.id
        store.memories = [PersonalMemory(id: UUID().uuidString, text: "轻便", revision: "x", updated: "now", sources: [source])]
        store.setRecallExcluded(true, conversation: original.id)
        XCTAssertTrue(store.memories.isEmpty); XCTAssertTrue(store.selected.messages[0].recalls![0].sources.isEmpty)
        store.openRecallSource(source); XCTAssertNotNil(store.error); XCTAssertEqual(store.selectedID, answer.id)
    }
}

private final class RecallFixtureProtocol: URLProtocol {
    static var entries: [String: [String: Any]] = [:]
    static var writes = 0
    static var token = "fixture-token"
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "recall.fixture.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var status = 200, result: [String: Any] = [:]
        if request.value(forHTTPHeaderField: "Authorization") != "Bearer \(Self.token)" { status = 401 }
        else if request.url!.path.hasSuffix("/status") { result = ["scope": "fixture-account", "entries": Self.entries, "memories": []] }
        else {
            var data = request.httpBody ?? Data()
            if let stream = request.httpBodyStream { stream.open(); defer { stream.close() }; var bytes = [UInt8](repeating: 0, count: 4096); while stream.hasBytesAvailable { let size = stream.read(&bytes, maxLength: bytes.count); if size <= 0 { break }; data.append(contentsOf: bytes.prefix(size)) } }
            if let body = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], let content = body["content"] as? String,
               let chat = (try? JSONSerialization.jsonObject(with: Data(content.utf8))) as? [String: Any], let id = chat["id"] as? String {
                let current = Self.entries[id]?["revision"] as? String
                if current != body["base"] as? String { status = 409 }
                else { let revision = RecallService.digest(content); Self.entries[id] = ["revision": revision, "excluded": chat["excluded"] ?? false]; Self.writes += 1; result = ["revision": revision] }
            } else { status = 400 }
        }
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: (try? JSONSerialization.data(withJSONObject: result)) ?? Data())
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

extension RecallTests {
    func testIncrementalSyncSurvivesRestartAndRefusesConflicts() async throws {
        RecallFixtureProtocol.entries = [:]; RecallFixtureProtocol.writes = 0
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString), storage = LocalStorage(root: root)
        defer { try? FileManager.default.removeItem(at: root) }
        var settings = ConnectionSettings(); settings.endpoint = "https://recall.fixture.invalid/v1/chat/completions"
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [RecallFixtureProtocol.self]
        let service = RecallService(settings: settings, token: "fixture-token", configuration: config)
        var chat = Conversation(); chat.messages = [ChatMessage(role: "user", text: "我选了 X100")]
        _ = try await service.sync([chat], storage: storage); XCTAssertEqual(RecallFixtureProtocol.writes, 1)
        _ = try await service.sync([chat], storage: storage); XCTAssertEqual(RecallFixtureProtocol.writes, 1)
        chat.messages.append(ChatMessage(role: "assistant", text: "已了解"))
        let restarted = RecallService(settings: settings, token: "fixture-token", configuration: config)
        _ = try await restarted.sync([chat], storage: storage); XCTAssertEqual(RecallFixtureProtocol.writes, 2)
        RecallFixtureProtocol.entries[chat.id.uuidString.lowercased()]?["revision"] = "other-device-revision"
        chat.title = "本机修改"
        do { _ = try await restarted.sync([chat], storage: storage); XCTFail("Must not overwrite remote edits") } catch { XCTAssertTrue(error.localizedDescription.contains("云端记录已改变")) }
        XCTAssertEqual(RecallFixtureProtocol.writes, 2)
    }
}
