import SwiftUI

enum AppEnvironment {
    static var isUITesting: Bool { ProcessInfo.processInfo.arguments.contains("--ui-testing") }
    /// Isolated UI-test storage that behaves like a fresh install (no sample, sign-in required).
    static var isFirstRunPreview: Bool { isUITesting && ProcessInfo.processInfo.arguments.contains("--first-run-preview") }
    /// UI and unit tests keep the offline sample replies; people must sign in instead.
    static var isAutomatedTest: Bool { (isUITesting && !isFirstRunPreview) || ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil }
    static var versionLabel: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "0.2"
        guard let build = info?["CFBundleVersion"] as? String, !build.isEmpty, build != version else { return version }
        return "\(version) (\(build))"
    }
}

/// The service rejected the saved credential. The workspace offers sign-in instead of a generic error.
struct AuthorizationFailure: LocalizedError {
    var errorDescription: String? { Self.message }
    static var message: String { L10n.tr("登录已过期，请重新登录后继续。") }
}

enum ModelNaming {
    private static let brands = ["deepseek": "DeepSeek", "gpt": "GPT", "claude": "Claude", "gemini": "Gemini", "qwen": "Qwen",
                                 "glm": "GLM", "kimi": "Kimi", "doubao": "Doubao", "grok": "Grok", "llama": "Llama",
                                 "mistral": "Mistral", "minimax": "MiniMax", "openai": "OpenAI", "sonnet": "Sonnet", "opus": "Opus", "haiku": "Haiku"]
    /// A readable name for a model ID. Service-provided names are kept; only an expiry suffix is removed.
    static func displayName(id: String, name: String) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        // A name that only repeats the bare model ID is not a service-provided name.
        if !trimmed.isEmpty && trimmed != id && trimmed != id.split(separator: "/").last.map(String.init) { return withoutExpiry(trimmed) }
        let base = withoutExpiry(String(id.split(separator: "/").last ?? Substring(id)))
        let tokens = base.split(whereSeparator: { $0 == "-" || $0 == "_" || $0 == " " }).map(String.init)
        guard !tokens.isEmpty else { return id }
        var words: [String] = []
        for token in tokens {
            let lower = token.lowercased()
            if let brand = brands[lower] { words.append(brand); continue }
            if lower.range(of: "^v?[0-9]+(\\.[0-9]+)*[a-z]?$", options: .regularExpression) != nil {
                let version = lower.hasPrefix("v") ? "V" + lower.dropFirst() : lower
                // OpenAI writes its versions attached to the family name: GPT-5.6.
                if words.last == "GPT" { words[words.count - 1] = "GPT-" + version } else { words.append(version) }
                continue
            }
            words.append(lower.prefix(1).uppercased() + lower.dropFirst())
        }
        return words.joined(separator: " ")
    }
    static func withoutExpiry(_ value: String) -> String {
        value.replacingOccurrences(of: "[-_ ]?expires[-_ ]on[-_ ]?[0-9]{3,8}$", with: "", options: [.regularExpression, .caseInsensitive])
    }
    /// IDs such as `…-expires-on-0910` announce their own retirement date (month and day).
    static func isExpired(_ id: String, now: Date = Date(), calendar: Calendar = .current) -> Bool {
        guard let range = id.range(of: "expires[-_]on[-_]?[0-9]{4}$", options: [.regularExpression, .caseInsensitive]) else { return false }
        let digits = id[range].filter(\.isNumber)
        guard let month = Int(digits.prefix(2)), let day = Int(digits.suffix(2)), (1...12).contains(month), (1...31).contains(day) else { return false }
        var parts = calendar.dateComponents([.year], from: now)
        parts.month = month; parts.day = day
        guard var date = calendar.date(from: parts) else { return false }
        // A date far in the future belongs to last year (for example, 1231 read in January).
        if date.timeIntervalSince(now) > 183 * 86400, let previous = calendar.date(byAdding: .year, value: -1, to: date) { date = previous }
        guard let end = calendar.date(byAdding: .day, value: 1, to: calendar.startOfDay(for: date)) else { return false }
        return now >= end
    }
}

