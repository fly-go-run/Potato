import SwiftUI

extension RemoteAccountProfile {
    var cloudEndpoint: URL? {
        guard relay.scheme == "https", relay.host != nil, relay.user == nil, relay.password == nil,
              relay.query == nil, relay.fragment == nil, relay.path.isEmpty || relay.path == "/" else { return nil }
        return relay.appendingPathComponent("v1/chat/completions")
    }
}
extension ConnectionSettings {
    var connectionToken: String {
        guard let cloudAccount else { return SecureToken.read() }
        // Never send an account session to an edited endpoint or a different origin.
        guard let endpoint = cloudAccount.cloudEndpoint, validatedURL == endpoint else { return "" }
        return SecureToken.read(account: cloudAccount.account)
    }
}
extension WorkspaceStore {
    func connectCloud(_ account: RemoteAccountProfile, configuration: URLSessionConfiguration = .ephemeral) async throws {
        guard let endpoint = account.cloudEndpoint else { throw LocalFailure.message("云端服务地址无效。") }
        let previous = settings
        var next = settings
        next.cloudAccount = account; next.endpoint = endpoint.absoluteString; next.modelCatalog = nil
        let credential = next.connectionToken
        guard !credential.isEmpty else { throw LocalFailure.message("请先登录 Cloudflare。") }
        let catalog = try await LocalModelService.catalog(settings: next, token: credential, configuration: configuration)
        try Task.checkCancellation()
        guard settings == previous, credential == next.connectionToken else { throw LocalFailure.message("连接已改变，请重新连接云端模型。") }
        guard let model = catalog.models.first(where: { $0.id == catalog.defaultModel }) ?? catalog.models.first else { throw LocalFailure.message("账号已登录，云端尚未配置可用模型。") }
        next.modelCatalog = catalog; next.model = model.id; next.demo = false
        settings = next
        installLocalModelCatalog(catalog)
        // A conversation's explicit choice is retained only if still part of this service.
        for i in conversations.indices {
            if let choice = conversations[i].modelChoice,
               choice.endpoint != next.serviceIdentity || !catalog.models.contains(where: { $0.id == choice.model }) {
                conversations[i].modelChoice = nil
            }
        }
        persist()
    }
}

struct CloudAccountView: View {
    @ObservedObject var store: WorkspaceStore
    @StateObject private var remote = RemoteStore(cloudOnly: true)
    let connected: () -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var phase
    @State private var connecting = false
    @State private var issue: String?
    @State private var connectTask: Task<Void, Never>?
    private let defaultRelay = "https://potato-remote.recodex.top"
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Label("Cloudflare 账号", systemImage: "person.crop.circle").font(.headline)
                    Text("登录获准的账号后，自动加载云端模型。模型密钥保存在服务器，电脑无需在线。").font(.subheadline).foregroundStyle(Palette.secondary)
                }
                if let account = remote.profile {
                    Section {
                        Text(account.email).textSelection(.enabled)
                        Button { connect(account) } label: { HStack { if connecting { ProgressView() }; Text("使用此账号的云端模型") } }
                            .disabled(connecting).accessibilityIdentifier("cloud-connect-account")
                        Button("退出 Cloudflare", role: .destructive) {
                            Task {
                                await remote.logout()
                                if remote.profile == nil { store.settings.modelCatalog = nil; store.persist() }
                            }
                        }.disabled(connecting)
                    }
                } else if let login = remote.login {
                    Section("核对登录验证码") {
                        Text(login.code).font(.title.monospaced()).textSelection(.enabled)
                        Text("在浏览器中完成登录，核对验证码并确认，然后返回 Potato。")
                        Button("打开登录页面") { UIApplication.shared.open(login.verification_url) }
                        Button("我已登录，刷新") { Task { await finishLogin() } }
                        Button("重新开始") { remote.cancelLogin() }
                    }
                } else {
                    Section {
                        Button {
                            Task {
                                issue = nil
                                if let url = await remote.beginLogin(relay: store.settings.cloudAccount?.relay.absoluteString ?? defaultRelay) { await UIApplication.shared.open(url) }
                            }
                        } label: { HStack { if remote.signingIn { ProgressView() }; Text("继续使用 Cloudflare") } }
                            .disabled(remote.signingIn).accessibilityIdentifier("cloud-sign-in")
                    }
                }
                if let issue = issue ?? remote.error { Section { Text(issue).foregroundStyle(.red).accessibilityIdentifier("cloud-connection-error") } }
                Section { Text("仅限所有者授权的邮箱。登录不会自动开启电脑远程访问，对话与附件仍保存在 iPhone。").font(.footnote).foregroundStyle(Palette.secondary) }
            }.navigationTitle("云端模型").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("完成") { dismiss() }.disabled(connecting || remote.signingIn) } }
        }
        .task(id: phase == .active && remote.login != nil) {
            while phase == .active && remote.login != nil && !Task.isCancelled {
                await finishLogin()
                do { try await Task.sleep(for: .seconds(2)) } catch { return }
            }
        }
        .onDisappear { connectTask?.cancel() }
    }
    private func finishLogin() async {
        do { try await remote.pollLogin(); if let account = remote.profile { connect(account) } }
        catch { if !Task.isCancelled { issue = error.localizedDescription } }
    }
    private func connect(_ account: RemoteAccountProfile) {
        guard !connecting else { return }; connecting = true; issue = nil
        connectTask = Task { @MainActor in
            defer { connecting = false }
            do { try await store.connectCloud(account); guard !Task.isCancelled else { return }; dismiss(); connected() }
            catch { if !Task.isCancelled { issue = error.localizedDescription } }
        }
    }
}
