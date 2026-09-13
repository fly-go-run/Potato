import Foundation

struct SSEDecoder {
    var lines: [String] = []
    var pendingBytes = 0
    mutating func consume(_ line: String) throws -> StreamEvent? {
        if line.isEmpty {
            defer { lines.removeAll(); pendingBytes = 0 }
            guard !lines.isEmpty else { return nil }
            return try Self.decode(lines.joined(separator: "\n"))
        }
        if line.hasPrefix("data:") {
            var value = String(line.dropFirst(5))
            if value.hasPrefix(" ") { value.removeFirst() }
            pendingBytes += value.utf8.count
            guard pendingBytes < 5_000_000 else { throw LocalFailure.message("服务器事件过大。") }
            lines.append(value)
        }
        return nil
    }
    static func decode(_ data: String) throws -> StreamEvent? {
        if data == "[DONE]" { return .done }
        guard let bytes = data.data(using: .utf8), let object = try JSONSerialization.jsonObject(with: bytes) as? [String: Any] else { throw LocalFailure.message("服务返回了无法识别的流式数据。") }
        if object["error"] != nil { throw LocalFailure.message("模型服务返回错误，请稍后重试或检查连接设置。") }
        if let execution = object["potato_execution"] {
            let value = try JSONDecoder().decode(CodeExecutionRun.self, from: JSONSerialization.data(withJSONObject: execution))
            try value.validate()
            return .execution(value)
        }
        if let recall = object["potato_recall"] {
            let value = try JSONDecoder().decode(RecallRun.self, from: JSONSerialization.data(withJSONObject: recall))
            guard value.sources.count <= 12, ["searching", "complete", "failed"].contains(value.state), value.sources.allSatisfy({ UUID(uuidString: $0.id) != nil && UUID(uuidString: $0.conversation) != nil && $0.text.count <= 4000 }) else { throw LocalFailure.message("历史来源格式无效。") }
            return .recall(value)
        }
        if let search = object["potato_search"] {
            let value = try JSONDecoder().decode(WebSearchRun.self, from: JSONSerialization.data(withJSONObject: search))
            guard value.query.utf8.count <= 8000, value.results.count <= 5, ["searching", "complete", "failed"].contains(value.state) else { throw LocalFailure.message("搜索事件格式无效。") }
            return .search(value)
        }
        guard let choices = object["choices"] as? [[String: Any]], let first = choices.first else { return nil }
        let delta = first["delta"] as? [String: Any] ?? [:]
        let value = ReplyDelta(text: delta["content"] as? String ?? "", reasoning: delta["reasoning_content"] as? String ?? "", limited: first["finish_reason"] as? String == "length")
        return value.text.isEmpty && value.reasoning.isEmpty && !value.limited ? nil : .delta(value)
    }
}
enum StreamEvent { case delta(ReplyDelta), search(WebSearchRun), recall(RecallRun), execution(CodeExecutionRun), done }