enum ConversationTitle {
    static var defaults: Set<String> { ["新对话", "New conversation", L10n.tr("新对话")] }
    /// A short title from the first line of a message. `limit` is in CJK characters; Latin letters
    /// count as half width, and Latin text ends on a word boundary.
    static func make(from text: String, limit: Int = 22) -> String {
        let line = text.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? ""
        var value = line.replacingOccurrences(of: "^([#>*+-]+|[0-9]+[.)])\\s*", with: "", options: .regularExpression)
        value = value.replacingOccurrences(of: "[*_`]", with: "", options: .regularExpression)
        value = value.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        value = value.trimmingCharacters(in: CharacterSet.whitespaces.union(CharacterSet(charactersIn: "。．.，,！!？?：:；;、")))
        let budget = limit * 2
        var width = 0, cut = value.endIndex
        for index in value.indices {
            width += value[index].unicodeScalars.first.map { $0.value < 0x1100 } == true ? 1 : 2
            if width > budget { cut = index; break }
        }
        guard cut < value.endIndex else { return value }
        let prefix = value[..<cut]
        if value[cut].isLetter, value[cut].isASCII, let space = prefix.lastIndex(of: " "), prefix.distance(from: prefix.startIndex, to: space) >= prefix.count / 2 {
            return String(prefix[..<space]) + "…"
        }
        return String(prefix).trimmingCharacters(in: .whitespaces) + "…"
    }
    /// Clean a model-written title; nil when nothing usable remains.
    static func sanitizeGenerated(_ text: String) -> String? {
        var value = text.split(whereSeparator: \.isNewline).first.map(String.init) ?? ""
        value = value.replacingOccurrences(of: "^(标题|title)\\s*[:：]\\s*", with: "", options: [.regularExpression, .caseInsensitive])
        value = value.trimmingCharacters(in: CharacterSet.whitespaces.union(CharacterSet(charactersIn: "\"'“”‘’「」『』《》#*`。．.，,！!？?：:；;")))
        guard !value.isEmpty else { return nil }
        return value.count > 30 ? String(value.prefix(30)) : value
    }
}

extension Conversation {
    var isEmptyShell: Bool { messages.isEmpty && input.isEmpty && pendingAttachments.isEmpty && draft == nil }
    /// Older conversations may still carry the placeholder title; derive one from the first message.
    var displayTitle: String {
        guard ConversationTitle.defaults.contains(title) else { return title }
        guard let first = messages.first(where: { $0.role == "user" }) else {
            // An unsent draft is still worth finding again.
            let draft = input.trimmingCharacters(in: .whitespacesAndNewlines)
            return draft.isEmpty ? L10n.tr("新对话") : L10n.tr("草稿 · \(ConversationTitle.make(from: draft, limit: 16))")
        }
        let text = first.text.trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? L10n.tr("附件对话") : ConversationTitle.make(from: text)
    }
}

/// Date sections for conversation lists, newest first.
enum ConversationPeriod: Int, CaseIterable {
    case today, yesterday, week, month, older
    var title: String {
        switch self {
        case .today: L10n.tr("今天")
        case .yesterday: L10n.tr("昨天")
        case .week: L10n.tr("过去 7 天")
        case .month: L10n.tr("过去 30 天")
        case .older: L10n.tr("更早")
        }
    }
    static func of(_ date: Date, now: Date = Date(), calendar: Calendar = .current) -> ConversationPeriod {
        if calendar.isDate(date, inSameDayAs: now) { return .today }
        if let yesterday = calendar.date(byAdding: .day, value: -1, to: now), calendar.isDate(date, inSameDayAs: yesterday) { return .yesterday }
        let days = calendar.dateComponents([.day], from: calendar.startOfDay(for: date), to: calendar.startOfDay(for: now)).day ?? 0
        if days < 7 { return .week }
        if days < 30 { return .month }
        return .older
    }
}

