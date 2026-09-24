import Foundation
import CryptoKit

struct RecallSource: Codable, Equatable, Identifiable {
    var id: String
    var role: String
    var text: String
    var date: String
    var version: String?
    var conversation: String
    var title: String
    var revision: String
    var identity: String { "\(conversation)/\(id)/\(version ?? "")" }
}
struct RecallRun: Codable, Equatable, Identifiable {
    var id: String
    var state: String
    var sources: [RecallSource]
    var message: String?
}
struct PersonalMemory: Codable, Identifiable, Equatable {
    var id: String
    var text: String
    var revision: String
    var updated: String
    var sources: [RecallSource]
}
struct RecallStatus: Decodable {
    struct Entry: Decodable { var revision: String; var excluded: Bool }
    var scope: String
    var entries: [String: Entry]
    var memories: [PersonalMemory]
}
struct RecallPayload: Encodable {
    struct Message: Encodable {
        var id: String; var role: String; var text: String; var date: String; var version: String?
    }
    var id: String; var title: String; var excluded: Bool; var messages: [Message]
    init(_ chat: Conversation) {
        id = chat.id.uuidString.lowercased(); title = String(chat.title.prefix(200))
        excluded = chat.recallExcluded == true || chat.deletedAt != nil || chat.isExample
        let formatter = ISO8601DateFormatter(); formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        messages = excluded ? [] : chat.messages.filter { $0.displayState == .complete && !$0.displayText.isEmpty && ["user", "assistant"].contains($0.role) }.map {
            Message(id: $0.id.uuidString.lowercased(), role: $0.role, text: $0.displayText, date: formatter.string(from: $0.selectedVersion?.date ?? $0.createdAt), version: $0.selectedVersionID?.uuidString.lowercased())
        }
    }
    func content() throws -> String {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let data = try encoder.encode(self)
        guard data.count <= 512_000, messages.count <= 2000 else { throw LocalFailure.message(L10n.tr("“\(title)”超过历史同步上限（512 KB / 2000 条消息），请排除此对话后重试。")) }
        return String(decoding: data, as: UTF8.self)
    }
}
private final class RecallNoRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}
struct RecallService {
    let settings: ConnectionSettings
    let token: String
    var configuration: URLSessionConfiguration = .ephemeral
    static func digest(_ value: String) -> String { SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined() }
    func request(_ action: String, body: [String: Any]? = nil) throws -> URLRequest {
        guard let endpoint = settings.validatedURL, endpoint.path.hasSuffix("/chat/completions"), !token.isEmpty else { throw LocalFailure.message(L10n.tr("登录 Potato 账号后才能使用记忆与历史。")) }
        let url = endpoint.deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("recall").appendingPathComponent(action)
        var request = URLRequest(url: url); request.httpMethod = body == nil ? "GET" : "POST"; request.timeoutInterval = 45
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let body { request.httpBody = try JSONSerialization.data(withJSONObject: body) }
        return request
    }
    func send(_ request: URLRequest) async throws -> Data {
        let session = URLSession(configuration: configuration, delegate: RecallNoRedirect(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (data, response) = try await session.data(for: request)
        try Task.checkCancellation()
        guard let http = response as? HTTPURLResponse else { throw LocalFailure.message(L10n.tr("历史服务没有响应。")) }
        guard (200...299).contains(http.statusCode) else {
            let errors = [401: L10n.tr("登录已失效，请重新登录后同步历史。"), 409: L10n.tr("云端记录已改变，为避免覆盖，已停止同步。请保留本地记录并检查其他设备。"), 429: L10n.tr("历史请求过于频繁，请稍后重试。"), 503: L10n.tr("云端历史暂不可用，请稍后再试。"), 507: L10n.tr("历史或记忆容量已达上限。")]
            throw LocalFailure.message(errors[http.statusCode] ?? L10n.tr("历史操作失败（\(http.statusCode)）。"))
        }
        guard data.count <= 1_100_000 else { throw LocalFailure.message(L10n.tr("历史服务返回内容过大。")) }
        return data
    }
    func status() async throws -> RecallStatus { try await JSONDecoder().decode(RecallStatus.self, from: send(request("status"))) }
    func memory(id: String, text: String, base: String?, forget: Bool) async throws {
        _ = try await send(request("memory", body: ["id": id, "text": text, "base": base as Any? ?? NSNull(), "forget": forget]))
    }
    func sync(_ chats: [Conversation], storage: LocalStorage, deletionsOnly: Bool = false) async throws -> RecallStatus {
        let status = try await status()
        let cache = storage.root.appendingPathComponent("recall-\(Self.digest((settings.serviceIdentity ?? settings.endpoint) + status.scope)).json")
        var acknowledged: [String: String] = [:]
        if FileManager.default.fileExists(atPath: cache.path) { acknowledged = try JSONDecoder().decode([String: String].self, from: Data(contentsOf: cache)) }
        for chat in chats where !chat.isExample {
            try Task.checkCancellation()
            let payload = RecallPayload(chat)
            if deletionsOnly && !payload.excluded { continue }
            let content = try payload.content(), digest = Self.digest(content)
            if status.entries[payload.id]?.revision == digest { acknowledged[payload.id] = digest; continue }
            if status.entries[payload.id] == nil && (payload.excluded || payload.messages.isEmpty) { continue }
            let result = try await send(request("sync", body: ["content": content, "base": acknowledged[payload.id] as Any? ?? NSNull()]))
            struct Receipt: Decodable { var revision: String }
            acknowledged[payload.id] = try JSONDecoder().decode(Receipt.self, from: result).revision
            try storage.prepare()
            try JSONEncoder().encode(acknowledged).write(to: cache, options: [.atomic, .completeFileProtectionUnlessOpen])
        }
        try storage.prepare()
        try JSONEncoder().encode(acknowledged).write(to: cache, options: [.atomic, .completeFileProtectionUnlessOpen])
        return try await self.status()
    }
}

#if DEBUG
@MainActor enum RecallPreview {
    static func prepare(_ store: WorkspaceStore) {
        guard ProcessInfo.processInfo.arguments.contains("--ui-testing"), ProcessInfo.processInfo.arguments.contains("--recall-preview") else { return }
        var original = Conversation(); original.title = "昨日的购买决定"
        original.messages = [ChatMessage(role: "user", text: "我最后选择 X100，并已下单。")]
        original.messages += (1...20).map { ChatMessage(role: "assistant", text: "后续讨论 \($0)：这是一段用于检查来源定位的合成记录。") }
        let source = RecallSource(id: original.messages[0].id.uuidString.lowercased(), role: "user", text: original.messages[0].text, date: "2026-09-12T10:00:00Z", conversation: original.id.uuidString.lowercased(), title: original.title, revision: String(repeating: "a", count: 64))
        var answer = Conversation(); answer.title = "回忆昨天"
        var message = ChatMessage(role: "assistant", text: "你昨天选购了 X100。")
        message.recalls = [RecallRun(id: "sources", state: "complete", sources: [source])]
        answer.messages = [ChatMessage(role: "user", text: "昨天选了什么？"), message]
        store.conversations = [original, answer]; store.selectedID = answer.id; store.settings = ConnectionSettings(); store.persist()
    }
}
#endif
