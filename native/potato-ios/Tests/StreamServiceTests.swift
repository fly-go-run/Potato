import XCTest
@testable import PotatoMobile

final class StreamFixtureProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "fixture.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let path = request.url!.path
        if path == "/probe" {
            let body = request.httpBody ?? request.httpBodyStream.map { stream -> Data in
                stream.open(); defer { stream.close() }
                var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
                while stream.hasBytesAvailable { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(contentsOf: buffer.prefix(n)) }
                return data
            } ?? Data()
            let json = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
            let messages = json?["messages"] as? [[String: Any]]
            guard json?["max_tokens"] as? Int == 16, messages?.count == 2,
                  messages?.first?["content"] as? String == "这是连接测试。只回复 OK。",
                  messages?.last?["content"] as? String == "请回复 OK",
                  request.value(forHTTPHeaderField: "Authorization") == "Bearer fixture-token" else {
                client?.urlProtocol(self, didFailWithError: LocalFailure.message("测试请求带入了额外数据或缺少限制。")); return
            }
            if json?["model"] as? String == "deepseek-fixture", (json?["thinking"] as? [String: String])?["type"] != "disabled" {
                client?.urlProtocol(self, didFailWithError: LocalFailure.message("连接测试没有关闭思考模式。")); return
            }
            if json?["model"] as? String == "deepseek-unknown", json?["thinking"] != nil {
                client?.urlProtocol(self, didFailWithError: LocalFailure.message("未知模型被按名称附加了思考参数。")); return
            }
        }
        let status = path == "/unauthorized" ? 401 : 200
        let mime = path == "/wrong-format" ? "application/json" : "text/event-stream"
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": mime])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"跨字节中文🌱\"}}]}\r\n\r\n"
        // Deliberately split every UTF-8 character across network callbacks.
        if path != "/empty" { for byte in payload.utf8 { client?.urlProtocol(self, didLoad: Data([byte])) } }
        if path == "/limited" { client?.urlProtocol(self, didLoad: Data("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n".utf8)) }
        if path != "/interrupted" { client?.urlProtocol(self, didLoad: Data("data: [DONE]\n\n".utf8)) }
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}
final class StreamServiceTests: XCTestCase {
    func testConnectionProbeUsesOnlyShortIsolatedMessage() async throws {
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/probe"; settings.model = "deepseek-fixture"
        settings.systemPrompt = "私人回复偏好，不应发送到测试接口"
        settings.modelCatalog = LocalModelCatalog(endpoint: settings.serviceIdentity!, models: [LocalModelEntry(id: settings.model, name: settings.model, thinking_modes: ["disabled"])])
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [StreamFixtureProtocol.self]
        try await ChatService.testConnection(settings: settings, token: "fixture-token", configuration: config)
    }
    func testConnectionProbeRejectsEmptyInterruptedAndInvalidResponses() async {
        for path in ["empty", "interrupted", "unauthorized", "wrong-format", "limited"] {
            var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/" + path; settings.model = "fixture"
            let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [StreamFixtureProtocol.self]
            do { try await ChatService.testConnection(settings: settings, token: "", configuration: config); XCTFail("A \(path) response cannot prove connection success") }
            catch { XCTAssertFalse(error.localizedDescription.isEmpty) }
        }
    }
    func testConnectionProbeDoesNotInferThinkingFromModelPrefix() async throws {
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/probe"; settings.model = "deepseek-unknown"
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [StreamFixtureProtocol.self]
        try await ChatService.testConnection(settings: settings, token: "fixture-token", configuration: config)
    }
    func testNetworkErrorsGiveActionableDescriptionsWithoutURLs() {
        let error = URLError(.timedOut, userInfo: [NSURLErrorFailingURLStringErrorKey: "https://private.invalid/path"])
        XCTAssertEqual(ChatService.failureDescription(error), "等待服务响应超时，请稍后重试。")
        XCTAssertFalse(ChatService.failureDescription(error).contains("private.invalid"))
    }
    private func stream(_ path: String) -> AsyncThrowingStream<String, Error> {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StreamFixtureProtocol.self]
        return ChatService.stream(request: URLRequest(url: URL(string: "https://fixture.invalid/" + path)!), configuration: configuration)
    }
    func testRealURLSessionBytesDecodeSplitUnicode() async throws {
        var result = ""
        for try await text in stream("success") { result += text }
        XCTAssertEqual(result, "跨字节中文🌱")
    }
    func testMissingTerminalIsInterruption() async throws {
        var result = ""
        do { for try await text in stream("interrupted") { result += text }; XCTFail("should fail") }
        catch { XCTAssertTrue(error.localizedDescription.contains("连接中断")) }
        XCTAssertEqual(result, "跨字节中文🌱")
    }
    func testUnauthorizedAndNonStreamingResponses() async {
        for path in ["unauthorized", "wrong-format"] {
            do { for try await _ in stream(path) { XCTFail("must not emit text") }; XCTFail("must fail") }
            catch { XCTAssertFalse(error.localizedDescription.isEmpty) }
        }
    }
}

