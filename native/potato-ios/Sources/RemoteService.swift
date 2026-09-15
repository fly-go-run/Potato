import Foundation
import CryptoKit

struct RemoteDevice: Codable, Identifiable, Hashable {
    let id: String
    let name: String
    let relay: URL
    var owner: String? = nil
    var account: String { if let owner { return "remote-account-\(relay.host ?? "")-\(relay.port ?? 443)-\(owner)" }; return "remote-\(relay.host ?? "")-\(relay.port ?? 443)-\(id)" }
}
struct RemoteChat: Codable, Identifiable, Hashable {
    let id: String
    let session_id: String
    let name: String
    let status: String?
    let pinned: Bool?
    let project_path: String?
}
struct RemoteProject: Codable, Identifiable, Hashable { let path: String; let name: String; var id: String { path } }
struct RemoteOverview: Decodable { let chats: [RemoteChat]; let projects: [RemoteProject] }
struct RemoteMessage: Decodable, Identifiable {
    let id: String; let role: String?; let kind: String?; let text: String; let status: String?
    let callID: String?; let name: String?; let arguments: String?; let output: String?; let state: String?
    enum CodingKeys: String, CodingKey {
        case id, role, kind, text, status, name, arguments, output, state
        case callID = "call_id"
    }
}
struct RemoteApproval: Decodable, Identifiable {
    let request_id: String; let tool_name: String?; let findings_summary: String?; let exact_target: String?; let action_detail: String?; let justification: String?
    var allow_directory: Bool? = nil
    var suggested_directory: String? = nil
    var directory_recursive: Bool? = nil
    var supportsDirectoryGrant: Bool { allow_directory == true && !(suggested_directory ?? "").isEmpty }
    var review_outcome: String? = nil
    var review_rationale: String? = nil
    var review_failure: String? = nil
    var id: String { request_id }
    var reviewExplanation: String {
        if let failure = review_failure, !failure.isEmpty { return "自动审查未能完成，需要你确认这次操作。" }
        if let rationale = review_rationale, !rationale.isEmpty { return rationale }
        return "电脑正在等待你确认这次操作。允许仅对本次请求生效。"
    }
    private var arguments: [String: Any] { (action_detail.flatMap { $0.data(using: .utf8) }.flatMap { try? JSONSerialization.jsonObject(with: $0) }) as? [String: Any] ?? [:] }
    var command: String? { arguments["command"] as? String }
    var workingDirectory: String? { arguments["cwd"] as? String }
    var unsandboxed: Bool { arguments["sandbox_permissions"] as? String == "require_escalated" }
}
struct RemoteQuestion: Decodable, Identifiable {
    struct Option: Decodable, Identifiable { let id: String; let label: String }
    let request_id: String; let title: String; let status: String; let multiple: Bool?; let options: [Option]
    var id: String { request_id }
}
struct RemoteSnapshot: Decodable {
    var running_request_id: String? = nil
    var stop_protocol: Int? = nil
    var approval_scope_protocol: Int? = nil
    let chat: RemoteChat; let status: String; let messages: [RemoteMessage]; let live: [RemoteMessage]
    struct Outcome: Decodable {
        struct Failure: Decodable { let message: String }
        let status: String; let error: Failure?
    }
    let approvals: [RemoteApproval]; let questions: [RemoteQuestion]
    let outcome: Outcome?
    var displayMessages: [RemoteMessage] {
        var rows = messages
        for message in live { if let index = rows.firstIndex(where: { $0.id == message.id }) { rows[index] = message } else { rows.append(message) } }
        return rows.filter { !$0.text.isEmpty || $0.isProcess }
    }
}
enum RemoteService {
    struct Pairing { let url: URL; let token: String; let id: String; let relay: URL }
    static func parsePairing(_ code: String) throws -> Pairing {
        guard var parts = URLComponents(string: code.trimmingCharacters(in: .whitespacesAndNewlines)),
              let token = parts.fragment, token.count == 64, token.allSatisfy({ $0.isHexDigit }),
              parts.user == nil, parts.password == nil, parts.query == nil, let host = parts.host, !host.isEmpty else { throw LocalFailure.message("请粘贴电脑生成的完整配对码。") }
        var validScheme = parts.scheme == "https"
        #if DEBUG
        validScheme = validScheme || (parts.scheme == "http" && ["127.0.0.1", "localhost"].contains(host))
        #endif
        let segments = parts.path.split(separator: "/")
        guard validScheme, segments.count == 4, segments[0] == "v1", segments[1] == "remote", UUID(uuidString: String(segments[2])) != nil, segments[3] == "pair" else { throw LocalFailure.message("配对码格式无效，请重新复制。") }
        parts.fragment = nil
        guard let url = parts.url else { throw LocalFailure.message("配对地址无效。") }
        parts.path = "/"
        return Pairing(url: url, token: token, id: String(segments[2]), relay: parts.url!)
    }
    static func pair(_ code: String) async throws -> RemoteDevice {
        let pairing = try parsePairing(code)
        struct Reply: Decodable { let name: String }
        let attemptID = SHA256.hash(data: Data(code.trimmingCharacters(in: .whitespacesAndNewlines).utf8)).map { String(format: "%02x", $0) }.joined()
        let account = "remote-pairing-\(attemptID)"
        var token = SecureToken.read(account: account)
        if token.isEmpty {
            token = (UUID().uuidString + UUID().uuidString).replacingOccurrences(of: "-", with: "").lowercased()
            try SecureToken.save(token, account: account)
        }
        let reply: Reply = try await request(pairing.url, body: ["pair_token": pairing.token, "phone_token": token])
        let device = RemoteDevice(id: pairing.id, name: reply.name, relay: pairing.relay)
        try SecureToken.save(token, account: device.account)
        return device
    }
    static func status(_ device: RemoteDevice) async throws -> Bool {
        struct Status: Decodable { let online: Bool }
        let reply: Status = try await request(device.relay.appendingPathComponent("v1/remote/\(device.id)/status"), token: SecureToken.read(account: device.account))
        return reply.online
    }
    static func rpc<T: Decodable>(_ device: RemoteDevice, op: String, args: [String: Any] = [:], id: String = UUID().uuidString) async throws -> T {
        let reply: RemoteEnvelope<T> = try await request(device.relay.appendingPathComponent("v1/remote/\(device.id)/rpc"), token: SecureToken.read(account: device.account), body: ["id": id, "op": op, "args": args])
        return reply.result
    }
    static func request<T: Decodable>(_ url: URL, token: String? = nil, body: [String: Any]? = nil) async throws -> T {
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 22)
        request.httpMethod = body == nil ? "GET" : "POST"
        if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        if let body { request.httpBody = try JSONSerialization.data(withJSONObject: body); request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.urlCache = nil
        configuration.httpCookieStorage = nil
        configuration.urlCredentialStorage = nil
        configuration.timeoutIntervalForResource = 25
        let session = URLSession(configuration: configuration, delegate: RemoteRedirectPolicy(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (bytes, response) = try await session.bytes(for: request)
        var data = Data()
        for try await byte in bytes { data.append(byte); if data.count > 2_000_000 { throw LocalFailure.message("内容过大，请在电脑查看。") } }
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let error = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            throw RemoteFailure(status: (response as? HTTPURLResponse)?.statusCode ?? 0, message: error?["error"] as? String ?? "电脑连接失败，请检查网络后刷新。")
        }
        return try JSONDecoder().decode(T.self, from: data)
    }
}
private final class RemoteRedirectPolicy: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}
struct RemoteFailure: LocalizedError { let status: Int; let message: String; var errorDescription: String? { message } }

