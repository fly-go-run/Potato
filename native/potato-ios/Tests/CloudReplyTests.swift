import XCTest
@testable import PotatoMobile

final class CloudReplyFixtureProtocol: URLProtocol {
    struct State {
        var subscriptions: [Int] = []
        var stream = false
        var loseStream = false
        var puts = 0
        var gets: [Int] = []
        var cancels = 0
        var loseReceipt = false
        var loseRead = false
        var stopped = false
        var bodies: [Data] = []
    }
    static let lock = NSLock()
    static var jobs: [String: State] = [:]
    static func configure(_ id: String, _ state: State = State()) { lock.lock(); defer { lock.unlock() }; jobs[id] = state }
    static func state(_ id: String) -> State { lock.lock(); defer { lock.unlock() }; return jobs[id]! }
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "cloud-reply.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let url = request.url!, id = url.pathComponents[4]
        if url.path.hasSuffix("/events") {
            if Self.state(id).stream { subscribe(id, url: url); return }
            client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: 400, httpVersion: nil, headerFields: nil)!, cacheStoragePolicy: .notAllowed)
            client?.urlProtocolDidFinishLoading(self); return
        }
        Self.lock.lock()
        var state = Self.jobs[id] ?? State()
        var fail = false
        var body: [String: Any] = [:]
        if request.httpMethod == "PUT" {
            state.puts += 1
            if let data = request.httpBody { state.bodies.append(data) }
            else if let stream = request.httpBodyStream {
                stream.open(); defer { stream.close() }
                var data = Data(), buffer = [UInt8](repeating: 0, count: 1024)
                while stream.hasBytesAvailable { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(contentsOf: buffer.prefix(n)) }
                state.bodies.append(data)
            }
            fail = state.loseReceipt && state.puts == 1
            body = ["state": "queued"]
        } else if request.httpMethod == "POST" {
            state.cancels += 1; state.stopped = true; body = ["state": "stopped"]
        } else {
            let after = Int(URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems!.first!.value!)!
            state.gets.append(after)
            fail = state.loseRead && state.gets.count == 2
            let partial = state.loseRead && after == 0
            let stopCursor = state.subscriptions.isEmpty ? 0 : 1
            let events: [[String: Any]] = state.stopped ? (stopCursor > after ? [["seq": 1, "data": Self.delta("开头🌱")]] : []) : (partial ? [["seq": 1, "data": Self.delta("开头🌱")]] : [["seq": 1, "data": Self.delta("开头🌱")], ["seq": 2, "data": Self.delta("完整结尾")]]).filter { ($0["seq"] as! Int) > after }
            body = ["state": state.stopped ? "stopped" : partial ? "running" : "complete", "last": state.stopped ? stopCursor : partial ? 1 : 2, "events": events]
        }
        Self.jobs[id] = state; Self.lock.unlock()
        if fail { client?.urlProtocol(self, didFailWithError: URLError(.networkConnectionLost)); return }
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: body))
        client?.urlProtocolDidFinishLoading(self)
    }
    private var pendingDelivery: DispatchWorkItem?
    private func subscribe(_ id: String, url: URL) {
        let after = Int(URLComponents(url: url, resolvingAgainstBaseURL: false)!.queryItems!.first!.value!)!
        Self.lock.lock(); var state = Self.jobs[id]!; state.subscriptions.append(after); Self.jobs[id] = state; Self.lock.unlock()
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "text/event-stream"])!, cacheStoragePolicy: .notAllowed)
        func send(_ state: String, seq: Int, text: String) {
            let body: [String: Any] = ["state": state, "last": seq, "events": [["seq": seq, "data": Self.delta(text)]]]
            let json = String(decoding: try! JSONSerialization.data(withJSONObject: body), as: UTF8.self)
            for byte in "data: \(json)\n\n".utf8 { client?.urlProtocol(self, didLoad: Data([byte])) }
        }
        if after == 0 { send("running", seq: 1, text: "开头🌱") }
        let delivery = DispatchWorkItem { [self] in
            guard pendingDelivery?.isCancelled == false else { return }
            if state.loseStream && after == 0 { client?.urlProtocol(self, didFailWithError: URLError(.networkConnectionLost)); return }
            send("complete", seq: 2, text: "完整结尾")
            client?.urlProtocolDidFinishLoading(self)
        }
        pendingDelivery = delivery
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35, execute: delivery)
    }
    override func stopLoading() { pendingDelivery?.cancel(); pendingDelivery = nil }
    static func delta(_ text: String) -> String { String(data: try! JSONSerialization.data(withJSONObject: ["choices": [["delta": ["content": text]]]]), encoding: .utf8)! }
    static var configuration: URLSessionConfiguration { let value = URLSessionConfiguration.ephemeral; value.protocolClasses = [Self.self]; return value }
}

