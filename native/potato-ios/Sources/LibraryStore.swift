import Foundation
import UniformTypeIdentifiers
import CryptoKit
import PDFKit
import ImageIO

struct LibrarySource: Codable, Hashable {
    let conversationID: UUID
    let messageID: UUID
}

struct LibraryItem: Identifiable, Codable, Equatable {
    var id: UUID { attachment.id }
    var attachment: Attachment
    var digest: String?
    var addedAt = Date()
    var sources: [LibrarySource] = []
    var deletedAt: Date?
    var format: String {
        let suffix = (attachment.name as NSString).pathExtension
        return suffix.isEmpty ? (attachment.isImage ? L10n.tr("图片") : L10n.tr("文件")) : suffix.uppercased()
    }
    var sizeLabel: String {
        if attachment.size < 1_024 { return "\(attachment.size) B" }
        return ByteCountFormatter.string(fromByteCount: Int64(attachment.size), countStyle: .file)
    }
    var dateLabel: String {
        if Calendar.current.isDateInToday(addedAt) { return L10n.tr("今天") }
        if Calendar.current.isDateInYesterday(addedAt) { return L10n.tr("昨天") }
        let formatter = DateFormatter(); formatter.locale = AppLocalization.shared.locale; formatter.dateFormat = L10n.tr("M月d日")
        return formatter.string(from: addedAt)
    }
    func matches(_ query: String) -> Bool {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return term.isEmpty || attachment.name.localizedStandardContains(term) || (attachment.extractedText?.localizedStandardContains(term) ?? false)
    }
    func excerpt(for query: String) -> String? {
        let term = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !term.isEmpty, let text = attachment.extractedText,
              let range = text.range(of: term, options: [.caseInsensitive, .diacriticInsensitive]) else { return nil }
        let start = text.index(range.lowerBound, offsetBy: -18, limitedBy: text.startIndex) ?? text.startIndex
        return String(text[start...].prefix(90)).replacingOccurrences(of: "\n", with: " ")
    }
}

extension LocalStorage {
    static let libraryTypes: [UTType] = [.image, .pdf, .text] + ["docx", "xlsx", "pptx", "doc", "xls", "ppt"].compactMap { UTType(filenameExtension: $0) }

    func importLibraryFile(_ source: URL) throws -> Attachment {
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        let info = try source.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey, .contentTypeKey])
        guard info.isRegularFile == true, (info.fileSize ?? Int.max) <= 10 * 1_024 * 1_024 else {
            throw LocalFailure.message(L10n.tr("请选择不超过 10 MB 的普通文件。"))
        }
        return try importLibraryData(Data(contentsOf: source), name: source.lastPathComponent, type: info.contentType ?? UTType(filenameExtension: source.pathExtension) ?? .data)
    }

    /// Library owns the original bytes. Chat conversion never overwrites this file.
    func importLibraryData(_ data: Data, name: String, type: UTType) throws -> Attachment {
        guard data.count <= 10 * 1_024 * 1_024 else { throw LocalFailure.message(L10n.tr("单个资料最多 10 MB。")) }
        guard Self.libraryTypes.contains(where: { type.conforms(to: $0) }) else { throw LocalFailure.message(L10n.tr("支持图片、PDF、文本和 Office 文档。")) }
        var type = type
        if type.conforms(to: .image) {
            guard let source = CGImageSourceCreateWithData(data as CFData, nil), CGImageSourceGetCount(source) > 0 else {
                throw LocalFailure.message(L10n.tr("图片无法读取，请重新选择文件。"))
            }
            if let detected = CGImageSourceGetType(source) { type = UTType(detected as String) ?? type }
        }
        var file = Attachment(name: Self.safeLibraryName(name), filename: UUID().uuidString + "." + (type.preferredFilenameExtension ?? "bin"), type: type.preferredMIMEType ?? "application/octet-stream", size: data.count)
        if type.conforms(to: .text) { file.extractedText = String(data: data, encoding: .utf8).map { String($0.prefix(60_001)) } }
        if type == .pdf { file.extractedText = PDFDocument(data: data)?.string.map { String($0.prefix(60_001)) } }
        let destination = url(for: file)
        try FileManager.default.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: destination, options: [.atomic, .completeFileProtectionUnlessOpen])
        return file
    }
    static func safeLibraryName(_ value: String) -> String {
        let name = value.components(separatedBy: CharacterSet(charactersIn: "/\\\0\r\n")).joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        return name.isEmpty ? L10n.tr("未命名文件") : String(name.prefix(180))
    }
    func libraryDigest(_ file: Attachment) throws -> String {
        SHA256.hash(data: try Data(contentsOf: url(for: file))).map { String(format: "%02x", $0) }.joined()
    }
    func libraryChatLimitation(_ file: Attachment) -> String? {
        guard FileManager.default.fileExists(atPath: url(for: file).path) else { return L10n.tr("文件已不可用，请重新导入。") }
        if file.isImage { return nil }
        guard let text = file.extractedText, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return L10n.tr("此文件尚不能用于对话。请转为可提取文字的 PDF、文本或图片；仍可预览和分享原件。")
        }
        return text.count > 60_000 ? L10n.tr("文字超过 60,000 字，请拆分后用于对话。原件已完整保存。") : nil
    }
    func libraryChatAttachment(_ item: LibraryItem) throws -> Attachment {
        if let reason = libraryChatLimitation(item.attachment) { throw LocalFailure.message(reason) }
        var file = item.attachment
        if file.isImage { file = try importData(Data(contentsOf: url(for: file)), name: file.name, type: UTType(mimeType: file.type) ?? .image) }
        file.libraryID = item.id
        return file
    }
    func libraryShareURL(_ file: Attachment) throws -> URL {
        // Keep human file names in exported copies, including after a library rename.
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("LibraryShare/" + UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let destination = directory.appendingPathComponent(Self.safeLibraryName(file.name))
        try FileManager.default.copyItem(at: url(for: file), to: destination)
        return destination
    }
}

