import Foundation

struct SandboxExecution: Codable, Equatable {
    var status: String
    var stdout: String
    var stderr: String
    var error: String?
    var text: String
    var artifacts: [SandboxArtifact]
}
struct SandboxArtifact: Codable, Equatable {
    var name: String
    var mime: String
    var base64: String
}
private final class SandboxNoRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}
enum SandboxService {
    static func request(code: String, files: [Attachment], settings: ConnectionSettings, token: String, storage: LocalStorage) throws -> URLRequest {
        guard !settings.demo, let endpoint = settings.validatedURL, endpoint.path.hasSuffix("/v1/chat/completions") else { throw LocalFailure.message(L10n.tr("请先连接支持云端计算的 Potato 服务。")) }
        guard code.count <= 32_000, files.count <= 4 else { throw LocalFailure.message(L10n.tr("代码过长或文件超过 4 个。")) }
        var components = URLComponents(url: endpoint, resolvingAgainstBaseURL: false)!
        components.path = String(endpoint.path.dropLast("/v1/chat/completions".count)) + "/v1/sandbox/run"
        var input: [[String: String]] = []
        var total = 0
        for (index, file) in files.enumerated() {
            let data = try Data(contentsOf: storage.url(for: file)); total += data.count
            guard total <= 2_000_000 else { throw LocalFailure.message(L10n.tr("云端计算的输入文件合计最多 2 MB。")) }
            input.append(["name": filename(file, index: index), "base64": data.base64EncodedString()])
        }
        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"; request.timeoutInterval = 110
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["code": code, "files": input])
        return request
    }
    static func filename(_ file: Attachment, index: Int) -> String {
        let suffix = URL(fileURLWithPath: file.filename).pathExtension.lowercased().filter { $0.isASCII && $0.isLetter }
        return "input-\(index + 1).\(suffix.isEmpty ? "bin" : suffix)"
    }
    static func run(_ request: URLRequest, configuration: URLSessionConfiguration = .ephemeral) async throws -> SandboxExecution {
        let session = URLSession(configuration: configuration, delegate: SandboxNoRedirect(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (bytes, response) = try await session.bytes(for: request)
        guard let response = response as? HTTPURLResponse else { throw LocalFailure.message(L10n.tr("计算服务未响应。")) }
        switch response.statusCode {
        case 200: break
        case 401: throw LocalFailure.message(L10n.tr("连接令牌已失效，请检查设置。"))
        case 404, 503: throw LocalFailure.message(L10n.tr("云端代码运行暂不可用，请稍后再试。"))
        case 429: throw LocalFailure.message(L10n.tr("请求较多，请稍后再运行。"))
        default: throw LocalFailure.message(L10n.tr("计算失败，可能已超过执行时间或输出大小限制。"))
        }
        var data = Data()
        for try await byte in bytes { try Task.checkCancellation(); data.append(byte); guard data.count <= 4_200_000 else { throw LocalFailure.message(L10n.tr("计算结果过大，请减少输出。")) } }
        return try JSONDecoder().decode(SandboxExecution.self, from: data)
    }
}