@MainActor final class ToolHistoryTests: XCTestCase {
    private func search(_ id: String, state: String = "complete") -> WebSearchRun {
        WebSearchRun(id: id, query: id, state: state, results: [WebSource(title: "Source", url: "https://example.test/source", content: String(repeating: "界", count: 700), publishedDate: "2026-09-13")])
    }
    private func code(_ id: String, state: String = "complete") -> CodeExecutionRun {
        CodeExecutionRun(id: id, state: state, code: "print(323)", result: SandboxExecution(status: state, stdout: "323", stderr: "", text: "report", artifacts: [SandboxArtifact(name: "a.png", mime: "image/png", base64: "YQ==")]))
    }
    private func wire(_ messages: [ChatMessage]) throws -> [[String: Any]] {
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/v1/chat/completions"; settings.model = "fixture"
        let disk = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let request = try ChatService.request(settings: settings, token: "fixture", messages: messages, storage: disk)
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(request.httpBody)) as? [String: Any])
        return try XCTUnwrap(body["messages"] as? [[String: Any]])
    }
    private func calls(_ message: [String: Any]) throws -> [[String: Any]] {
        try XCTUnwrap(message["tool_calls"] as? [[String: Any]])
    }
    func testSearchAndCodeReplayInOrderWithoutBase64() throws {
        let message = ChatMessage(codeRuns: [code("python")], searches: [search("search")], role: "assistant", text: "Final answer")
        let messages = Array(try wire([message]).dropFirst())
        XCTAssertEqual(messages.compactMap { $0["role"] as? String }, ["assistant", "tool", "tool", "assistant"])
        let toolCalls = try calls(messages[0])
        XCTAssertEqual(toolCalls.compactMap { $0["id"] as? String }, ["search", "python"])
        XCTAssertEqual(toolCalls.compactMap { ($0["function"] as? [String: String])?["name"] }, ["web_search", "run_python"])
        XCTAssertEqual((toolCalls[0]["function"] as? [String: String])?["arguments"], "{\"query\":\"search\"}")
        XCTAssertEqual((toolCalls[1]["function"] as? [String: String])?["arguments"], "{\"code\":\"print(323)\"}")
        XCTAssertEqual(messages[0]["content"] as? String, "")
        XCTAssertEqual(messages[1]["tool_call_id"] as? String, "search")
        XCTAssertEqual(messages[2]["tool_call_id"] as? String, "python")
        let searchText = try XCTUnwrap(messages[1]["content"] as? String)
        let searchJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(searchText.utf8)) as? [String: Any])
        XCTAssertEqual(((searchJSON["results"] as? [[String: Any]])?.first?["content"] as? String)?.count, 600)
        let codeText = try XCTUnwrap(messages[2]["content"] as? String)
        XCTAssertFalse(codeText.contains("base64")); XCTAssertFalse(codeText.contains("YQ=="))
        let codeJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(codeText.utf8)) as? [String: Any])
        XCTAssertEqual(codeJSON["artifacts"] as? [[String: String]], [["name": "a.png", "mime": "image/png"]])
        XCTAssertEqual(messages[3]["content"] as? String, "Final answer")
    }
    func testSelectedReplyVersionSuppliesItsOwnRuns() throws {
        let old = ReplyVersion(codeRuns: [code("old-code")], searches: [search("old-search")], text: "Old answer", state: .complete)
        var message = ChatMessage(codeRuns: [code("new-code")], searches: [search("new-search")], versions: [old], role: "assistant", text: "New answer")
        for selected in [true, false] {
            message.selectedVersionID = selected ? old.id : nil
            let messages = Array(try wire([message]).dropFirst())
            XCTAssertEqual(try calls(messages[0]).compactMap { $0["id"] as? String }, selected ? ["old-search", "old-code"] : ["new-search", "new-code"])
            XCTAssertEqual(messages.last?["content"] as? String, selected ? "Old answer" : "New answer")
        }
    }
    func testBudgetDropsWholeOldestReplyAndKeepsLaterPairs() throws {
        let replies = (0..<3).map { index in
            let runs = (0..<3).map { runIndex in
                var run = code("\(index)-\(runIndex)")
                run.code = String(repeating: "x", count: 16_000)
                run.result?.stdout = String(repeating: "界", count: 4_000)
                return run
            }
            return ChatMessage(codeRuns: runs, role: "assistant", text: "Answer \(index)")
        }
        let history = try ChatService.toolHistory(replies)
        XCTAssertNil(history[replies[0].id]); XCTAssertEqual(history.count, 2)
        var bytes = 0
        for reply in replies.dropFirst() {
            let segment = try XCTUnwrap(history[reply.id])
            XCTAssertEqual(segment.count, 4)
            let ids = try calls(segment[0]).compactMap { $0["id"] as? String }
            XCTAssertEqual(segment.dropFirst().compactMap { $0["tool_call_id"] as? String }, ids)
            bytes += try JSONSerialization.data(withJSONObject: segment).count
            for tool in segment.dropFirst() {
                let content = try XCTUnwrap(tool["content"] as? String)
                XCTAssertLessThanOrEqual(content.utf8.count, 8_000)
                XCTAssertTrue(content.hasSuffix("…[truncated]"))
            }
        }
        XCTAssertLessThanOrEqual(bytes, 200_000)
        let messages = Array(try wire(replies).dropFirst())
        XCTAssertEqual(messages[0]["role"] as? String, "assistant")
        XCTAssertEqual(messages[0]["content"] as? String, "Answer 0")
        XCTAssertNil(messages[0]["tool_calls"])
        XCTAssertEqual(messages.filter { $0["role"] as? String == "tool" }.count, 6)
    }
    func testRunningAndStoppedRunsAreNotReplayed() throws {
        let message = ChatMessage(codeRuns: [code("running", state: "running"), code("stopped", state: "stopped")], searches: [search("running-search", state: "running"), search("stopped-search", state: "stopped"), search("searching", state: "searching")], role: "assistant", text: "Partial answer")
        let messages = Array(try wire([message]).dropFirst())
        XCTAssertEqual(messages.count, 1); XCTAssertNil(messages[0]["tool_calls"])
        XCTAssertEqual(messages[0]["content"] as? String, "Partial answer")
    }
}
