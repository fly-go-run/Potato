import Foundation

struct Attachment: Identifiable, Codable, Equatable {
    static let maximumPerMessage = 20
    var id = UUID()
    var name: String
    var filename: String
    var type: String
    var size: Int
    var extractedText: String?
    var libraryID: UUID? = nil
    var isImage: Bool { type.hasPrefix("image/") }
}

enum MessageState: String, Codable { case complete, streaming, stopped, failed }
struct ReplyVersion: Identifiable, Codable, Equatable {
    var activityOrder: [String]? = nil
    var activityAnchors: [String: Int]? = nil
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
    var cloudReply: CloudReply? = nil
    var activityOrder: [String]? = nil
    /// UTF-16 offset of the reply text when each activity began; nil on older replies.
    var activityAnchors: [String: Int]? = nil
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
    /// True while the title was derived by Potato; a rename by the person turns it off.
    var automaticTitle: Bool? = nil
    var recallExcluded: Bool? = nil
    var modelChoice: LocalModelChoice? = nil
    var id = UUID()
    var title = L10n.tr("新对话")
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
enum AppAppearance: String, Codable, CaseIterable {
    case light, dark, automatic
    var title: String {
        switch self { case .light: L10n.tr("亮色"); case .dark: L10n.tr("暗色"); case .automatic: L10n.tr("自动") }
    }
}

struct ConnectionSettings: Codable, Equatable {
    var language: AppLanguage? = nil
    var languageMode: AppLanguage {
        get { language ?? .system }
        set { language = newValue }
    }
    // Optional on disk so workspaces saved before appearance selection still decode.
    var appearance: AppAppearance? = nil
    var appearanceMode: AppAppearance {
        get { appearance ?? .automatic }
        set { appearance = newValue }
    }
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
    /// Developer-only connection controls (manual endpoint, sample replies) are hidden until unlocked.
    var developerMode: Bool? = nil
    /// Written by the person in Settings; appended to the base instructions on every request.
    var customInstructions: String? = nil
    static let defaultSystemPrompt = "你是 Potato，一位清晰、周到的中文助手。使用 Markdown，回答简洁且有帮助。"
    var systemPrompt = ConnectionSettings.defaultSystemPrompt
    var requestInstructions: String {
        let extra = customInstructions?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return extra.isEmpty ? systemPrompt : systemPrompt + "\n\n用户希望你注意：\n" + extra
    }
    var validatedURL: URL? {
        guard let url = URL(string: endpoint.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme == "https", let host = url.host, !host.isEmpty,
              url.user == nil, url.password == nil, url.query == nil, url.fragment == nil else { return nil }
        return url
    }
}
struct SavedWorkspace: Codable {
    var version = 2
    var conversations: [Conversation]
    var selectedID: UUID
    var settings = ConnectionSettings()
    var library: [LibraryItem]? = nil
}