@MainActor
final class CloudReplyTests: XCTestCase {
    private func store() -> WorkspaceStore {
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)), streamConfiguration: CloudReplyFixtureProtocol.configuration, tokenProvider: { "synthetic" })
        store.newChat(); store.settings.demo = false
        store.settings.endpoint = "https://cloud-reply.invalid/v1/chat/completions"; store.settings.model = "fixture/one"
        store.settings.cloudAccount = RemoteAccountProfile(owner: "owner", email: "fixture@example.test", relay: URL(string: "https://cloud-reply.invalid")!, scope: "cloud")
        return store
    }
    private func pending(_ store: WorkspaceStore, body: Data? = nil, cursor: Int = 0, stop: Bool = false) -> ChatMessage {
        var message = ChatMessage(role: "assistant", text: cursor == 1 ? "开头🌱" : "", state: .streaming)
        message.cloudReply = CloudReply(id: message.id.uuidString.lowercased(), endpoint: store.settings.endpoint, owner: "owner", requestBody: body, cursor: cursor, stopRequested: stop)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id)
        store.update { $0.messages = [message] }; store.persist()
        return message
    }
    private func finish(_ store: WorkspaceStore) async throws {
        for _ in 0..<400 where store.generatingID != nil { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertNil(store.generatingID)
    }
    func testSubscriptionShowsTextBeforeCompletionAndNeverPolls() async throws {
        let store = store(), message = pending(store)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id, .init(stream: true))
        store.resumeCloudReplies()
        for _ in 0..<40 where store.selected.messages.last?.text.isEmpty == true { try await Task.sleep(for: .milliseconds(5)) }
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱")
        XCTAssertEqual(store.selected.messages.last?.state, .streaming)
        try await finish(store)
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertEqual(store.selected.messages.last?.state, .complete)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).subscriptions, [0])
        XCTAssertTrue(CloudReplyFixtureProtocol.state(message.cloudReply!.id).gets.isEmpty)
        XCTAssertEqual(try store.storage.load()?.conversations.last?.messages.last?.cloudReply?.cursor, 2)
    }
    func testStopCancelsSubscriptionAndPreservesReceivedText() async throws {
        let store = store(), message = pending(store)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id, .init(stream: true))
        store.resumeCloudReplies()
        for _ in 0..<40 where store.selected.messages.last?.text.isEmpty == true { try await Task.sleep(for: .milliseconds(5)) }
        store.stop(); try await finish(store)
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱")
        XCTAssertEqual(store.selected.messages.last?.state, .stopped)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).cancels, 1)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).subscriptions, [0])
    }
    func testSubscriptionDisconnectResumesWithoutDuplicatingText() async throws {
        let store = store(), message = pending(store)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id, .init(stream: true, loseStream: true))
        store.resumeCloudReplies(); try await finish(store)
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertEqual(store.selected.messages.last?.state, .complete)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).subscriptions, [0, 1])
        XCTAssertTrue(CloudReplyFixtureProtocol.state(message.cloudReply!.id).gets.isEmpty)
    }
    func testRestartRestoresCloudStateAndFetchesOnlyMissingEvents() async throws {
        let original = store(), message = pending(original, cursor: 1)
        let restored = WorkspaceStore(storage: original.storage, streamConfiguration: CloudReplyFixtureProtocol.configuration, tokenProvider: { "synthetic" })
        XCTAssertEqual(restored.selected.messages.last?.state, .streaming)
        XCTAssertNil(restored.selected.messages.last?.failure)
        restored.resumeCloudReplies(); try await finish(restored)
        XCTAssertEqual(restored.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertEqual(restored.selected.messages.last?.state, .complete)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).gets, [1])
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).puts, 0)
    }
    func testLostReceiptRetriesSameIDAndBodyWithoutDuplicatingOutput() async throws {
        let store = store(), body = Data(#"{"model":"fixture/one","stream":true,"messages":[{"role":"user","content":"hello"}]}"#.utf8)
        let message = pending(store, body: body)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id, .init(loseReceipt: true))
        store.resumeCloudReplies(); try await finish(store)
        let state = CloudReplyFixtureProtocol.state(message.cloudReply!.id)
        XCTAssertEqual(state.puts, 2); XCTAssertEqual(state.bodies, [body, body])
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertNil(store.selected.messages.last?.cloudReply?.requestBody)
    }
    func testNetworkLossAfterPartialPageResumesFromPersistedCursor() async throws {
        let store = store(), message = pending(store)
        CloudReplyFixtureProtocol.configure(message.cloudReply!.id, .init(loseRead: true))
        store.resumeCloudReplies(); try await finish(store)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).gets, [0, 1, 1])
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertEqual(store.selected.messages.last?.state, .complete)
    }
    func testStopIntentSurvivesRestartAndPreventsLateSubmission() async throws {
        let original = store(), message = pending(original, body: Data("{}".utf8), stop: true)
        let restored = WorkspaceStore(storage: original.storage, streamConfiguration: CloudReplyFixtureProtocol.configuration, tokenProvider: { "synthetic" })
        restored.resumeCloudReplies(); try await finish(restored)
        XCTAssertEqual(restored.selected.messages.last?.state, .stopped)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).puts, 0)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).cancels, 1)
    }
    func testReplayIsIdempotentAndCompletionWaitsForAllPages() throws {
        let store = store(), message = pending(store)
        let first = CloudReplyPage(state: "complete", last: 2, events: [.init(seq: 1, data: CloudReplyFixtureProtocol.delta("开头🌱"))], failure: nil)
        try store.applyCloudPage(first, messageID: message.id, conversationID: store.selectedID)
        try store.applyCloudPage(first, messageID: message.id, conversationID: store.selectedID)
        XCTAssertEqual(store.selected.messages.last?.text, "开头🌱")
        XCTAssertEqual(store.selected.messages.last?.state, .streaming)
        let disk = try store.storage.load()!
        XCTAssertEqual(disk.conversations.last?.messages.last?.cloudReply?.cursor, 1)
        XCTAssertEqual(disk.conversations.last?.messages.last?.text, "开头🌱")
        let end = CloudReplyPage(state: "complete", last: 2, events: [.init(seq: 2, data: CloudReplyFixtureProtocol.delta("完整结尾"))], failure: nil)
        try store.applyCloudPage(end, messageID: message.id, conversationID: store.selectedID)
        XCTAssertEqual(store.selected.messages.last?.state, .complete)
    }
    func testUncheckpointedTextAndCursorReplayTogetherAfterRestart() async throws {
        var original: WorkspaceStore? = store()
        let message = pending(original!), storage = original!.storage
        let page = CloudReplyPage(state: "running", last: 1, events: [.init(seq: 1, data: CloudReplyFixtureProtocol.delta("开头🌱"))], failure: nil)
        try original!.applyCloudPage(page, messageID: message.id, conversationID: original!.selectedID, checkpoint: false)
        XCTAssertEqual(original!.selected.messages.last?.cloudReply?.cursor, 1)
        let disk = try storage.load()!.conversations.last!.messages.last!
        XCTAssertEqual(disk.cloudReply?.cursor, 0); XCTAssertEqual(disk.text, "")
        original = nil
        let restored = WorkspaceStore(storage: storage, streamConfiguration: CloudReplyFixtureProtocol.configuration, tokenProvider: { "synthetic" })
        restored.resumeCloudReplies(); try await finish(restored)
        XCTAssertEqual(restored.selected.messages.last?.text, "开头🌱完整结尾")
        XCTAssertEqual(restored.selected.messages.last?.cloudReply?.cursor, 2)
    }
    func testInvalidPageRollsBackTextAndCursorTogether() throws {
        let store = store(), message = pending(store)
        let page = CloudReplyPage(state: "running", last: 3, events: [.init(seq: 1, data: CloudReplyFixtureProtocol.delta("must rollback")), .init(seq: 3, data: CloudReplyFixtureProtocol.delta("gap"))], failure: nil)
        XCTAssertThrowsError(try store.applyCloudPage(page, messageID: message.id, conversationID: store.selectedID))
        XCTAssertEqual(store.selected.messages.last?.text, "")
        XCTAssertEqual(store.selected.messages.last?.cloudReply?.cursor, 0)
        XCTAssertEqual(try store.storage.load()?.conversations.last?.messages.last?.text, "")
    }
    func testDiskWriteFailureRollsBackTextAndCursor() throws {
        let store = store(), message = pending(store)
        let original = try Data(contentsOf: store.storage.stateURL)
        try FileManager.default.removeItem(at: store.storage.stateURL)
        try FileManager.default.createDirectory(at: store.storage.stateURL, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: store.storage.stateURL); try? original.write(to: store.storage.stateURL) }
        let page = CloudReplyPage(state: "complete", last: 1, events: [.init(seq: 1, data: CloudReplyFixtureProtocol.delta("must rollback"))], failure: nil)
        XCTAssertThrowsError(try store.applyCloudPage(page, messageID: message.id, conversationID: store.selectedID))
        XCTAssertEqual(store.selected.messages.last?.text, "")
        XCTAssertEqual(store.selected.messages.last?.cloudReply?.cursor, 0)
        XCTAssertEqual(store.selected.messages.last?.state, .streaming)
    }
    func testNewCloudSendPersistsRecoveryIDAndCompletesThroughJobAPI() async throws {
        let store = store(); store.update { $0.input = "请继续" }; store.send(); try await finish(store)
        let message = store.selected.messages.last!
        XCTAssertEqual(message.state, .complete); XCTAssertEqual(message.text, "开头🌱完整结尾")
        XCTAssertNotNil(message.cloudReply)
        XCTAssertEqual(CloudReplyFixtureProtocol.state(message.cloudReply!.id).puts, 1)
    }
}
