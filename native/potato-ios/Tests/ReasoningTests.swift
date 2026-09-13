import XCTest
@testable import PotatoMobile

final class ReasoningFixtureProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "reasoning.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "text/event-stream"])!, cacheStoragePolicy: .notAllowed)
        let path = request.url!.path
        func emit(_ delta: [String: String], limited: Bool = false) {
            var choice: [String: Any] = ["delta": delta]
            if limited { choice["finish_reason"] = "length" }
            let data = try! JSONSerialization.data(withJSONObject: ["choices": [choice]])
            let frame = Data("data: ".utf8) + data + Data("\r\n\r\n".utf8)
            if path == "/budget" { client?.urlProtocol(self, didLoad: frame) }
            else { for byte in frame { client?.urlProtocol(self, didLoad: Data([byte])) } }
        }
        if path == "/budget" {
            for index in 0..<7 { emit([index % 2 == 0 ? "reasoning_content" : "content": String(repeating: "中", count: 100_000)]) }
        } else {
            emit(["reasoning_content": "先比较🌱"])
            if path != "/only" && path != "/interrupted" { emit(["reasoning_content": "，再核对。", "content": "结论：9.8 更大。"], limited: path == "/limited") }
        }
        if path != "/interrupted" { client?.urlProtocol(self, didLoad: Data("data: [DONE]\n\n".utf8)) }
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    static var configuration: URLSessionConfiguration { let c = URLSessionConfiguration.ephemeral; c.protocolClasses = [Self.self]; return c }
}