/// One icon and color per file family, shared by chat cards and the library.
struct FileKind {
    let symbol: String
    let label: String
    let tint: Color
    init(filename: String, type: String = "") {
        switch (filename as NSString).pathExtension.lowercased() {
        case "ppt", "pptx", "key": symbol = "rectangle.on.rectangle.angled"; label = L10n.tr("演示文稿"); tint = Color(red: 0.84, green: 0.40, blue: 0.20)
        case "pdf": symbol = "doc.richtext"; label = L10n.tr("PDF 文档"); tint = Color(red: 0.80, green: 0.22, blue: 0.20)
        case "xls", "xlsx", "csv", "numbers": symbol = "tablecells"; label = L10n.tr("电子表格"); tint = Color(red: 0.16, green: 0.55, blue: 0.32)
        case "doc", "docx", "pages", "rtf": symbol = "doc.text"; label = L10n.tr("Word 文档"); tint = Color(red: 0.19, green: 0.40, blue: 0.80)
        case "md", "markdown", "txt": symbol = "text.alignleft"; label = L10n.tr("文本"); tint = Color(red: 0.42, green: 0.42, blue: 0.40)
        case "py", "js", "ts", "swift", "json", "html", "css", "sh": symbol = "chevron.left.forwardslash.chevron.right"; label = L10n.tr("代码"); tint = Color(red: 0.45, green: 0.33, blue: 0.70)
        case "zip", "gz", "tar": symbol = "doc.zipper"; label = L10n.tr("压缩包"); tint = Color(red: 0.55, green: 0.45, blue: 0.30)
        default:
            if type.hasPrefix("image/") { symbol = "photo"; label = L10n.tr("图片"); tint = Color(red: 0.19, green: 0.40, blue: 0.80) }
            else { symbol = "doc"; label = L10n.tr("文件"); tint = Color(red: 0.42, green: 0.42, blue: 0.40) }
        }
    }
    /// "llm_inference-overview.pptx" → "Llm inference overview".
    static func readableTitle(_ filename: String) -> String {
        let base = (filename as NSString).deletingPathExtension
            .replacingOccurrences(of: "[-_]+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespaces)
        guard !base.isEmpty else { return filename }
        // Short vowel-less words are almost always acronyms: llm → LLM, pdf → PDF.
        let words = base.split(separator: " ").map { word -> String in
            let lower = word.lowercased()
            if (2...5).contains(lower.count), lower.allSatisfy({ $0.isASCII && $0.isLetter }), !lower.contains(where: { "aeiou".contains($0) }) { return lower.uppercased() }
            return String(word)
        }
        let joined = words.joined(separator: " ")
        return joined.prefix(1).uppercased() + joined.dropFirst()
    }
}

/// Shown above the composer when the saved sign-in no longer works.
struct AuthorizationBanner: View {
    var manualConnection = false
    let signIn: () -> Void
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "person.crop.circle.badge.exclamationmark").font(.system(size: 18)).foregroundStyle(.orange).accessibilityHidden(true)
            Text(manualConnection ? L10n.tr("连接令牌无效") : L10n.tr("登录已过期")).font(.subheadline.weight(.medium)).lineLimit(2)
            Spacer(minLength: 8)
            Button(manualConnection ? L10n.tr("检查设置") : L10n.tr("重新登录"), action: signIn).font(.subheadline.weight(.semibold))
                .padding(.horizontal, 14).frame(minHeight: 36).foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule())
                .frame(minHeight: 44).accessibilityIdentifier("authorization-sign-in")
        }.padding(.leading, 14).padding(.trailing, 6).padding(.vertical, 2)
            .chatGlass(in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .padding(.horizontal, 16)
            .accessibilityElement(children: .contain).accessibilityIdentifier("authorization-banner")
    }
}
