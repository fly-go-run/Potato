import Foundation

struct LocalModelEntry: Codable, Equatable, Identifiable {
    let id: String
    var name: String
    var reasoning_effort_options: [String]? = nil
    var thinking_modes: [String]? = nil
    var thinking_param_style: String? = nil
    var efforts: [String] { thinking_param_style == nil || thinking_param_style == "effort" ? unique(reasoning_effort_options ?? []) : [] }
    var modes: [String] { unique(thinking_modes ?? []).filter { ["enabled", "disabled"].contains($0) } }
    private func unique(_ values: [String]) -> [String] { values.reduce(into: []) { if !$0.contains($1) { $0.append($1) } } }
    func documented(for endpoint: URL?) -> Self {
        var value = self
        // Exact origin and published IDs, not a name-prefix capability guess.
        // https://api-docs.deepseek.com/guides/thinking_mode/ — checked 2026-09-13.
        if let endpoint, endpoint.scheme == "https", endpoint.host == "api.deepseek.com", endpoint.port == nil || endpoint.port == 443,
           ["/chat/completions", "/v1/chat/completions"].contains(endpoint.path), ["deepseek-flash", "deepseek-v4-flash", "deepseek-v4-pro"].contains(id) {
            if value.reasoning_effort_options == nil { value.reasoning_effort_options = ["low", "high", "max"] }
            if value.thinking_modes == nil { value.thinking_modes = ["enabled", "disabled"] }
        }
        if let endpoint, endpoint.scheme == "https", endpoint.host == "api.openai.com", endpoint.port == nil || endpoint.port == 443,
           endpoint.path == "/v1/chat/completions", ["gpt-5.6", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"].contains(id) {
            if value.reasoning_effort_options == nil { value.reasoning_effort_options = ["none", "low", "medium", "high", "xhigh", "max"] }
            if value.thinking_modes == nil { value.thinking_modes = [] }
        }
        return value
    }
}
struct LocalModelCatalog: Codable, Equatable {
    let endpoint: String
    let models: [LocalModelEntry]
    var defaultModel: String? = nil
    var source = "service"
    var fetchedAt = Date()
}
struct LocalModelChoice: Codable, Equatable {
    let endpoint: String
    let model: String
    var thinkingMode: String? = nil
    var reasoningEffort: String? = nil
    var thinkingLabel: String {
        if thinkingMode == "disabled" { return "思考关闭" }
        if let reasoningEffort { return "思考 · \(remoteEffortName(reasoningEffort))" }
        return thinkingMode == "enabled" ? "思考开启" : "服务默认"
    }
    var compactThinkingLabel: String {
        if thinkingMode == "disabled" { return "关闭" }
        if let reasoningEffort { return remoteEffortName(reasoningEffort) }
        return thinkingMode == "enabled" ? "开启" : "默认"
    }
    func validate(settings: ConnectionSettings) throws {
        guard settings.serviceIdentity != nil else { throw LocalFailure.message("请先在连接设置中填写有效的 HTTPS 地址。草稿已保留。") }
        guard endpoint == settings.serviceIdentity else { throw LocalFailure.message("此会话的模型属于之前的服务，请重新选择当前服务的模型。草稿已保留。") }
        guard !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, model.utf8.count <= 256 else { throw LocalFailure.message("请填写有效的模型名称。") }
        if settings.cloudAccount != nil, let catalog = settings.currentCatalog, !catalog.models.contains(where: { $0.id == model }) {
            throw LocalFailure.message("此模型已不在云端列表中，请选择当前可用模型。旧回复与草稿已保留。")
        }
        let entry = settings.modelEntry(model)
        if let thinkingMode, !entry.modes.contains(thinkingMode) { throw LocalFailure.message("当前服务未声明这个思考模式，请刷新模型列表或使用服务默认。") }
        if let reasoningEffort, !entry.efforts.contains(reasoningEffort) { throw LocalFailure.message("当前模型未声明这个思考档位，请重新选择。") }
        if thinkingMode == "disabled" && reasoningEffort != nil { throw LocalFailure.message("关闭思考时不能设置思考档位。") }
    }
}
extension ConnectionSettings {
    var serviceIdentity: String? { validatedURL?.absoluteString }
    var currentCatalog: LocalModelCatalog? { modelCatalog?.endpoint == serviceIdentity ? modelCatalog : nil }
    func modelEntry(_ id: String) -> LocalModelEntry {
        (currentCatalog?.models.first(where: { $0.id == id }) ?? LocalModelEntry(id: id, name: id)).documented(for: validatedURL)
    }
}

