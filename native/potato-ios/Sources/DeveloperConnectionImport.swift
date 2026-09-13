#if DEBUG
import Foundation

// Local developer provisioning. Never included in a Release build or an app bundle resource.
enum DeveloperConnectionImport {
    private struct Connection: Decodable { var endpoint: String; var model: String; var token: String }
    @MainActor static func apply(to store: WorkspaceStore) {
        guard ProcessInfo.processInfo.arguments.contains("--import-desktop-connection") else { return }
        let file = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("PotatoConnectionImport.json")
        guard FileManager.default.fileExists(atPath: file.path) else { return }
        defer { try? FileManager.default.removeItem(at: file) }
        do {
            let data = try Data(contentsOf: file)
            guard data.count < 65536 else { throw LocalFailure.message("配置文件过大。") }
            let value = try JSONDecoder().decode(Connection.self, from: data)
            var settings = store.settings
            settings.endpoint = value.endpoint; settings.model = value.model; settings.demo = false; settings.modelCatalog = nil
            guard settings.validatedURL != nil, !value.model.isEmpty, !value.token.isEmpty else { throw LocalFailure.message("配置不完整。") }
            try SecureToken.save(value.token)
            store.settings = settings; store.persist()
        } catch let error as LocalFailure { store.error = error.localizedDescription }
        catch { store.error = "未能读取本机模型连接，请重新生成配置文件。" }
    }
}
#endif
