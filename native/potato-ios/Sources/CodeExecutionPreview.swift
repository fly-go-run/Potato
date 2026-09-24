#if DEBUG
import Foundation

final class CodeExecutionPreview: URLProtocol {
    private var delivery: Task<Void, Never>?
    static var requested: Bool { let args = ProcessInfo.processInfo.arguments; return args.contains("--ui-testing") && args.contains("--code-preview") }
    static var configuration: URLSessionConfiguration? {
        guard requested else { return nil }
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [Self.self]; return config
    }
    @MainActor static func prepare(_ store: WorkspaceStore) {
        guard requested, ProcessInfo.processInfo.arguments.contains("--reset") else { return }
        store.settings.demo = false; store.settings.endpoint = "https://code-preview.invalid/v1/chat/completions"; store.settings.model = "code-fixture"
        // This UI fixture emits tool events over SSE. Cloud job replay is covered by CloudReplyTests.
        store.settings.cloudAccount = nil
        store.newChat(); store.update { chat in chat.draft = nil; chat.messages = []; chat.title = "代码工具验证"; chat.input = "计算 17×19 并生成报告。" }; store.persist()
    }
    override class func canInit(with request: URLRequest) -> Bool { requested && request.url?.host == "code-preview.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        delivery = Task { [weak self] in
            guard let self else { return }
            do {
                self.client?.urlProtocol(self, didReceive: HTTPURLResponse(url: self.request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "text/event-stream"])!, cacheStoragePolicy: .notAllowed)
                self.emit(["potato_execution": ["id": "python-fixture", "state": "running", "code": "print(17 * 19)"]])
                try await Task.sleep(for: .seconds(ProcessInfo.processInfo.arguments.contains("--code-hold") ? 30 : 4))
                self.emit(["potato_execution": ["id": "python-fixture", "state": "complete", "code": "print(17 * 19)", "result": ["status": "complete", "stdout": "323", "stderr": "", "text": "", "artifacts": [["name": "report.md", "mime": "text/markdown", "base64": Data("# 计算报告\n17 × 19 = 323".utf8).base64EncodedString()]]]]])
                self.emit(["choices": [["delta": ["content": "计算结果是 **323**。已生成 report.md。"]]]])
                self.client?.urlProtocol(self, didLoad: Data("data: [DONE]\n\n".utf8)); self.client?.urlProtocolDidFinishLoading(self)
            } catch { }
        }
    }
    private func emit(_ value: [String: Any]) {
        guard !Task.isCancelled, let bytes = try? JSONSerialization.data(withJSONObject: value) else { return }
        client?.urlProtocol(self, didLoad: Data("data: ".utf8) + bytes + Data("\n\n".utf8))
    }
    override func stopLoading() { delivery?.cancel(); delivery = nil }
}
#endif