extension WorkspaceStore {
    var visibleLibrary: [LibraryItem] { library.filter { $0.deletedAt == nil }.sorted { $0.addedAt > $1.addedAt } }

    private func inserting(_ file: Attachment, source: LibrarySource?, date: Date, explicitlySaved: Bool, into items: inout [LibraryItem]) throws -> UUID {
        let digest = try storage.libraryDigest(file)
        if let index = items.firstIndex(where: { $0.id == file.libraryID || $0.attachment.filename == file.filename || $0.digest == digest }) {
            if let source, !items[index].sources.contains(source) { items[index].sources.append(source) }
            if explicitlySaved { items[index].deletedAt = nil }
            return items[index].id
        }
        var original = file; original.libraryID = nil
        let item = LibraryItem(attachment: original, digest: digest, addedAt: date, sources: source.map { [$0] } ?? [])
        items.append(item); return item.id
    }
    @discardableResult
    func saveLibraryFile(_ file: Attachment) throws -> UUID {
        var next = library
        let id = try inserting(file, source: nil, date: Date(), explicitlySaved: true, into: &next)
        try commitLibrary(next); return id
    }
    func collectLibrary(_ files: [Attachment], conversationID: UUID, messageID: UUID) throws {
        guard !files.isEmpty else { return }
        var next = library
        for file in files { _ = try inserting(file, source: LibrarySource(conversationID: conversationID, messageID: messageID), date: Date(), explicitlySaved: false, into: &next) }
        try commitLibrary(next)
    }
    func migrateLibrary() {
        // This runs only when the saved schema has no library. An empty library is
        // intentional, so deleted files never reappear on the next launch.
        for chat in conversations {
            for message in chat.messages {
                let files = message.attachments + (message.versions ?? []).flatMap { $0.attachments ?? [] }
                for file in files {
                    let source = LibrarySource(conversationID: chat.id, messageID: message.id)
                    do { _ = try inserting(file, source: source, date: message.createdAt, explicitlySaved: false, into: &library) }
                    catch {
                        // Preserve a visible missing-file record; never invent file contents.
                        if !library.contains(where: { $0.attachment.filename == file.filename }) {
                            library.append(LibraryItem(attachment: file, addedAt: message.createdAt, sources: [source]))
                        }
                    }
                }
            }
        }
    }
    @discardableResult
    func saveLibraryText(_ text: String, name: String = "", source: LibrarySource? = nil) throws -> UUID {
        let content = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !content.isEmpty else { throw LocalFailure.message(L10n.tr("请先填写要保存的文本。")) }
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let fallback = String(content.components(separatedBy: .newlines).first!.trimmingCharacters(in: CharacterSet(charactersIn: "# ")).prefix(40))
        let filename = (title.isEmpty ? (fallback.isEmpty ? L10n.tr("保存的文本") : fallback) : title)
        let file = try storage.importLibraryData(Data(content.utf8), name: filename.hasSuffix(".md") || filename.hasSuffix(".txt") ? filename : filename + ".md", type: .plainText)
        var next = library
        let id = try inserting(file, source: source, date: Date(), explicitlySaved: true, into: &next)
        try commitLibrary(next); return id
    }
    func renameLibrary(_ id: UUID, name: String) throws {
        var next = library
        guard let i = next.firstIndex(where: { $0.id == id }) else { return }
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { throw LocalFailure.message(L10n.tr("文件名不能为空。")) }
        let suffix = (next[i].attachment.name as NSString).pathExtension
        var renamed = LocalStorage.safeLibraryName(title)
        if !suffix.isEmpty && !(renamed as NSString).pathExtension.lowercased().elementsEqual(suffix.lowercased()) { renamed += "." + suffix }
        next[i].attachment.name = renamed; try commitLibrary(next)
    }
    func trashLibrary(_ ids: Set<UUID>, restore: Bool = false) throws {
        var next = library
        for i in next.indices where ids.contains(next[i].id) { next[i].deletedAt = restore ? nil : Date() }
        try commitLibrary(next)
    }
    func permanentlyDeleteLibrary(_ ids: Set<UUID>) throws {
        try commitLibrary(library.filter { !ids.contains($0.id) || $0.deletedAt == nil })
        // Startup cleanup considers every chat/draft/version reference and library trash.
    }
    func useLibrary(_ ids: Set<UUID>, newConversation: Bool = false) throws {
        let items = visibleLibrary.filter { ids.contains($0.id) }
        guard !items.isEmpty, items.count == ids.count else { throw LocalFailure.message(L10n.tr("所选资料已变更，请重新选择。")) }
        let existing = newConversation ? [] : selected.pendingAttachments
        let fresh = items.filter { item in !existing.contains { $0.libraryID == item.id || $0.id == item.attachment.id } }
        guard fresh.count + existing.count <= Attachment.maximumPerMessage else { throw LocalFailure.message(L10n.tr("每条消息最多添加 \(Attachment.maximumPerMessage) 个附件，请减少选择或使用新对话。")) }
        let files = try fresh.map { try storage.libraryChatAttachment($0) }
        // Prepare all files before changing the selected conversation or its draft.
        if newConversation { newChat() }
        update { $0.pendingAttachments.append(contentsOf: files) }; persist()
    }
}
