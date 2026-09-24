import SwiftUI
import PhotosUI
import UniformTypeIdentifiers

/// Owns imports outside the sheet so dismissal does not cancel a selected photo.
/// Reservations happen synchronously; queued imports retain tap order and chat ownership.
@MainActor
final class ComposerAttachments: ObservableObject {
    static let limit = Attachment.maximumPerMessage
    @Published private(set) var pending: [UUID: Set<String>] = [:]
    @Published private(set) var added: [UUID: [String: UUID]] = [:]
    @Published private(set) var failures: [UUID: [String: String]] = [:]
    private var tail: Task<Void, Never>?

    func count(in chat: UUID) -> Int { pending[chat]?.count ?? 0 }
    func remaining(in chat: Conversation) -> Int { max(0, Self.limit - chat.pendingAttachments.count - count(in: chat.id)) }
    func attachment(for key: String, in chat: Conversation) -> UUID? {
        guard let id = added[chat.id]?[key], chat.pendingAttachments.contains(where: { $0.id == id }) else { return nil }
        return id
    }
    func failure(in chat: UUID) -> String? { failures[chat]?.sorted(by: { $0.key < $1.key }).first?.value }
    func clearFailures(in chat: UUID) { failures[chat] = nil }

    @discardableResult
    func add(key: String = UUID().uuidString, to chatID: UUID, store: WorkspaceStore,
             onFinish: @escaping () -> Void = {},
             load: @escaping () async throws -> Attachment) -> Bool {
        guard let chat = store.conversations.first(where: { $0.id == chatID && $0.deletedAt == nil }),
              !((pending[chatID] ?? []).contains(key)), attachment(for: key, in: chat) == nil,
              remaining(in: chat) > 0 else { return false }
        pending[chatID, default: []].insert(key)
        failures[chatID]?[key] = nil
        let previous = tail
        tail = Task {
            await previous?.value
            defer { pending[chatID]?.remove(key); onFinish() }
            guard store.conversations.contains(where: { $0.id == chatID && $0.deletedAt == nil }) else { return }
            do {
                let attachment = try await load()
                guard let target = store.conversations.first(where: { $0.id == chatID && $0.deletedAt == nil }),
                      target.pendingAttachments.count < Self.limit else {
                    // A library action may have filled the draft while the sheet was closed.
                    try? FileManager.default.removeItem(at: store.storage.url(for: attachment))
                    failures[chatID, default: [:]][key] = L10n.tr("每条消息最多添加 \(Self.limit) 个附件。")
                    return
                }
                store.update(chatID) { $0.pendingAttachments.append(attachment) }
                added[chatID, default: [:]][key] = attachment.id
                store.persist()
            } catch {
                failures[chatID, default: [:]][key] = error.localizedDescription
            }
        }
        return true
    }

    func importPickerResults(_ items: [PHPickerResult], to chatID: UUID, store: WorkspaceStore) {
        for item in items {
            let storage = store.storage
            add(key: item.assetIdentifier ?? UUID().uuidString, to: chatID, store: store) {
                let provider = item.itemProvider
                guard let type = provider.registeredTypeIdentifiers.first(where: { UTType($0)?.conforms(to: .image) == true }) else {
                    throw LocalFailure.message(L10n.tr("照片无法读取，请重新选择。"))
                }
                let data: Data = try await withCheckedThrowingContinuation { continuation in
                    provider.loadDataRepresentation(forTypeIdentifier: type) { data, error in
                        if let data { continuation.resume(returning: data) }
                        else { continuation.resume(throwing: error ?? LocalFailure.message(L10n.tr("照片无法读取，请重新选择。"))) }
                    }
                }
                return try await Task.detached { try storage.importData(data, name: L10n.tr("照片.jpg"), type: .image) }.value
            }
        }
    }

    func importFiles(_ urls: [URL], to chatID: UUID, store: WorkspaceStore) {
        guard let chat = store.conversations.first(where: { $0.id == chatID }), urls.count <= remaining(in: chat) else {
            failures[chatID, default: [:]]["files"] = L10n.tr("每条消息最多 \(Self.limit) 个附件，请重新选择。")
            return
        }
        failures[chatID]?["files"] = nil
        for url in urls {
            let storage = store.storage
            // Hold the provider grant while this file waits behind other imports.
            let scoped = url.startAccessingSecurityScopedResource()
            let accepted = add(to: chatID, store: store, onFinish: { if scoped { url.stopAccessingSecurityScopedResource() } }) {
                return try await Task.detached { try storage.importFile(url) }.value
            }
            if !accepted && scoped { url.stopAccessingSecurityScopedResource() }
        }
    }

    func waitUntilFinished() async { await tail?.value }
}
