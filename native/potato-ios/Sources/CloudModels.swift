import SwiftUI

struct CloudModelOption: Identifiable, Equatable {
    let id: String
    let name: String
    let enabled: Bool
    var displayName: String { ModelNaming.displayName(id: id, name: name) }
}
struct CloudModelProvider: Identifiable {
    let id: String
    let name: String
    let unavailable: Bool
    let models: [CloudModelOption]
}

/// Admin-only edits to the Worker's shared cloud model list. Keys never leave the Worker.
enum CloudModelAdmin {
    static func request(settings: ConnectionSettings, token: String, path: String, method: String = "GET", body: Data? = nil) throws -> URLRequest {
        var request = try LocalModelService.catalogRequest(settings: settings, token: token)
        request.url = request.url?.appendingPathComponent(path)
        request.httpMethod = method
        if let body { request.httpBody = body; request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        return request
    }
    static func send(_ request: URLRequest, configuration: URLSessionConfiguration) async throws -> Data {
        let session = URLSession(configuration: configuration, delegate: ModelRedirectPolicy(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw LocalFailure.message(L10n.tr("模型列表响应无效。")) }
        switch http.statusCode {
        case 200: guard data.count <= 1_000_000 else { throw LocalFailure.message(L10n.tr("模型列表响应无效。")) }; return data
        case 401: throw AuthorizationFailure()
        case 403: throw LocalFailure.message(L10n.tr("只有管理员可以修改云端模型。"))
        case 409: throw LocalFailure.message(L10n.tr("模型列表已在其他设备上修改，请重试。"))
        case 400: throw LocalFailure.message(L10n.tr("这个模型当前不可用。"))
        default: throw LocalFailure.message(L10n.tr("暂时无法修改模型（\(http.statusCode)）"))
        }
    }
    static func available(settings: ConnectionSettings, token: String, configuration: URLSessionConfiguration) async throws -> [CloudModelProvider] {
        let data = try await send(request(settings: settings, token: token, path: "available"), configuration: configuration)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any], let providers = json["providers"] as? [[String: Any]] else {
            throw LocalFailure.message(L10n.tr("模型列表响应无效。"))
        }
        return providers.compactMap { provider in
            guard let id = provider["id"] as? String, let models = provider["models"] as? [[String: Any]] else { return nil }
            return CloudModelProvider(id: id, name: provider["name"] as? String ?? id, unavailable: provider["unavailable"] as? Bool == true, models: models.compactMap { model in
                guard let id = model["id"] as? String else { return nil }
                return CloudModelOption(id: id, name: model["name"] as? String ?? id, enabled: model["enabled"] as? Bool == true)
            })
        }
    }
    static func save(settings: ConnectionSettings, token: String, models: [String], defaultModel: String, revision: Int, configuration: URLSessionConfiguration) async throws -> LocalModelCatalog {
        let body = try JSONSerialization.data(withJSONObject: ["models": models, "default_model": defaultModel, "revision": revision])
        let data = try await send(request(settings: settings, token: token, path: "enabled", method: "PUT", body: body), configuration: configuration)
        return try LocalModelService.decode(data, settings: settings)
    }
}

extension WorkspaceStore {
    func saveCloudModels(_ models: [String], defaultModel: String) async throws {
        let configuration = settings, token = connectionToken
        guard let revision = configuration.currentCatalog?.revision else { throw LocalFailure.message(L10n.tr("模型列表响应无效。")) }
        do {
            let catalog = try await CloudModelAdmin.save(settings: configuration, token: token, models: models, defaultModel: defaultModel, revision: revision, configuration: streamConfiguration)
            guard configuration.serviceIdentity == settings.serviceIdentity else { throw LocalFailure.message(L10n.tr("连接已改变，请重新读取模型列表。")) }
            installLocalModelCatalog(catalog); persist()
        } catch is AuthorizationFailure { authorizationExpired = true; throw AuthorizationFailure() }
    }
    func availableCloudModels() async throws -> [CloudModelProvider] {
        do { return try await CloudModelAdmin.available(settings: settings, token: connectionToken, configuration: streamConfiguration) }
        catch is AuthorizationFailure { authorizationExpired = true; throw AuthorizationFailure() }
    }
}

struct CloudModelsView: View {
    @ObservedObject var store: WorkspaceStore
    @State private var adding = false
    @State private var busy = false
    @State private var failure: String?
    private var catalog: LocalModelCatalog? { store.settings.currentCatalog }
    private var models: [LocalModelEntry] { catalog?.models ?? [] }
    private var defaultModel: String? { catalog?.defaultModel }
    var body: some View {
        List {
            Section(L10n.tr("已启用")) {
                ForEach(models) { model in
                    Button { save(models.map(\.id), defaultModel: model.id) } label: {
                        HStack {
                            Text(model.displayName).foregroundStyle(Palette.ink)
                            Spacer()
                            if model.id == defaultModel { Text(L10n.tr("默认")).foregroundStyle(Palette.secondary) }
                        }
                    }.disabled(busy || model.id == defaultModel)
                        .deleteDisabled(busy || model.id == defaultModel || models.count == 1)
                        .accessibilityIdentifier("cloud-model-\(model.id)")
                }.onDelete { offsets in
                    guard let defaultModel else { return }
                    save(models.enumerated().filter { !offsets.contains($0.offset) }.map(\.element.id), defaultModel: defaultModel)
                }
            }
        }
        .navigationTitle(L10n.tr("云端模型")).navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                if busy { ProgressView() }
                else { Button { adding = true } label: { Image(systemName: "plus") }.accessibilityLabel(L10n.tr("添加模型")).accessibilityIdentifier("cloud-model-add") }
            }
        }
        .sheet(isPresented: $adding) { CloudModelPicker(store: store) }
        .alert(L10n.tr("未能修改模型"), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) { Button(L10n.tr("好")) { failure = nil } } message: { Text(failure ?? "") }
        .task { try? await store.reloadLocalModels(force: true) }
    }
    private func save(_ ids: [String], defaultModel: String) {
        busy = true
        Task {
            defer { busy = false }
            do { try await store.saveCloudModels(ids, defaultModel: defaultModel) }
            catch {
                failure = error.localizedDescription
                try? await store.reloadLocalModels(force: true)
            }
        }
    }
}

