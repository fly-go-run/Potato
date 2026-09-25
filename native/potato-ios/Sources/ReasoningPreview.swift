#if DEBUG
import Foundation

/// Explicit, isolated UI fixture. It exercises URLSession/SSE and store updates;
/// it never contacts a model, and cannot be enabled in a Release build.
final class ReasoningPreview: URLProtocol {
    private var delivery: Task<Void, Never>?
    static var requested: Bool {
        let args = ProcessInfo.processInfo.arguments
        return args.contains("--ui-testing") && args.contains("--reasoning-preview")
    }
    static var configuration: URLSessionConfiguration {
        let result = URLSessionConfiguration.ephemeral
        if requested { result.protocolClasses = [Self.self] }
        return result
    }
    @MainActor static func prepare(_ store: WorkspaceStore) {
        guard requested, ProcessInfo.processInfo.arguments.contains("--reset") else { return }
        let mode = ProcessInfo.processInfo.arguments.first { $0.hasPrefix("--reasoning-case=") }?.components(separatedBy: "=").last ?? "complete"
        store.settings.demo = false; store.settings.endpoint = "https://potato-reasoning-preview.invalid/" + mode; store.settings.model = "reasoning-fixture"
        store.newChat()
        store.update { chat in
            chat.draft = nil; chat.messages = []; chat.title = "思考流程验证"; chat.input = "比较 9.11 和 9.8，解释结论。"
        }
        store.persist()
    }
    override class func canInit(with request: URLRequest) -> Bool { requested && request.url?.host == "potato-reasoning-preview.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        delivery = Task { [weak self] in
            guard let self else { return }
            do {
                let mode = self.request.url!.lastPathComponent
                self.client?.urlProtocol(self, didReceive: HTTPURLResponse(url: self.request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "text/event-stream"])!, cacheStoragePolicy: .notAllowed)
                if mode == "streaming" {
                    let history = (1...12).map { "第\($0)段：这是一段用于检查长回复追底的合成文字，逐步显示时不应忽然缩回。" }.joined(separator: "\n\n")
                    self.emit(["choices": [["delta": ["content": history + "\n\n稳定正文锚点"]]]])
                    try await Task.sleep(for: .seconds(2))
                    for character in "\n```swift\nlet value = 42\n```\n\n回复末尾：中文与家庭👨‍👩‍👧‍👦完整保留。" {
                        self.emit(["choices": [["delta": ["content": String(character)]]]])
                        try await Task.sleep(for: .milliseconds(100))
                    }
                    try await Task.sleep(for: .seconds(2))
                    self.done(); return
                }
                if mode == "timeline" {
                    // Every hand-off in one reply: thought → commentary → tool → thought again → answer.
                    try await Task.sleep(for: .seconds(1.5))
                    self.emit(["choices": [["delta": ["reasoning_content": "先把小数位对齐，再比较相同数位。"]]]])
                    try await Task.sleep(for: .seconds(3))
                    for chunk in ["我先查一下", "小数比较的规则，", "再对照两个数。"] {
                        self.emit(["choices": [["delta": ["content": chunk]]]]); try await Task.sleep(for: .milliseconds(250))
                    }
                    try await Task.sleep(for: .seconds(0.8))
                    self.emit(["potato_search": ["id": "fixture-search", "query": "十进制小数比较规则", "state": "searching", "results": []]])
                    try await Task.sleep(for: .seconds(3.5))
                    self.emit(["potato_search": ["id": "fixture-search", "query": "十进制小数比较规则", "state": "complete", "results": []]])
                    try await Task.sleep(for: .seconds(1.2))
                    self.emit(["choices": [["delta": ["reasoning_content": "\n继续核对：9.8 等于 9.80，十分位 8 大于 1。"]]]])
                    try await Task.sleep(for: .seconds(3))
                    for chunk in ["\n\n**9.8 更大。**", " 把 9.8 写成 9.80，", "就能逐位比较：", "整数位相同，", "十分位 8 > 1，", "所以 9.80 > 9.11。"] {
                        self.emit(["choices": [["delta": ["content": chunk]]]]); try await Task.sleep(for: .milliseconds(300))
                    }
                    try await Task.sleep(for: .seconds(0.5))
                    self.done(); return
                }
                try await Task.sleep(for: .seconds(2))
                self.emit(["choices": [["delta": ["reasoning_content": "先把小数位对齐，再比较相同数位。"]]]])
                try await Task.sleep(for: .seconds(mode == "hold" ? 30 : 7))
                if mode == "interrupted" { self.client?.urlProtocolDidFinishLoading(self); return }
                if mode == "only" { self.done(); return }
                if mode == "search" {
                    self.emit(["potato_search": ["id": "fixture-search", "query": "十进制小数比较规则", "state": "searching", "results": []]])
                    try await Task.sleep(for: .seconds(6))
                    self.emit(["potato_search": ["id": "fixture-search", "query": "十进制小数比较规则", "state": "complete", "results": []]])
                    self.emit(["choices": [["delta": ["reasoning_content": "\n继续核对：9.8 等于 9.80。"]]]])
                    try await Task.sleep(for: .seconds(5))
                }
                self.emit(["choices": [["delta": ["reasoning_content": "\n十分位的 8 大于 1。", "content": "**9.8 更大。**"]]]])
                try await Task.sleep(for: .seconds(4))
                self.emit(["choices": [["delta": ["content": " 把 9.8 写成 9.80，就能直接比较：9.80 > 9.11。"]]]])
                self.done()
            } catch { /* Cancellation ends the fixture without delivering late data. */ }
        }
    }
    private func emit(_ value: [String: Any]) {
        guard !Task.isCancelled, let data = try? JSONSerialization.data(withJSONObject: value) else { return }
        client?.urlProtocol(self, didLoad: Data("data: ".utf8) + data + Data("\n\n".utf8))
    }
    private func done() {
        guard !Task.isCancelled else { return }
        client?.urlProtocol(self, didLoad: Data("data: [DONE]\n\n".utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() { delivery?.cancel(); delivery = nil }
}
#endif