struct RemotePendingSend: Codable, Equatable, Identifiable {
    var modelChoice: RemoteModelChoice? = nil
    var expectedRunID: String? = nil
    let id: String; let text: String; let chatID: String?; let projectPath: String?
    let target: RemoteTargetIdentity?
    func bound(to device: RemoteDevice) -> Self { Self(modelChoice: modelChoice, expectedRunID: expectedRunID, id: id, text: text, chatID: chatID, projectPath: projectPath, target: RemoteTargetIdentity(device)) }
    func arguments(for device: RemoteDevice) throws -> [String: Any] {
        guard target == RemoteTargetIdentity(device) else { throw LocalFailure.message("请先确认这条指令所属的电脑，不能向其他电脑重试。") }
        var result: [String: Any] = ["text": text]
        if let modelChoice { result["model_choice"] = modelChoice.arguments }
        if let expectedRunID { result["expected_run_id"] = expectedRunID }
        if let chatID { result["chat_id"] = chatID }; if let projectPath { result["project_path"] = projectPath }
        return result
    }
}

private struct RemoteEnvelope<T: Decodable>: Decodable { let result: T }
struct RemoteAck: Decodable {}
struct RemoteSent: Decodable {
    let chat: RemoteChat
    var delivery: String? = nil
    var recoveryNotice: String? {
        delivery == "recovered" ? "已找回电脑保存的这条指令，没有重复发送。指令已保存不代表执行已完成，请查看任务内容与状态。" : nil
    }
}

