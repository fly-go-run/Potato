import Foundation
import CryptoKit
import Combine

/// Credential identity is account-wide; a command destination is not.
struct RemoteTargetIdentity: Codable, Equatable {
    let scheme: String
    let host: String
    let port: Int
    let deviceID: String
    let owner: String?
    var key: String {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        return SHA256.hash(data: try! encoder.encode(self)).map { String(format: "%02x", $0) }.joined()
    }

    init(_ device: RemoteDevice) {
        scheme = device.relay.scheme?.lowercased() ?? "https"
        host = device.relay.host?.lowercased() ?? ""
        port = device.relay.port ?? (scheme == "https" ? 443 : 80)
        deviceID = device.id
        owner = device.owner
    }
}

struct RemoteDraftAddress: Codable, Equatable {
    let target: RemoteTargetIdentity
    let kind: String
    let value: String

    init(device: RemoteDevice, chatID: String? = nil, projectPath: String? = nil) {
        target = RemoteTargetIdentity(device)
        kind = chatID != nil ? "chat" : projectPath != nil ? "project" : "new"
        value = chatID ?? projectPath ?? ""
    }
    func conversation(_ id: String) -> Self { Self(target: target, kind: "chat", value: id) }
    private init(target: RemoteTargetIdentity, kind: String, value: String) {
        self.target = target; self.kind = kind; self.value = value
    }
    var key: String {
        // Length-delimited JSON fields avoid collisions between paths, scopes,
        // default ports and account/target identifiers.
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        return SHA256.hash(data: try! encoder.encode(self)).map { String(format: "%02x", $0) }.joined()
    }
}

struct RemoteDraftRecord: Codable {
    var modelChoice: RemoteModelChoice? = nil
    var text = ""
    var pending: RemotePendingSend?
    var otherDrafts: [String] = []
}

struct RemoteLegacyDraft {
    let key: String
    let text: String
    let pending: RemotePendingSend?
}