enum LocalModelService {
    static func catalogRequest(settings: ConnectionSettings, token: String) throws -> URLRequest {
        guard var url = settings.validatedURL, url.path.hasSuffix("/chat/completions") else { throw LocalFailure.message("此地址无法自动读取模型列表，可手动填写服务提供的模型名称。") }
        url.deleteLastPathComponent(); url.deleteLastPathComponent(); url.appendPathComponent("models")
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 20)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if !token.isEmpty { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        return request
    }
    static func catalog(settings: ConnectionSettings, token: String, configuration: URLSessionConfiguration = .ephemeral) async throws -> LocalModelCatalog {
        let request = try catalogRequest(settings: settings, token: token)
        let session = URLSession(configuration: configuration, delegate: ModelRedirectPolicy(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse else { throw LocalFailure.message("模型列表响应无效。") }
        guard http.statusCode == 200 else {
            if [404, 405].contains(http.statusCode) { throw LocalFailure.message("服务未提供模型列表，可手动填写模型名称。") }
            if [401, 403].contains(http.statusCode) { throw LocalFailure.message("读取模型列表未获授权。云端账号请重新登录或联系所有者授权；手动连接请检查令牌。") }
            throw LocalFailure.message("暂时无法读取模型列表（\(http.statusCode)），已保留当前选择。")
        }
        guard http.value(forHTTPHeaderField: "Content-Type")?.lowercased().contains("application/json") == true else { throw LocalFailure.message("模型列表格式不兼容，可手动填写模型名称。") }
        var data = Data()
        for try await byte in bytes {
            try Task.checkCancellation(); data.append(byte)
            guard data.count <= 1_000_000 else { throw LocalFailure.message("模型列表过大，已保留当前选择。") }
        }
        return try decode(data, settings: settings)
    }
    static func decode(_ data: Data, settings: ConnectionSettings) throws -> LocalModelCatalog {
        guard data.count <= 1_000_000,
              let json = try JSONSerialization.jsonObject(with: data) as? [String: Any], let entries = json["data"] as? [[String: Any]], entries.count <= 500,
              let endpoint = settings.serviceIdentity else { throw LocalFailure.message("模型列表格式无效。") }
        var models: [LocalModelEntry] = [], seen = Set<String>()
        for entry in entries {
            guard let id = entry["id"] as? String, !id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, id.utf8.count <= 256, seen.insert(id).inserted else { continue }
            func options(_ key: String) -> [String]? {
                guard let value = entry[key], !(value is NSNull) else { return nil }
                return Array(((value as? [String]) ?? []).filter { $0.range(of: "^[a-z0-9_-]{1,32}$", options: .regularExpression) != nil }.prefix(32))
            }
            let model = LocalModelEntry(id: id, name: String((entry["name"] as? String ?? id).prefix(256)), reasoning_effort_options: options("reasoning_effort_options"), thinking_modes: options("thinking_modes"), thinking_param_style: entry["thinking_param_style"] as? String)
            models.append(model.documented(for: settings.validatedURL))
        }
        return LocalModelCatalog(endpoint: endpoint, models: models, defaultModel: json["default_model"] as? String, source: json["catalog_source"] as? String == "configured" ? "configured" : "service")
    }
}
private final class ModelRedirectPolicy: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}
