import Foundation

struct Attachment: Identifiable, Codable, Equatable {
    var id = UUID()
    var name: String
    var filename: String
    var type: String
    var size: Int
    var extractedText: String?
    var isImage: Bool { type.hasPrefix("image/") }
}

enum MessageState: String, Codable { case complete, streaming, stopped, failed }
struct ReplyVersion: Identifiable, Codable, Equatable {
    var modelChoice: LocalModelChoice? = nil
    var reasoning: ReasoningTrace? = nil
    var recalls: [RecallRun]? = nil
    var codeRuns: [CodeExecutionRun]? = nil
    var searches: [WebSearchRun]? = nil
    var execution: SandboxExecution? = nil
    var attachments: [Attachment]? = nil
    var id = UUID()
    var text: String
    var state: MessageState
    var failure: String?
    var date = Date()
}
struct ChatMessage: Identifiable, Codable, Equatable {
    var modelChoice: LocalModelChoice? = nil
    var displayModelChoice: LocalModelChoice? { if let selectedVersion { return selectedVersion.modelChoice }; return modelChoice }
    var reasoning: ReasoningTrace? = nil
    var displayReasoning: ReasoningTrace? { if let selectedVersion { return selectedVersion.reasoning }; return reasoning }
    var recalls: [RecallRun]? = nil
    var codeRuns: [CodeExecutionRun]? = nil
    var searches: [WebSearchRun]? = nil
    var displayCodeRuns: [CodeExecutionRun] { if let selectedVersion { return selectedVersion.codeRuns ?? [] }; return codeRuns ?? [] }
    var displayRecalls: [RecallRun] { if let selectedVersion { return selectedVersion.recalls ?? [] }; return recalls ?? [] }
    var displaySearches: [WebSearchRun] { if let selectedVersion { return selectedVersion.searches ?? [] }; return searches ?? [] }
    var execution: SandboxExecution? = nil
    var versions: [ReplyVersion]? = nil
    var selectedVersionID: UUID? = nil
    var selectedVersion: ReplyVersion? { versions?.first { $0.id == selectedVersionID } }
    var displayText: String { selectedVersion?.text ?? text }
    var displayAttachments: [Attachment] { if let selectedVersion { return selectedVersion.attachments ?? [] }; return attachments }
    var displayExecution: SandboxExecution? { if let selectedVersion { return selectedVersion.execution }; return execution }
    var displayState: MessageState { selectedVersion?.state ?? state }
    var displayFailure: String? { if let selectedVersion { return selectedVersion.failure }; return failure }
    var versionCount: Int { (versions?.count ?? 0) + 1 }
    var versionIndex: Int { versions?.firstIndex { $0.id == selectedVersionID } ?? (versions?.count ?? 0) }
    var id = UUID()
    var role: String
    var text: String
    var attachments: [Attachment] = []
    var state: MessageState = .complete
    var failure: String?
    var createdAt = Date()
}
struct Conversation: Identifiable, Codable {
    var recallExcluded: Bool? = nil
    var modelChoice: LocalModelChoice? = nil
    var id = UUID()
    var title = "新对话"
    var messages: [ChatMessage] = []
    var draft: WorkingDraft?
    var input = ""
    var pendingAttachments: [Attachment] = []
    var updatedAt = Date()
    var pinned = false
    var deletedAt: Date?
    var isExample = false
    static func example() -> Conversation {
        var value = Conversation()
        value.title = "周末计划"
        value.isExample = true
        value.messages = [ChatMessage(role: "user", text: "把这些想法整理成一个周末计划"),
                          ChatMessage(role: "assistant", text: "整理成了一份可以继续修改的计划。")]
        value.draft = WorkingDraft()
        return value
    }
}
struct ConnectionSettings: Codable, Equatable {
    var recallEnabled: Bool? = nil
    var automaticMemory: Bool? = nil
    var cloudAccount: RemoteAccountProfile? = nil
    var modelCatalog: LocalModelCatalog? = nil
    var demo = true
    var endpoint = ""
    var model = ""
    var displayModelName: String {
        modelEntry(model).name
    }
    var haptics = true
    var systemPrompt = "你是 Potato，一位清晰、周到的中文助手。使用 Markdown，回答简洁且有帮助。"
    var validatedURL: URL? {
        guard let url = URL(string: endpoint.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme == "https", let host = url.host, !host.isEmpty,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil else { return nil }
        return url
    }
}
struct SavedWorkspace: Codable {
    var version = 1
    var conversations: [Conversation]
    var selectedID: UUID
    var settings = ConnectionSettings()
}
