import SwiftUI

struct CodeExecutionRun: Codable, Equatable, Identifiable {
    var id: String
    var title: String? = nil
    var attachmentIDs: [UUID]? = nil
    var state: String
    var code: String
    var result: SandboxExecution?
    var message: String?

    func validate() throws {
        guard !id.isEmpty, id.utf8.count <= 200, (title?.count ?? 0) <= 120, code.utf8.count <= 128_000,
              ["running", "complete", "failed"].contains(state), (message?.count ?? 0) <= 500 else {
            throw LocalFailure.message(L10n.tr("代码执行事件格式无效。"))
        }
        if let result {
            guard state != "running", result.status == state,
                  result.stdout.count <= 32_000, result.stderr.count <= 16_000,
                  result.text.count <= 32_000, (result.error?.count ?? 0) <= 4_000,
                  result.artifacts.count <= 8,
                  result.artifacts.reduce(0, { $0 + $1.base64.utf8.count }) <= 4_000_000 else {
                throw LocalFailure.message(L10n.tr("代码执行结果过大或格式无效。"))
            }
        } else if state == "complete" { throw LocalFailure.message(L10n.tr("代码执行缺少结果。")) }
    }
}

extension SandboxService {
    /// Files already attached to this conversation are task inputs. Keep large-file
    /// chat usable, but explicitly tell the model which files cannot fit the tool.
    static func automaticInput(messages: [ChatMessage], storage: LocalStorage, bodyBytes: Int) throws -> [String: Any] {
        let candidates = messages.last(where: { !$0.displayAttachments.isEmpty })?.displayAttachments ?? []
        var files: [[String: String]] = [], notes: [String] = []
        var remaining = min(2_800_000, max(0, 4 * 1_024 * 1_024 - bodyBytes - 16_000))
        for (index, file) in candidates.prefix(4).enumerated() {
            let label = String(file.name.prefix(150)), name = filename(file, index: index)
            guard file.size <= 2_000_000 else { notes.append(L10n.tr("\(label)：文件过大，未提供给代码执行工具。")); continue }
            let data = try Data(contentsOf: storage.url(for: file))
            let encoded = data.base64EncodedString()
            guard data.count <= 2_000_000, encoded.utf8.count + 512 <= remaining else { notes.append(L10n.tr("\(label)：超出本轮附件预算，未提供给代码执行工具。")); continue }
            remaining -= encoded.utf8.count + 512
            files.append(["name": name, "base64": encoded]); notes.append("\(label) → /home/user/\(name)")
        }
        if candidates.count > 4 { notes.append(L10n.tr("其余 \(candidates.count - 4) 个文件未提供，本轮最多使用 4 个输入文件。")) }
        return ["enabled": true, "files": files, "file_notes": notes]
    }
}

extension WorkspaceStore {
    func recordCodeExecution(_ event: CodeExecutionRun, messageID: UUID, conversationID: UUID) throws {
        guard let chat = conversations.first(where: { $0.id == conversationID }),
              let message = chat.messages.first(where: { $0.id == messageID }), message.state == .streaming else { return }
        var runs = message.codeRuns ?? []
        if let old = runs.first(where: { $0.id == event.id }), old.state != "running" { return }
        guard runs.contains(where: { $0.id == event.id }) || runs.count < 3 else { throw LocalFailure.message(L10n.tr("本轮代码执行次数超过限制。")) }
        var saved = event
        let files = try importExecutionArtifacts(event.result?.artifacts ?? [])
        try collectLibrary(files, conversationID: conversationID, messageID: messageID)
        saved.attachmentIDs = files.map(\.id)
        saved.result?.artifacts = event.result?.artifacts.map { SandboxArtifact(name: $0.name, mime: $0.mime, base64: "") } ?? []
        if let i = runs.firstIndex(where: { $0.id == event.id }) { runs[i] = saved } else { runs.append(saved) }
        update(conversationID) { chat in
            guard let i = chat.messages.firstIndex(where: { $0.id == messageID }) else { return }
            chat.messages[i].reasoning?.finish(.complete, at: Date())
            chat.messages[i].recordActivity("code:\(event.id)")
            chat.messages[i].codeRuns = runs
            chat.messages[i].attachments.append(contentsOf: files)
        }
        persist()
    }
}