struct RemoteAccountProfile: Codable, Equatable {
    let owner: String; let email: String; let relay: URL
    var scope: String? = nil
    var account: String { if scope == "cloud" { return "cloud-account-\(relay.host ?? "")-\(relay.port ?? 443)-\(owner)" }; return RemoteDevice(id: "", name: "", relay: relay, owner: owner).account }
}
struct RemoteLoginAttempt: Codable {
    let id: String; let verification_url: URL; let code: String; let expires: Double
    var relay: URL { URL(string: "/", relativeTo: verification_url)!.absoluteURL }
    var credentialAccount: String { "remote-login-\(id)" }
}

@MainActor final class RemoteStore: ObservableObject {
    @Published var devices: [RemoteDevice] = []
    @Published var online: [String: Bool] = [:]
    @Published var overviews: [String: RemoteOverview] = [:]
    @Published var error: String?
    @Published var refreshing = false
    @Published var profile: RemoteAccountProfile?
    @Published var login: RemoteLoginAttempt?
    @Published var signingIn = false
    private let defaults: UserDefaults
    private let cloudOnly: Bool
    private var profileKey: String { cloudOnly ? "cloud-profile" : "remote-profile" }
    private var loginKey: String { cloudOnly ? "cloud-login" : "remote-login" }
    init(defaults suppliedDefaults: UserDefaults? = nil, cloudOnly: Bool = false) {
        let testing = ProcessInfo.processInfo.arguments.contains("--ui-testing")
        let testSuite = cloudOnly ? "PotatoCloudUITests" : "PotatoRemoteUITests"
        let defaults = suppliedDefaults ?? (testing ? UserDefaults(suiteName: testSuite)! : .standard)
        if testing && ProcessInfo.processInfo.arguments.contains("--reset") { defaults.removePersistentDomain(forName: testSuite) }
        self.defaults = defaults; self.cloudOnly = cloudOnly
        RemoteDraftRepository.resetTestStorage()
        if let data = defaults.data(forKey: profileKey) { profile = try? JSONDecoder().decode(RemoteAccountProfile.self, from: data) }
        if let data = defaults.data(forKey: loginKey) { login = try? JSONDecoder().decode(RemoteLoginAttempt.self, from: data) }
        if let data = defaults.data(forKey: "remote-devices") { devices = (try? JSONDecoder().decode([RemoteDevice].self, from: data)) ?? [] }
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--remote-preview") {
            devices = [RemoteDevice(id: "00000000-0000-4000-8000-000000000001", name: "MacBook Pro", relay: URL(string: "http://127.0.0.1:18999")!), RemoteDevice(id: "00000000-0000-4000-8000-000000000002", name: "Mac mini", relay: URL(string: "http://127.0.0.1:18999")!)]
            if ProcessInfo.processInfo.arguments.contains("--remote-process-preview") {
                devices = devices.map { RemoteDevice(id: $0.id, name: $0.name, relay: URL(string: "http://127.0.0.1:19014")!) }
            }
            if ProcessInfo.processInfo.arguments.contains("--remote-draft-preview") {
                devices = devices.map { RemoteDevice(id: $0.id, name: $0.name, relay: URL(string: "http://127.0.0.1:19013")!, owner: "draft-fixture-account") }
                if ProcessInfo.processInfo.arguments.contains("--remote-legacy-draft-preview"), ProcessInfo.processInfo.arguments.contains("--reset") {
                    let key = "remote-draft-\(devices[0].account)-new"
                    defaults.set("旧版草稿：先核对目标电脑", forKey: key)
                    let request = RemotePendingSend(id: "00000000-0000-4000-8000-000000000099", text: "旧版待确认指令", chatID: nil, projectPath: nil, target: nil)
                    defaults.set(try? JSONEncoder().encode(request), forKey: key + "-pending")
                }
            }
            for device in devices { online[device.id] = true }
            overviews[devices[0].id] = RemoteOverview(chats: [
                RemoteChat(id: "fixture-review", session_id: "fixture-review", name: "检查 iPhone 远程控制的实现", status: "running", pinned: true, project_path: "/fixture/Potato"),
                RemoteChat(id: "fixture-sidebar", session_id: "fixture-sidebar", name: "调整侧栏和项目导航", status: "completed", pinned: false, project_path: "/fixture/Potato"),
                RemoteChat(id: "fixture-log", session_id: "fixture-log", name: "查看今天的运行日志", status: "completed", pinned: false, project_path: nil),
                RemoteChat(id: "fixture-doc", session_id: "fixture-doc", name: "整理项目说明文档", status: "completed", pinned: false, project_path: "/fixture/lifeProjects")
            ], projects: [RemoteProject(path: "/fixture/Potato", name: "Potato"), RemoteProject(path: "/fixture/dynamo", name: "dynamo"), RemoteProject(path: "/fixture/lifeProjects", name: "lifeProjects")])
            overviews[devices[1].id] = RemoteOverview(chats: [], projects: [RemoteProject(path: "/fixture/notes", name: "个人笔记")])
            if ProcessInfo.processInfo.arguments.contains("--sidebar-long-list-preview"), let overview = overviews[devices[0].id] {
                let extra = (1...35).map { RemoteChat(id: "gesture-history-\($0)", session_id: "gesture-history-\($0)", name: "手势测试会话 \($0)", status: "completed", pinned: false, project_path: nil) }
                overviews[devices[0].id] = RemoteOverview(chats: overview.chats + extra, projects: overview.projects)
            }
        }
        #endif
    }
    func beginLogin(relay: String) async -> URL? {
        guard !signingIn, profile == nil else { return nil }; signingIn = true; defer { signingIn = false }
        do {
            guard let url = URL(string: relay.trimmingCharacters(in: .whitespacesAndNewlines)), url.scheme == "https", url.host != nil, url.user == nil, url.password == nil, url.query == nil, url.fragment == nil, url.path.isEmpty || url.path == "/" else { throw LocalFailure.message("请输入 HTTPS 服务根地址。") }
            let token = (UUID().uuidString + UUID().uuidString).replacingOccurrences(of: "-", with: "").lowercased()
            let attempt: RemoteLoginAttempt = try await RemoteService.request(url.appendingPathComponent("v1/remote/auth/start"), body: ["client_token": token, "role": cloudOnly ? "cloud" : "phone", "name": cloudOnly ? "我的 iPhone · 云端模型" : "我的 iPhone"])
            guard attempt.verification_url.scheme == url.scheme, attempt.verification_url.host == url.host, attempt.verification_url.port == url.port, attempt.verification_url.path == "/v1/remote/auth/authorize", attempt.verification_url.user == nil, attempt.verification_url.password == nil, attempt.verification_url.fragment == nil, URLComponents(url: attempt.verification_url, resolvingAgainstBaseURL: false)?.queryItems == [URLQueryItem(name: "id", value: attempt.id)], UUID(uuidString: attempt.id) != nil else { throw LocalFailure.message("登录服务返回了无效地址。") }
            try SecureToken.save(token, account: attempt.credentialAccount)
            defaults.set(try JSONEncoder().encode(attempt), forKey: loginKey); login = attempt; error = nil
            return attempt.verification_url
        } catch { self.error = error.localizedDescription; return nil }
    }
    func cancelLogin() {
        if let login { try? SecureToken.save("", account: login.credentialAccount) }
        defaults.removeObject(forKey: loginKey); login = nil
    }
    func pollLogin() async throws {
        guard let login else { return }
        if login.expires / 1000 < Date().timeIntervalSince1970 { cancelLogin(); throw LocalFailure.message("登录已过期，请重新开始。") }
        struct Reply: Decodable { let status: String; let owner: String?; let email: String? }
        let secret = SecureToken.read(account: login.credentialAccount)
        let reply: Reply = try await RemoteService.request(login.relay.appendingPathComponent("v1/remote/auth/poll"), body: ["id": login.id, "client_token": secret])
        guard self.login?.id == login.id, SecureToken.read(account: login.credentialAccount) == secret else { return }
        guard reply.status == "authorized", let owner = reply.owner, let email = reply.email, owner.count == 64, owner.allSatisfy({ $0.isHexDigit }) else { return }
        let value = RemoteAccountProfile(owner: owner, email: email, relay: login.relay, scope: cloudOnly ? "cloud" : nil)
        try SecureToken.save("\(owner).\(login.id).\(secret)", account: value.account)
        defaults.set(try JSONEncoder().encode(value), forKey: profileKey); profile = value; cancelLogin(); error = nil
    }
    func logout() async {
        guard let profile else { return }
        do {
            do {
                let _: RemoteAck = try await RemoteService.request(profile.relay.appendingPathComponent("v1/remote/account/logout"), token: SecureToken.read(account: profile.account), body: [:])
            } catch let failure as RemoteFailure where failure.status == 401 {
                // Already expired/revoked: allow local cleanup and a fresh login.
            }
            try SecureToken.save("", account: profile.account)
            devices.removeAll { $0.owner == profile.owner }; self.profile = nil; defaults.removeObject(forKey: profileKey); error = nil
        } catch { self.error = error.localizedDescription }
    }
    func revoke(_ device: RemoteDevice) async {
        guard let profile, device.owner == profile.owner else { return }
        do {
            let _: RemoteAck = try await RemoteService.request(profile.relay.appendingPathComponent("v1/remote/account/revoke"), token: SecureToken.read(account: profile.account), body: ["device_id": device.id])
            devices.removeAll { $0.id == device.id }; online.removeValue(forKey: device.id); overviews.removeValue(forKey: device.id); error = nil
        } catch { self.error = error.localizedDescription }
    }
    func pair(_ code: String) async {
        do {
            let device = try await RemoteService.pair(code)
            devices.removeAll { $0.id == device.id && $0.relay == device.relay }; devices.append(device)
            defaults.set(try JSONEncoder().encode(devices.filter { $0.owner == nil }), forKey: "remote-devices")
            error = nil; await refresh()
        } catch { self.error = error.localizedDescription }
    }
    func forget(_ device: RemoteDevice) {
        guard device.owner == nil else { return }
        do { try SecureToken.save("", account: device.account); devices.removeAll { $0 == device }; defaults.set(try JSONEncoder().encode(devices.filter { $0.owner == nil }), forKey: "remote-devices"); online.removeValue(forKey: device.id); overviews.removeValue(forKey: device.id) }
        catch { self.error = error.localizedDescription }
    }
    func refresh() async {
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--remote-preview") { return }
        #endif
        guard !refreshing else { return }; refreshing = true; defer { refreshing = false }
        do {
            try await pollLogin()
            if cloudOnly { return }
            if let profile {
                struct Listing: Decodable { struct Device: Decodable { let id: String; let name: String }; let devices: [Device] }
                let listing: Listing = try await RemoteService.request(profile.relay.appendingPathComponent("v1/remote/account/devices"), token: SecureToken.read(account: profile.account))
                let values = listing.devices.map { RemoteDevice(id: $0.id, name: $0.name, relay: profile.relay, owner: profile.owner) }
                guard self.profile?.owner == profile.owner, self.profile?.relay == profile.relay else { return }
                devices = devices.filter { $0.owner == nil } + values
            }
        } catch {
            if !Task.isCancelled { self.error = error.localizedDescription; for device in devices where device.owner != nil { online[device.id] = false } }
            return
        }
        for device in devices {
            if Task.isCancelled { return }
            do {
                let connected = try await RemoteService.status(device)
                online[device.id] = connected
                if connected { overviews[device.id] = try await RemoteService.rpc(device, op: "overview") }
            } catch { if Task.isCancelled { return }; online[device.id] = false; self.error = "\(device.name)：\(error.localizedDescription)" }
        }
    }
}