private struct CloudModelPicker: View {
    @ObservedObject var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    @State private var providers: [CloudModelProvider]?
    @State private var search = ""
    @State private var saving: String?
    @State private var failure: String?
    private var enabled: [String] { store.settings.currentCatalog?.models.map(\.id) ?? [] }
    var body: some View {
        NavigationStack {
            List {
                if let providers {
                    ForEach(providers) { provider in
                        let models = provider.models.filter { search.isEmpty || $0.displayName.localizedCaseInsensitiveContains(search) || $0.id.localizedCaseInsensitiveContains(search) }
                        if !models.isEmpty || provider.unavailable {
                            Section {
                                ForEach(models) { model in row(model) }
                            } header: { Text(provider.name) } footer: {
                                if provider.unavailable { Text(L10n.tr("暂时无法读取这个服务的模型。")) }
                            }
                        }
                    }
                } else if failure == nil {
                    ProgressView(L10n.tr("正在读取模型…")).frame(maxWidth: .infinity)
                }
                if let failure { Text(failure).foregroundStyle(.red) }
            }
            .searchable(text: $search, prompt: L10n.tr("搜索模型"))
            .navigationTitle(L10n.tr("添加模型")).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { dismiss() } } }
            .task { await load() }
        }
    }
    private func row(_ model: CloudModelOption) -> some View {
        let added = enabled.contains(model.id)
        return Button { add(model.id) } label: {
            HStack {
                Text(model.displayName).foregroundStyle(Palette.ink)
                Spacer()
                if saving == model.id { ProgressView() }
                else { Image(systemName: added ? "checkmark" : "plus.circle").foregroundStyle(added ? Palette.secondary : Palette.ink) }
            }
        }.disabled(added || saving != nil).accessibilityIdentifier("cloud-model-option-\(model.id)")
    }
    private func load() async {
        do { providers = try await store.availableCloudModels(); failure = nil }
        catch { failure = error.localizedDescription }
    }
    private func add(_ id: String) {
        guard let defaultModel = store.settings.currentCatalog?.defaultModel else { return }
        saving = id; failure = nil
        Task {
            defer { saving = nil }
            do { try await store.saveCloudModels(enabled + [id], defaultModel: defaultModel) }
            catch { failure = error.localizedDescription; try? await store.reloadLocalModels(force: true) }
        }
    }
}