final class ReasoningTests: XCTestCase {
    func testOneFrameKeepsBothReasoningAndBodyAndLimitMarker() throws {
        let event = try SSEDecoder.decode(#"{"choices":[{"delta":{"reasoning_content":"思考","content":"回答"},"finish_reason":"length"}]}"#)
        guard case .delta(let delta) = event else { return XCTFail("Missing compound delta") }
        XCTAssertEqual(delta, ReplyDelta(text: "回答", reasoning: "思考", limited: true))
    }
    func testRealByteStreamPreservesUnicodeAndSameFrameOrdering() async throws {
        var deltas: [ReplyDelta] = []
        for try await event in ChatService.events(request: URLRequest(url: URL(string: "https://reasoning.invalid/success")!), configuration: ReasoningFixtureProtocol.configuration) {
            if case .delta(let value) = event { deltas.append(value) }
        }
        XCTAssertEqual(deltas, [ReplyDelta(reasoning: "先比较🌱"), ReplyDelta(text: "结论：9.8 更大。", reasoning: "，再核对。")])
    }
    func testLimitedFrameIsDeliveredBeforeFailure() async {
        var text = "", reasoning = ""
        do {
            for try await event in ChatService.events(request: URLRequest(url: URL(string: "https://reasoning.invalid/limited")!), configuration: ReasoningFixtureProtocol.configuration) {
                if case .delta(let value) = event { text += value.text; reasoning += value.reasoning }
            }
            XCTFail("The limit must fail the stream")
        } catch { XCTAssertTrue(error.localizedDescription.contains("输出上限")) }
        XCTAssertEqual(text, "结论：9.8 更大。"); XCTAssertEqual(reasoning, "先比较🌱，再核对。")
    }
    func testReasoningAndBodyShareTheResponseByteBudget() async {
        var bytes = 0
        do {
            for try await event in ChatService.events(request: URLRequest(url: URL(string: "https://reasoning.invalid/budget")!), configuration: ReasoningFixtureProtocol.configuration) {
                if case .delta(let value) = event { bytes += value.text.utf8.count + value.reasoning.utf8.count }
            }
            XCTFail("Combined response exceeds the shared limit")
        } catch { XCTAssertTrue(error.localizedDescription.contains("回复过长")) }
        XCTAssertEqual(bytes, 1_800_000)
    }
    func testReasoningClockStopsOnBodyAndAccumulatesResumedIntervals() {
        let origin = Date(timeIntervalSince1970: 1_000)
        var message = ChatMessage(role: "assistant", text: "", state: .streaming)
        message.receive(ReplyDelta(reasoning: "first"), at: origin)
        message.receive(ReplyDelta(text: "body"), at: origin.addingTimeInterval(4))
        XCTAssertEqual(message.reasoning?.state, .complete)
        XCTAssertEqual(message.reasoning?.duration(at: origin.addingTimeInterval(20)), 4)
        message.receive(ReplyDelta(reasoning: "second"), at: origin.addingTimeInterval(30))
        message.finishReply(.stopped, at: origin.addingTimeInterval(32))
        XCTAssertEqual(message.reasoning?.state, .stopped); XCTAssertEqual(message.reasoning?.elapsed, 6)
        message.receive(ReplyDelta(text: "late", reasoning: "late"), at: origin.addingTimeInterval(40))
        XCTAssertEqual(message.text, "body"); XCTAssertEqual(message.reasoning?.text, "firstsecond")
    }
    func testBodyFirstAndCompoundDeltaDoNotInventEarlierReasoning() {
        let now = Date()
        var message = ChatMessage(role: "assistant", text: "", state: .streaming)
        message.receive(ReplyDelta(text: "first"), at: now)
        XCTAssertNil(message.reasoning)
        message.receive(ReplyDelta(text: "second", reasoning: "same frame"), at: now)
        XCTAssertEqual(message.reasoning?.elapsed, 0); XCTAssertEqual(message.reasoning?.state, .complete)
        XCTAssertEqual(message.text, "firstsecond")
    }
    func testOldMessagesAndVersionsDecodeWithoutReasoning() throws {
        let message = ChatMessage(role: "assistant", text: "old")
        var json = try JSONSerialization.jsonObject(with: JSONEncoder().encode(message)) as! [String: Any]
        json.removeValue(forKey: "reasoning")
        let decoded = try JSONDecoder().decode(ChatMessage.self, from: JSONSerialization.data(withJSONObject: json))
        XCTAssertEqual(decoded.text, "old"); XCTAssertNil(decoded.displayReasoning)
        let version = ReplyVersion(text: "old version", state: .complete)
        let restored = try JSONDecoder().decode(ReplyVersion.self, from: JSONEncoder().encode(version))
        XCTAssertNil(restored.reasoning)
    }
}

@MainActor final class ReasoningStoreTests: XCTestCase {
    private func store(path: String) -> WorkspaceStore {
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)), streamConfiguration: ReasoningFixtureProtocol.configuration)
        store.settings.demo = false; store.settings.endpoint = "https://reasoning.invalid/" + path; store.settings.model = "fixture"
        store.newChat(); store.update { $0.input = "Compare two numbers" }; return store
    }
    private func finish(_ store: WorkspaceStore) async throws {
        for _ in 0..<100 where store.generatingID != nil { try await Task.sleep(for: .milliseconds(20)) }
        XCTAssertNil(store.generatingID)
    }
    func testCompleteReasoningPersistsAndRetryVersionsKeepTheirOwnTrace() async throws {
        let store = store(path: "success"); store.send(); try await finish(store)
        let old = store.selected.messages.last!
        XCTAssertEqual(old.state, .complete); XCTAssertEqual(old.reasoning?.state, .complete)
        XCTAssertEqual(old.reasoning?.text, "先比较🌱，再核对。")
        store.settings.demo = true; store.retry(); store.stop()
        let current = store.selected.messages.last!
        XCTAssertNil(current.reasoning); XCTAssertEqual(current.versions?.last?.reasoning, old.reasoning)
        store.chooseReplyVersion(current.id, offset: -1)
        store.persist()
        let restored = WorkspaceStore(storage: store.storage)
        XCTAssertEqual(restored.selected.messages.last?.displayReasoning, old.reasoning)
        // Previous reasoning is display history, never spliced into a plain chat prompt.
        var settings = ConnectionSettings(); settings.endpoint = "https://reasoning.invalid/next"; settings.model = "fixture"
        let request = try ChatService.request(settings: settings, token: "", messages: restored.selected.messages, storage: store.storage)
        let body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        XCTAssertFalse((body["messages"] as! [[String: Any]]).contains { $0["reasoning_content"] != nil })
    }
    func testOnlyReasoningAndInterruptedReasoningPreserveContentWithoutFalseSuccess() async throws {
        for path in ["only", "interrupted"] {
            let store = store(path: path); store.send(); try await finish(store)
            let message = store.selected.messages.last!
            XCTAssertEqual(message.state, .failed); XCTAssertEqual(message.reasoning?.state, .failed)
            XCTAssertEqual(message.reasoning?.text, "先比较🌱"); XCTAssertNil(message.reasoning?.activeSince)
            XCTAssertTrue(message.failure?.contains(path == "only" ? "仅返回思考内容" : "连接中断") == true)
        }
    }
    func testProcessRecoveryFreezesAtLastReceivedDataNotRelaunchTime() throws {
        let store = store(path: "unused"), start = Date(timeIntervalSince1970: 1_000)
        var message = ChatMessage(role: "assistant", text: "", state: .streaming)
        message.receive(ReplyDelta(reasoning: "first"), at: start)
        message.receive(ReplyDelta(reasoning: "last"), at: start.addingTimeInterval(5))
        store.update { $0.messages = [message] }; store.persist()
        let restored = WorkspaceStore(storage: store.storage).selected.messages[0]
        XCTAssertEqual(restored.state, .stopped); XCTAssertEqual(restored.reasoning?.state, .stopped)
        XCTAssertEqual(restored.reasoning?.elapsed, 5); XCTAssertNil(restored.reasoning?.activeSince)
    }
    func testSearchEndsReasoningIntervalAndFailureStopsUnfinishedSearch() {
        let store = store(path: "unused")
        var message = ChatMessage(role: "assistant", text: "", state: .streaming)
        message.receive(ReplyDelta(reasoning: "需要核对资料"))
        store.update { $0.messages = [message] }
        store.recordSearch(WebSearchRun(id: "search", query: "资料", state: "searching", results: []), messageID: message.id, conversationID: store.selectedID)
        XCTAssertEqual(store.selected.messages[0].reasoning?.state, .complete)
        var failed = store.selected.messages[0]
        failed.finishReply(.failed, failure: "断线")
        XCTAssertEqual(failed.searches?.first?.state, "failed")
        XCTAssertEqual(failed.reasoning?.state, .complete)
        XCTAssertNil(failed.reasoning?.activeSince)
    }
}