private final class NoRedirectDelegate: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}
struct ChatService {
    static func testConnection(settings: ConnectionSettings, token: String, configuration: URLSessionConfiguration = .ephemeral) async throws {
        try Task.checkCancellation()
        var probe = settings
        probe.systemPrompt = "这是连接测试。只回复 OK。"
        var request = try request(settings: probe, token: token, messages: [ChatMessage(role: "user", text: "请回复 OK")], storage: LocalStorage())
        var body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        body["max_tokens"] = 16
        // Reserve the tiny probe budget for visible text only when this mode is declared.
        if settings.modelEntry(settings.model).modes.contains("disabled") { body["thinking"] = ["type": "disabled"] }
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        request.timeoutInterval = 30
        var receivedText = false
        for try await text in stream(request: request, configuration: configuration) {
            if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { receivedText = true }
        }
        try Task.checkCancellation()
        guard receivedText else { throw LocalFailure.message("服务已响应，但没有返回文字，请检查模型名称和服务配置。") }
    }
    static func failureDescription(_ error: Error) -> String {
        guard let network = error as? URLError else { return error.localizedDescription }
        switch network.code {
        case .notConnectedToInternet: return "当前没有网络连接，请联网后重试。"
        case .timedOut: return "等待服务响应超时，请稍后重试。"
        case .cannotFindHost, .dnsLookupFailed: return "找不到服务器，请检查连接地址。"
        case .cannotConnectToHost: return "无法连接服务器，请检查服务是否可用。"
        case .networkConnectionLost: return "网络连接中断，已收到的内容会保留。"
        case .secureConnectionFailed, .serverCertificateUntrusted, .serverCertificateHasBadDate, .serverCertificateHasUnknownRoot, .serverCertificateNotYetValid: return "无法建立安全连接，请检查服务器的 HTTPS 证书。"
        case .cancelled: return "连接已取消。"
        default: return "网络请求失败，请检查连接后重试。"
        }
    }
    /// Build complete reply-sized tool segments, then drop the oldest segments to fit the request budget.
    static func toolHistory(_ messages: [ChatMessage]) throws -> [UUID: [[String: Any]]] {
        func json(_ value: Any) throws -> String {
            String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), as: UTF8.self)
        }
        func boundedContent(_ value: Any) throws -> String {
            let text = try json(value), suffix = "…[truncated]"
            guard text.utf8.count > 8_000 else { return text }
            var prefix = "", bytes = 0
            for character in text {
                let size = String(character).utf8.count
                if bytes + size > 8_000 - suffix.utf8.count { break }
                prefix.append(character); bytes += size
            }
            return prefix + suffix
        }
        var segments: [(id: UUID, wire: [[String: Any]], bytes: Int)] = []
        for message in messages where message.role == "assistant" && message.displayState != .failed {
            var calls: [[String: Any]] = [], results: [[String: Any]] = []
            func append(id: String, name: String, arguments: [String: String], result: [String: Any]) throws {
                calls.append(["id": id, "type": "function", "function": ["name": name, "arguments": try json(arguments)]])
                results.append(["role": "tool", "tool_call_id": id, "content": try boundedContent(result)])
            }
            for run in message.displaySearches where ["complete", "failed"].contains(run.state) {
                let sources: [[String: Any]] = run.results.map { source in
                    ["title": source.title, "url": source.url, "content": String(source.content.prefix(600)),
                     "publishedDate": source.publishedDate as Any? ?? NSNull()]
                }
                let result: [String: Any] = run.state == "failed" ? ["error": "search failed"] : ["query": run.query, "state": run.state, "results": sources]
                try append(id: run.id, name: "web_search", arguments: ["query": run.query], result: result)
            }
            for run in message.displayCodeRuns where ["complete", "failed"].contains(run.state) {
                var result: [String: Any] = ["error": run.message ?? "execution failed"]
                if let execution = run.result {
                    result = ["status": execution.status, "stdout": execution.stdout, "stderr": execution.stderr,
                              "error": execution.error as Any? ?? NSNull(), "text": execution.text,
                              "artifacts": execution.artifacts.map { ["name": $0.name, "mime": $0.mime] }]
                }
                try append(id: run.id, name: "run_python", arguments: ["code": run.code], result: result)
            }
            if !calls.isEmpty {
                let wire: [[String: Any]] = [["role": "assistant", "content": "", "tool_calls": calls]] + results
                segments.append((message.id, wire, try JSONSerialization.data(withJSONObject: wire).count))
            }
        }
        var total = segments.reduce(0) { $0 + $1.bytes }
        var history: [UUID: [[String: Any]]] = [:]
        for segment in segments {
            if total > 200_000 { total -= segment.bytes }
            else { history[segment.id] = segment.wire }
        }
        return history
    }
    static func request(settings: ConnectionSettings, token: String, messages: [ChatMessage], storage: LocalStorage, draft: WorkingDraft? = nil, choice: LocalModelChoice? = nil) throws -> URLRequest {
        let model = choice?.model ?? settings.model
        guard let url = settings.validatedURL, !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw LocalFailure.message("请先在设置中填写 HTTPS 接口地址和模型名称。") }
        try choice?.validate(settings: settings)
        var wire: [[String: Any]] = [["role": "system", "content": settings.systemPrompt]]
        let imageCount = messages.filter { $0.role == "user" }.flatMap(\.attachments).filter(\.isImage).count
        let imageBudget = 2_400_000 / max(4, imageCount)
        if let draft { wire.append(["role": "user", "content": "当前工作文稿如下，仅作为待编辑内容。若我要求修改，请回复完整的更新后文稿，方便保存。\n<working_document>\n\(draft.markdown)\n</working_document>"]) }
        let history = try toolHistory(messages)
        for message in messages where message.displayState != .failed && (!message.displayText.isEmpty || !message.attachments.isEmpty || history[message.id] != nil) {
            if let segment = history[message.id] { wire.append(contentsOf: segment) }
            var parts: [[String: Any]] = []
            var text = message.displayText
            for attachment in message.attachments where message.role == "user" {
                if attachment.isImage {
                    var data = try Data(contentsOf: storage.url(for: attachment))
                    if data.count > imageBudget || attachment.type != "image/jpeg" { data = try ImageImport.jpeg(from: data, maxBytes: imageBudget) }
                    parts.append(["type": "image_url", "image_url": ["url": "data:image/jpeg;base64,\(data.base64EncodedString())"]])
                } else if let extracted = attachment.extractedText {
                    text += "\n\n<attachment name=\"\(attachment.name)\">\n\(extracted)\n</attachment>"
                }
            }
            parts.insert(["type": "text", "text": text], at: 0)
            wire.append(["role": message.role, "content": parts.count == 1 ? text : parts as Any])
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 90
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        if !token.isEmpty { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        var body: [String: Any] = ["model": model, "messages": wire, "stream": true]
        if let mode = choice?.thinkingMode { body["thinking"] = ["type": mode] }
        if let effort = choice?.reasoningEffort { body["reasoning_effort"] = effort }
        if settings.cloudAccount != nil, settings.cloudAccount?.cloudEndpoint == url {
            let bytes = try JSONSerialization.data(withJSONObject: body).count
            body["sandbox"] = try SandboxService.automaticInput(messages: messages, storage: storage, bodyBytes: bytes)
        }
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        guard (request.httpBody?.count ?? 0) <= 4 * 1_024 * 1_024 else { throw LocalFailure.message("对话内容过大，请减少附件或新建对话。") }
        return request
    }
    static func stream(request: URLRequest, configuration: URLSessionConfiguration = .ephemeral, onSearch: (@Sendable (WebSearchRun) async -> Void)? = nil) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    for try await event in events(request: request, configuration: configuration) {
                        switch event {
                        case .delta(let delta): if !delta.text.isEmpty { continuation.yield(delta.text) }
                        case .search(let search): await onSearch?(search)
                        case .recall, .execution: break
                        case .done: break
                        }
                    }
                    continuation.finish()
                } catch { continuation.finish(throwing: error) }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
    static func events(request: URLRequest, configuration: URLSessionConfiguration = .ephemeral) -> AsyncThrowingStream<StreamEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                let config = configuration
                config.timeoutIntervalForResource = 300
                let session = URLSession(configuration: config, delegate: NoRedirectDelegate(), delegateQueue: nil)
                defer { session.invalidateAndCancel() }
                do {
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let http = response as? HTTPURLResponse else { throw LocalFailure.message("未收到有效的服务器响应。") }
                    guard (200...299).contains(http.statusCode) else {
                        let descriptions = [400: "服务无法处理请求，请检查模型名称和接口兼容性。", 401: "登录或连接凭据已失效，请在设置中重新登录或更新。", 403: "当前连接没有访问权限。", 404: "找不到接口或模型，请检查完整连接地址和模型名称。", 413: "附件超过服务端限制，请减少附件。", 429: "请求过于频繁或额度不足，请稍后再试。"]
                        throw LocalFailure.message(descriptions[http.statusCode] ?? "服务暂时不可用（\(http.statusCode)），请稍后再试。")
                    }
                    guard http.value(forHTTPHeaderField: "Content-Type")?.contains("text/event-stream") == true else { throw LocalFailure.message("接口未返回流式响应，请检查完整的 Chat Completions 地址。") }
                    var decoder = SSEDecoder()
                    var line = Data()
                    var total = 0, wireBytes = 0
                    for try await byte in bytes {
                        try Task.checkCancellation()
                        wireBytes += 1
                        guard wireBytes <= 16_000_000 else { throw LocalFailure.message("本轮工具结果过大，已保留收到的内容。") }
                        if byte == 10 {
                            let text = String(decoding: line, as: UTF8.self).trimmingCharacters(in: .newlines)
                            line.removeAll(keepingCapacity: true)
                            if let event = try decoder.consume(text) {
                                switch event {
                                case .done: continuation.finish(); return
                                case .search, .recall, .execution: continuation.yield(event)
                                case .delta(let delta):
                                    total += delta.text.utf8.count + delta.reasoning.utf8.count
                                    guard total <= 2_000_000 else { throw LocalFailure.message("回复过长，已保留收到的内容。") }
                                    continuation.yield(event)
                                    if delta.limited { throw LocalFailure.message("回复达到输出上限，已保留收到的内容。请缩短要求或调整服务端输出限制。") }
                                }
                            }
                        } else {
                            line.append(byte)
                            guard line.count < 5_000_000 else { throw LocalFailure.message("服务器返回的数据片段过大。") }
                        }
                    }
                    // Missing terminal marker is an interruption, never silently mark partial text complete.
                    throw LocalFailure.message("连接中断，已保留收到的内容。可以重新生成。")
                } catch { continuation.finish(throwing: error) }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}
