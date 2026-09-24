import Foundation
import Security
import UniformTypeIdentifiers
import PDFKit

struct LocalStorage {
    let root: URL
    init(root: URL? = nil) {
        self.root = root ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Potato", isDirectory: true)
    }
    var stateURL: URL { root.appendingPathComponent("workspace.json") }
    func prepare() throws { try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true) }
    func load() throws -> SavedWorkspace? {
        guard FileManager.default.fileExists(atPath: stateURL.path) else { return nil }
        let saved = try JSONDecoder().decode(SavedWorkspace.self, from: Data(contentsOf: stateURL))
        guard (1...2).contains(saved.version) else { throw LocalFailure.message(L10n.tr("本地记录来自更新版本，请先更新应用。")) }
        return saved
    }
    func save(_ workspace: SavedWorkspace) throws {
        try prepare()
        let data = try JSONEncoder().encode(workspace)
        try data.write(to: stateURL, options: [.atomic, .completeFileProtectionUnlessOpen])
    }
    func url(for attachment: Attachment) -> URL { root.appendingPathComponent("Attachments").appendingPathComponent(attachment.filename) }
    func pruneUnreferencedAttachments(in workspace: SavedWorkspace) throws {
        var referenced = Set<String>()
        referenced.formUnion((workspace.library ?? []).map { $0.attachment.filename })
        for chat in workspace.conversations {
            referenced.formUnion(chat.pendingAttachments.map(\.filename))
            for message in chat.messages {
                referenced.formUnion(message.attachments.map(\.filename))
                for version in message.versions ?? [] { referenced.formUnion((version.attachments ?? []).map(\.filename)) }
            }
        }
        let directory = root.appendingPathComponent("Attachments", isDirectory: true)
        guard FileManager.default.fileExists(atPath: directory.path) else { return }
        for file in try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.isRegularFileKey]) {
            // Only this app's generated files; never directories, unexpected files, or external URLs.
            guard !referenced.contains(file.lastPathComponent), UUID(uuidString: file.deletingPathExtension().lastPathComponent) != nil,
                  try file.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true else { continue }
            try FileManager.default.removeItem(at: file)
        }
    }
    func importFile(_ source: URL) throws -> Attachment {
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        let resources = try source.resourceValues(forKeys: [.fileSizeKey, .contentTypeKey, .isRegularFileKey])
        guard resources.isRegularFile == true else { throw LocalFailure.message(L10n.tr("请选择普通文件。")) }
        guard (resources.fileSize ?? Int.max) <= 10 * 1_024 * 1_024 else { throw LocalFailure.message(L10n.tr("单个附件最多 10 MB，请选择较小的文件。")) }
        let data = try Data(contentsOf: source)
        return try importData(data, name: source.lastPathComponent, type: resources.contentType ?? .data)
    }
    func importData(_ data: Data, name: String, type: UTType) throws -> Attachment {
        guard data.count <= 10 * 1_024 * 1_024 else { throw LocalFailure.message(L10n.tr("单个附件最多 10 MB。")) }
        guard type.conforms(to: .image) || type.conforms(to: .text) || type == .pdf else { throw LocalFailure.message(L10n.tr("支持图片、PDF 和文本文件。")) }
        var data = data, name = name, type = type
        if type.conforms(to: .image) {
            data = try ImageImport.jpeg(from: data); type = .jpeg
            name = URL(fileURLWithPath: name).deletingPathExtension().lastPathComponent + ".jpg"
        }
        var attachment = Attachment(name: name, filename: UUID().uuidString + "." + (type.preferredFilenameExtension ?? "bin"), type: type.preferredMIMEType ?? "application/octet-stream", size: data.count)
        let destination = url(for: attachment)
        try FileManager.default.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: destination, options: [.atomic, .completeFileProtectionUnlessOpen])
        if type.conforms(to: .text) { attachment.extractedText = String(data: data, encoding: .utf8) }
        if type == .pdf { attachment.extractedText = PDFDocument(data: data)?.string }
        if let text = attachment.extractedText, text.count > 60_000 {
            try? FileManager.default.removeItem(at: destination)
            throw LocalFailure.message(L10n.tr("\(name) 的文字超过 60,000 字，请拆分后导入。"))
        }
        if !attachment.isImage && (attachment.extractedText?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true) {
            try? FileManager.default.removeItem(at: destination)
            throw LocalFailure.message(L10n.tr("无法提取 \(name) 的文字。扫描版 PDF 请先转换成文字或图片。"))
        }
        return attachment
    }
    func importArtifact(_ artifact: SandboxArtifact) throws -> Attachment {
        let suffix = URL(fileURLWithPath: artifact.name).pathExtension.lowercased()
        guard ["png", "jpg", "jpeg", "pdf", "csv", "txt", "md", "docx", "xlsx", "pptx"].contains(suffix),
              artifact.name == URL(fileURLWithPath: artifact.name).lastPathComponent,
              let data = Data(base64Encoded: artifact.base64), data.count <= 2_000_000,
              let type = UTType(filenameExtension: suffix) else { throw LocalFailure.message(L10n.tr("计算产物格式或大小不支持。")) }
        let file = Attachment(name: artifact.name, filename: UUID().uuidString + "." + suffix, type: type.preferredMIMEType ?? "application/octet-stream", size: data.count)
        let destination = url(for: file)
        try FileManager.default.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: destination, options: [.atomic, .completeFileProtectionUnlessOpen]); return file
    }
}
enum LocalFailure: LocalizedError {
    case message(String)
    var errorDescription: String? { if case .message(let text) = self { return text }; return nil }
}
enum SecureToken {
    private static let service = "com.potato.iphone.connection"
    static func read(account: String = "token") -> String {
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account, kSecReturnData as String: true]
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess, let data = result as? Data else { return "" }
        return String(data: data, encoding: .utf8) ?? ""
    }
    static func save(_ token: String, account: String = "token") throws {
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
        if token.isEmpty { SecItemDelete(query as CFDictionary); return }
        let update = [kSecValueData as String: Data(token.utf8)]
        var status = SecItemUpdate(query as CFDictionary, update as CFDictionary)
        if status == errSecItemNotFound {
            var insert = query.merging(update) { _, new in new }
            insert[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(insert as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            if status == errSecMissingEntitlement { throw LocalFailure.message(L10n.tr("当前应用缺少 Keychain 签名权限，请安装正确签名的构建。")) }
            throw LocalFailure.message(L10n.tr("无法保存连接凭据（\(status)），请解锁设备后重试。"))
        }
    }
}
