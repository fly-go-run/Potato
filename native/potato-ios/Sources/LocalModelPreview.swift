#if DEBUG
import Foundation

final class LocalModelPreview: URLProtocol {
    private var work: Task<Void, Never>?
    private static var enabled: Bool { let args = ProcessInfo.processInfo.arguments; return args.contains("--ui-testing") && args.contains("--local-model-preview") }
    static var configuration: URLSessionConfiguration? {
        guard enabled else { return nil }; let value = URLSessionConfiguration.ephemeral; value.protocolClasses = [Self.self]; return value
    }
    @MainActor static func prepare(_ store: WorkspaceStore) {
        guard enabled, ProcessInfo.processInfo.arguments.contains("--reset") else { return }
        store.settings.demo = false; store.settings.endpoint = "https://local-model-preview.invalid/v1/chat/completions"; store.settings.model = "quick"; store.settings.modelCatalog = nil
        store.newChat(); store.update { $0.title = "模型选择验证" }; store.persist()
        if ProcessInfo.processInfo.arguments.contains("--unavailable-model-preview") {
            store.settings.model = "retired"; store.persist()
        }
        if ProcessInfo.processInfo.arguments.contains("--curated-cloud-preview") {
            store.settings.cloudAccount = RemoteAccountProfile(owner: "curated-cloud-fixture", email: "fixture@example.test", relay: URL(string: "https://local-model-preview.invalid")!, scope: "cloud")
            store.settings.model = "deepseek/deepseek-v4-pro"
            store.update { $0.modelChoice = LocalModelChoice(endpoint: store.settings.serviceIdentity!, model: "deepseek/deepseek-v4.1-flash-expires-on-0910"); $0.input = "保留云端草稿" }
            store.persist()
        }
    }
    override class func canInit(with request: URLRequest) -> Bool { enabled && ["local-model-preview.invalid", "other-model-preview.invalid"].contains(request.url?.host ?? "") }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var data = request.httpBody ?? Data()
        if let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }; var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let count = stream.read(&buffer, maxLength: buffer.count); if count <= 0 { break }; data.append(contentsOf: buffer.prefix(count)) }
        }
        let body = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        if ProcessInfo.processInfo.arguments.contains("--cloud-models-preview"), let path = request.url?.path, path.hasPrefix("/v1/models") {
            answerCloudModels(path: path, body: body); return
        }
        let catalog = request.httpMethod == "GET"
        work = Task {
            try? await Task.sleep(for: .milliseconds(100)); guard !Task.isCancelled else { return }
            if !catalog, ProcessInfo.processInfo.arguments.contains("--unavailable-model-preview"), body["model"] as? String == "retired" {
                client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: "HTTP/1.1", headerFields: [:])!, cacheStoragePolicy: .notAllowed)
                client?.urlProtocolDidFinishLoading(self); return
            }
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": catalog ? "application/json" : "text/event-stream"])!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            let payload: String
            if catalog {
                if ProcessInfo.processInfo.arguments.contains("--curated-cloud-preview") {
                    payload = #"{"default_model":"deepseek/deepseek-flash","data":[{"id":"deepseek/deepseek-flash","name":"DeepSeek V4.1 Flash","reasoning_effort_options":["low","high","max"],"thinking_modes":["enabled","disabled"]},{"id":"sub2api/gpt-5.6","name":"GPT-5.6","reasoning_effort_options":["none","low","medium","high","xhigh","max"],"thinking_modes":[]}]}"#
                } else {
                    payload = #"{"data":[{"id":"quick","name":"快速模型"},{"id":"deep","name":"深入模型","reasoning_effort_options":["low","high"],"thinking_modes":["enabled","disabled"]}]}"#
                }
            } else {
                let text = "MODEL=\(body["model"] as? String ?? "");THINKING=\((body["thinking"] as? [String: String])?["type"] ?? "default");EFFORT=\(body["reasoning_effort"] as? String ?? "default")"
                let frame = try! JSONSerialization.data(withJSONObject: ["choices": [["delta": ["content": text]]]])
                payload = "data: \(String(decoding: frame, as: UTF8.self))\n\ndata: [DONE]\n\n"
            }
            client?.urlProtocol(self, didLoad: Data(payload.utf8)); client?.urlProtocolDidFinishLoading(self)
        }
    }
    override func stopLoading() { work?.cancel() }
    // Synthetic admin-editable cloud list for the settings screens.
    nonisolated(unsafe) private static var cloudModels = (ids: ["deepseek/deepseek-flash", "sub2api/gpt-6"], defaultModel: "deepseek/deepseek-flash", revision: 1)
    private static let cloudNames = ["deepseek/deepseek-flash": "DeepSeek V4.1 Flash", "sub2api/gpt-6": "GPT-6", "sub2api/gpt-5.6-sol": "GPT-5.6 Sol", "sub2api/claude-sonnet-5": "claude-sonnet-5", "sub2api/gemini-3-pro": "gemini-3-pro"]
    private func answerCloudModels(path: String, body: [String: Any]) {
        if path.hasSuffix("/enabled"), let ids = body["models"] as? [String], let defaultModel = body["default_model"] as? String {
            Self.cloudModels = (ids, defaultModel, Self.cloudModels.revision + 1)
        }
        let value: [String: Any]
        if path.hasSuffix("/available") {
            let providers = [("deepseek", "DeepSeek", ["deepseek/deepseek-flash"]), ("sub2api", "sub2api", ["sub2api/gpt-6", "sub2api/gpt-5.6-sol", "sub2api/claude-sonnet-5", "sub2api/gemini-3-pro"])]
            value = ["revision": Self.cloudModels.revision, "providers": providers.map { id, name, models in
                ["id": id, "name": name, "models": models.map { ["id": $0, "name": Self.cloudNames[$0] ?? $0, "enabled": Self.cloudModels.ids.contains($0)] }] as [String: Any] }]
        } else {
            value = ["object": "list", "catalog_source": "configured", "default_model": Self.cloudModels.defaultModel, "revision": Self.cloudModels.revision, "can_edit": true,
                     "data": Self.cloudModels.ids.map { ["id": $0, "name": Self.cloudNames[$0] ?? $0] }]
        }
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: try! JSONSerialization.data(withJSONObject: value)); client?.urlProtocolDidFinishLoading(self)
    }
}
#endif