/// One atomic document lets acknowledgment move a draft from a temporary slot
/// to its server conversation without a crash window between two key writes.
struct RemoteDraftRepository {
    let file: URL
    let legacyDefaults: UserDefaults
    private struct Database: Codable {
        var version = 1
        var drafts: [String: RemoteDraftRecord] = [:]
        var claimedLegacy: [String: RemoteTargetIdentity] = [:]
        var unconfirmed: [String: [RemotePendingSend]]? = nil
    }
    init(file: URL? = nil, legacyDefaults: UserDefaults? = nil) {
        let testing = ProcessInfo.processInfo.arguments.contains("--ui-testing")
        let root = testing
            ? FileManager.default.temporaryDirectory.appendingPathComponent("PotatoRemoteUITests")
            : FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("PotatoRemote")
        self.file = file ?? root.appendingPathComponent("drafts.json")
        self.legacyDefaults = legacyDefaults ?? (testing ? UserDefaults(suiteName: "PotatoRemoteUITests")! : .standard)
    }
    private func read() throws -> Database {
        guard FileManager.default.fileExists(atPath: file.path) else { return Database() }
        do {
            let value = try JSONDecoder().decode(Database.self, from: Data(contentsOf: file))
            guard value.version == 1 else { throw LocalFailure.message("版本不支持") }
            return value
        } catch { throw LocalFailure.message("远程草稿暂时无法读取，原记录已保留。请勿卸载应用。") }
    }
    private func write(_ database: Database) throws {
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(database).write(to: file, options: [.atomic, .completeFileProtectionUnlessOpen])
    }
    func load(_ address: RemoteDraftAddress) throws -> RemoteDraftRecord {
        let record = try read().drafts[address.key] ?? RemoteDraftRecord()
        if let request = record.pending, request.target != address.target || request.chatID != (address.kind == "chat" ? address.value : nil) {
            throw LocalFailure.message("待确认指令的目标与这台电脑不一致，原记录已保留。")
        }
        return record
    }
    func saveText(_ text: String, at address: RemoteDraftAddress) throws {
        var database = try read()
        var record = try load(address); record.text = text
        database.drafts[address.key] = record
        try write(database)
    }
    func saveModelChoice(_ choice: RemoteModelChoice?, at address: RemoteDraftAddress) throws {
        var database = try read(); var record = try load(address)
        record.modelChoice = choice; database.drafts[address.key] = record
        try write(database)
    }
    func reserve(_ request: RemotePendingSend, at address: RemoteDraftAddress, text: String) throws {
        guard request.target == address.target,
              request.chatID == (address.kind == "chat" ? address.value : nil) else {
            throw LocalFailure.message("指令目标已改变，请重新打开对应会话。")
        }
        var database = try read()
        var record = try load(address)
        guard record.pending == nil || record.pending == request else {
            throw LocalFailure.message("此会话已有待确认的指令，请先确认发送结果。")
        }
        record.text = text; record.pending = request
        database.drafts[address.key] = record
        try write(database)
    }
    func acknowledge(_ request: RemotePendingSend, at address: RemoteDraftAddress, chatID: String) throws -> RemoteDraftAddress {
        guard request.target == address.target else { throw LocalFailure.message("指令不属于这台电脑。") }
        guard !chatID.isEmpty, request.chatID == nil || request.chatID == chatID else { throw LocalFailure.message("发送回执返回了其他会话，原指令已保留。") }
        var database = try read()
        let destination = address.conversation(chatID)
        guard var source = database.drafts[address.key], source.pending?.id == request.id else {
            // Another view may already have consumed the same immutable receipt.
            if database.drafts[destination.key] != nil { return destination }
            throw LocalFailure.message("发送回执与本地记录不一致，草稿已保留。")
        }
        source.pending = nil
        if source.text == request.text { source.text = "" }
        if destination != address, var existing = database.drafts[destination.key] {
            // Never replace an independently edited draft with a late receipt.
            if !source.text.isEmpty && source.text != existing.text && !existing.otherDrafts.contains(source.text) {
                existing.otherDrafts.append(source.text)
            }
            for text in source.otherDrafts where text != existing.text && !existing.otherDrafts.contains(text) { existing.otherDrafts.append(text) }
            database.drafts[destination.key] = existing
        } else { database.drafts[destination.key] = source }
        if destination != address { database.drafts.removeValue(forKey: address.key) }
        try write(database)
        return destination
    }
    func reject(_ request: RemotePendingSend, at address: RemoteDraftAddress) throws {
        var database = try read()
        guard var record = database.drafts[address.key], record.pending?.id == request.id else { return }
        record.pending = nil; database.drafts[address.key] = record
        try write(database)
    }
    func archived(for target: RemoteTargetIdentity) throws -> [RemotePendingSend] {
        let requests = try read().unconfirmed?[target.key] ?? []
        guard requests.allSatisfy({ $0.target == target }) else { throw LocalFailure.message("待确认记录的电脑信息不一致，原文件已保留。") }
        return requests
    }
    /// Explicit user review only: this is neither an acknowledgment nor a retry.
    /// The original immutable request stays available at every draft for this device.
    func archiveUnconfirmed(_ request: RemotePendingSend, at address: RemoteDraftAddress) throws {
        var database = try read()
        guard request.target == address.target, var record = database.drafts[address.key], record.pending == request else {
            throw LocalFailure.message("待确认指令已改变，请重新核对。")
        }
        var archive = try archived(for: address.target)
        if let old = archive.first(where: { $0.id == request.id }), old != request { throw LocalFailure.message("原编号对应的记录不一致，未结束等待。") }
        if !archive.contains(request) { archive.append(request) }
        if database.unconfirmed == nil { database.unconfirmed = [:] }
        database.unconfirmed?[address.target.key] = archive
        record.pending = nil
        if record.text == request.text { record.text = "" }
        database.drafts[address.key] = record
        try write(database)
    }
    func chooseOtherDraft(_ text: String, at address: RemoteDraftAddress) throws {
        var database = try read(); var record = try load(address)
        guard record.otherDrafts.contains(text) else { return }
        record.otherDrafts.removeAll { $0 == text }
        if !record.text.isEmpty && !record.otherDrafts.contains(record.text) { record.otherDrafts.append(record.text) }
        record.text = text; database.drafts[address.key] = record
        try write(database)
    }
    func legacy(device: RemoteDevice, chatID: String?, projectPath: String?) throws -> RemoteLegacyDraft? {
        let key = "remote-draft-\(device.account)-\(chatID ?? projectPath ?? "new")"
        guard try read().claimedLegacy[key] == nil else { return nil }
        let text = legacyDefaults.string(forKey: key) ?? ""
        let data = legacyDefaults.data(forKey: key + "-pending")
        let pending: RemotePendingSend?
        do { pending = try data.map { try JSONDecoder().decode(RemotePendingSend.self, from: $0) } }
        catch { throw LocalFailure.message("旧版待确认指令无法读取，原记录已保留，未重新发送。") }
        guard !text.isEmpty || pending != nil else { return nil }
        return RemoteLegacyDraft(key: key, text: text, pending: pending)
    }
    func claimLegacy(_ legacy: RemoteLegacyDraft, device: RemoteDevice, at address: RemoteDraftAddress) throws -> RemoteDraftAddress {
        var database = try read()
        guard database.claimedLegacy[legacy.key] == nil else { throw LocalFailure.message("这份旧草稿已在另一处恢复，请重新打开会话。") }
        guard address.target == RemoteTargetIdentity(device), legacy.pending?.target == nil || legacy.pending?.target == address.target else { throw LocalFailure.message("旧指令已属于其他电脑，不能重新分配。") }
        // The user confirms the missing target; retain the original operation ID
        // and payload. A first conversation's old slot may contain a follow-up.
        let pending = legacy.pending.map { $0.bound(to: device) }
        let destination = pending?.chatID.map { address.conversation($0) } ?? address
        var record = try load(destination)
        guard record.pending == nil else { throw LocalFailure.message("此会话已有待确认指令，请先处理后再恢复旧稿。") }
        if !record.text.isEmpty && record.text != legacy.text && !record.otherDrafts.contains(record.text) { record.otherDrafts.append(record.text) }
        record.text = legacy.text; record.pending = pending
        database.drafts[destination.key] = record
        database.claimedLegacy[legacy.key] = address.target
        try write(database)
        // Keep the old defaults as an archive; claimedLegacy prevents reuse.
        return destination
    }
    static func resetTestStorage() {
        #if DEBUG
        guard ProcessInfo.processInfo.arguments.contains("--ui-testing"), ProcessInfo.processInfo.arguments.contains("--reset") else { return }
        let file = RemoteDraftRepository().file
        try? FileManager.default.removeItem(at: file)
        #endif
    }
}

