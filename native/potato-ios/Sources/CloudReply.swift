import Foundation

/// The request is saved before submitting; its stable ID makes a lost receipt safe to retry.
/// Credentials stay in Keychain. Accepted request bodies are removed from the local checkpoint.
struct CloudReply: Codable, Equatable {
    var id: String
    var endpoint: String
    var owner: String
    var requestBody: Data?
    var cursor = 0
    var stopRequested = false
    var notice: String? = nil
    var createdAt = Date()
}
struct CloudReplyPage: Decodable {
    struct Event: Decodable { let seq: Int; let data: String }
    let state: String
    let last: Int
    let events: [Event]
    let failure: String?
}
struct CloudReplyHTTPError: Error {
    let status: Int
    var retryable: Bool { status == 401 || status == 403 || status == 429 || status >= 500 }
    var notice: String {
        if status == 401 || status == 403 { return L10n.tr("请重新登录原云端账号，以恢复这条回复。") }
        if status == 429 { return L10n.tr("正在等待服务恢复，稍后自动同步回复。") }
        return L10n.tr("暂时无法同步，连接恢复后会自动继续。")
    }
}
struct CloudReplyService {
    let configuration: URLSessionConfiguration
    /// False means this server predates subscriptions. Reuse the durable polling path.
    @MainActor
    func follow(_ job: CloudReply, token: String, receive: (CloudReplyPage) throws -> Void) async throws -> Bool {
        guard let endpoint = URL(string: job.endpoint), endpoint.scheme == "https",
              var url = URLComponents(url: endpoint, resolvingAgainstBaseURL: false) else { throw LocalFailure.message(L10n.tr("云端回复连接无效。")) }
        url.path = "/v1/chat/jobs/\(job.id)/events"
        url.queryItems = [URLQueryItem(name: "after", value: String(job.cursor))]; url.fragment = nil
        var request = URLRequest(url: url.url!); request.timeoutInterval = 45
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let session = URLSession(configuration: configuration, delegate: NoRedirectDelegate(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        if [400, 404, 405, 501].contains(http.statusCode) { return false }
        guard (200...299).contains(http.statusCode) else { throw CloudReplyHTTPError(status: http.statusCode) }
        guard http.value(forHTTPHeaderField: "Content-Type")?.lowercased().hasPrefix("text/event-stream") == true else { return false }
        var decoder = CloudReplyEventDecoder(cursor: job.cursor)
        var received = false
        for try await byte in bytes {
            try Task.checkCancellation()
            if let page = try decoder.consume(byte) {
                try receive(page); received = true
                if !["queued", "running"].contains(page.state), decoder.cursor == page.last { return true }
            }
        }
        try Task.checkCancellation()
        guard received, decoder.isEmpty else { throw URLError(.networkConnectionLost) }
        // The server rotates idle connections. Resume using the checkpoint, without delay.
        return true
    }
    func request(_ job: CloudReply, token: String, method: String) async throws -> CloudReplyPage? {
        guard let endpoint = URL(string: job.endpoint), endpoint.scheme == "https",
              var url = URLComponents(url: endpoint, resolvingAgainstBaseURL: false) else { throw LocalFailure.message(L10n.tr("云端回复连接无效。")) }
        url.path = "/v1/chat/jobs/\(job.id)" + (method == "POST" ? "/cancel" : "")
        url.queryItems = method == "GET" ? [URLQueryItem(name: "after", value: String(job.cursor))] : nil
        url.fragment = nil
        var request = URLRequest(url: url.url!)
        request.httpMethod = method; request.timeoutInterval = 30
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        if method == "PUT" { request.httpBody = job.requestBody; request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        let session = URLSession(configuration: configuration, delegate: NoRedirectDelegate(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        guard (200...299).contains(http.statusCode) else { throw CloudReplyHTTPError(status: http.statusCode) }
        guard data.count <= 8_000_000 else { throw LocalFailure.message(L10n.tr("云端回复数据过大。")) }
        guard method == "GET" else { return nil }
        let page = try JSONDecoder().decode(CloudReplyPage.self, from: data)
        try Self.validate(page, after: job.cursor)
        return page
    }
    static func validate(_ page: CloudReplyPage, after: Int) throws {
        guard ["queued", "running", "complete", "failed", "stopped"].contains(page.state), page.last >= after,
              page.events.count <= 64 else { throw LocalFailure.message(L10n.tr("云端回复状态无效。")) }
        var cursor = after
        for event in page.events {
            guard event.seq == cursor + 1, event.seq <= page.last, event.data.utf8.count <= 5_000_000 else { throw LocalFailure.message(L10n.tr("云端回复顺序无效。")) }
            cursor = event.seq
        }
    }
}

/// Bound raw lines before decoding; network packets can split a Chinese character or emoji.
struct CloudReplyEventDecoder {
    var cursor: Int
    init(cursor: Int) { self.cursor = cursor }
    private var line = Data()
    private var data = Data()
    var isEmpty: Bool { line.isEmpty && data.isEmpty }
    mutating func consume(_ byte: UInt8) throws -> CloudReplyPage? {
        guard byte == 10 else {
            guard line.count + data.count < 8_000_000 else { throw LocalFailure.message(L10n.tr("云端回复数据过大。")) }
            line.append(byte); return nil
        }
        if line.last == 13 { line.removeLast() }
        defer { line.removeAll(keepingCapacity: true) }
        if line.isEmpty {
            guard !data.isEmpty else { return nil }
            defer { data.removeAll(keepingCapacity: true) }
            let page = try JSONDecoder().decode(CloudReplyPage.self, from: data)
            try CloudReplyService.validate(page, after: cursor)
            cursor = page.events.last?.seq ?? cursor
            return page
        }
        if line.starts(with: [100, 97, 116, 97, 58]) {
            var value = line.dropFirst(5)
            if value.first == 32 { value = value.dropFirst() }
            if !data.isEmpty { data.append(10) }
            data.append(contentsOf: value)
        }
        return nil
    }
}