@MainActor final class RemoteDraftSession: ObservableObject {
    @Published private(set) var modelChoice: RemoteModelChoice?
    @Published var text = "" { didSet { if !applying { dirty = true; scheduleSave() } } }
    @Published private(set) var pending: RemotePendingSend?
    @Published private(set) var otherDrafts: [String] = []
    @Published private(set) var archived: [RemotePendingSend] = []
    @Published private(set) var legacy: RemoteLegacyDraft?
    @Published private(set) var legacyError: String?
    @Published private(set) var storageError: String?
    private(set) var address: RemoteDraftAddress
    private let repository: RemoteDraftRepository
    private let device: RemoteDevice
    private let initialChatID: String?
    private let projectPath: String?
    private var applying = false
    private var dirty = false
    private var loaded = false
    private var saveTask: Task<Void, Never>?

    init(device: RemoteDevice, chatID: String? = nil, projectPath: String? = nil, repository: RemoteDraftRepository = RemoteDraftRepository()) {
        self.device = device; self.initialChatID = chatID; self.projectPath = projectPath; self.repository = repository
        address = RemoteDraftAddress(device: device, chatID: chatID, projectPath: projectPath)
    }
    func load() {
        guard !loaded else { return }; loaded = true
        do {
            apply(try repository.load(address))
            archived = try repository.archived(for: address.target)
        } catch { storageError = error.localizedDescription }
        do { legacy = try repository.legacy(device: device, chatID: initialChatID, projectPath: projectPath) }
        catch { legacyError = error.localizedDescription }
    }
    private func apply(_ record: RemoteDraftRecord) {
        applying = true; text = record.text; pending = record.pending; otherDrafts = record.otherDrafts; modelChoice = record.modelChoice
        applying = false; dirty = false; storageError = nil
    }
    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }; self?.flush()
        }
    }
    func flush() {
        saveTask?.cancel(); saveTask = nil
        guard dirty else { return }
        do { try repository.saveText(text, at: address); dirty = false; storageError = nil }
        catch { storageError = error.localizedDescription }
    }
    func chooseModel(_ choice: RemoteModelChoice?) throws {
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        try repository.saveModelChoice(choice, at: address); modelChoice = choice
    }
    func prepareSend(modelChoice: RemoteModelChoice? = nil, expectedRunID: String? = nil) throws -> RemotePendingSend {
        guard storageError == nil else { throw LocalFailure.message(storageError!) }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw LocalFailure.message("请先输入要发送的指令。") }
        let request = RemotePendingSend(modelChoice: modelChoice, expectedRunID: expectedRunID, id: UUID().uuidString, text: text, chatID: address.kind == "chat" ? address.value : nil, projectPath: projectPath, target: address.target)
        try repository.reserve(request, at: address, text: text)
        saveTask?.cancel(); dirty = false; pending = request
        return request
    }
    func acknowledge(_ request: RemotePendingSend, chatID: String) throws {
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        address = try repository.acknowledge(request, at: address, chatID: chatID)
        apply(try repository.load(address))
    }
    func reject(_ request: RemotePendingSend) throws {
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        try repository.reject(request, at: address); apply(try repository.load(address))
    }
    func archiveUnconfirmed(_ request: RemotePendingSend) throws {
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        try repository.archiveUnconfirmed(request, at: address)
        apply(try repository.load(address)); archived = try repository.archived(for: address.target)
    }
    func restoreLegacy() throws {
        guard let legacy else { return }
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        address = try repository.claimLegacy(legacy, device: device, at: address)
        apply(try repository.load(address)); self.legacy = nil
    }
    func chooseOtherDraft(_ value: String) throws {
        flush()
        if let storageError { throw LocalFailure.message(storageError) }
        try repository.chooseOtherDraft(value, at: address); apply(try repository.load(address))
    }
}
