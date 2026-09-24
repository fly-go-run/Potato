import Foundation
import Observation

enum AppLanguage: String, Codable, CaseIterable {
    case system, english = "en", simplifiedChinese = "zh-Hans"

    init(from decoder: Decoder) throws {
        self = Self(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .system
    }
    var title: String {
        switch self {
        case .system: return L10n.tr("跟随系统")
        case .english: return "English"
        case .simplifiedChinese: return "简体中文"
        }
    }
    func resolved(preferredLanguages: [String] = Locale.preferredLanguages) -> Self {
        guard self == .system else { return self }
        for language in preferredLanguages {
            let base = language.replacingOccurrences(of: "_", with: "-").lowercased().split(separator: "-").first
            if base == "zh" { return .simplifiedChinese }
            if base == "en" { return .english }
        }
        return .english
    }
}

/// Observation keeps computed labels current without recreating conversations or editors.
/// The lock also permits localized service errors to be read off the main thread.
@Observable final class AppLocalization {
    static let shared = AppLocalization()
    @ObservationIgnored private let lock = NSLock()
    @ObservationIgnored private var storedSelection: AppLanguage = .system
    var selection: AppLanguage {
        get {
            access(keyPath: \.selection)
            lock.lock(); defer { lock.unlock() }
            return storedSelection
        }
        set {
            withMutation(keyPath: \.selection) {
                lock.lock(); defer { lock.unlock() }
                storedSelection = newValue
            }
        }
    }
    var locale: Locale {
        Locale(identifier: selection.resolved() == .simplifiedChinese ? "zh_CN" : "en_US")
    }
    func refreshSystemLanguage() { selection = selection }
}

enum L10n {
    /// Keep interpolated values separate from lookup keys; never translate user content.
    struct Message: ExpressibleByStringLiteral, ExpressibleByStringInterpolation {
        var key: String
        var arguments: [String] = []
        init(stringLiteral value: String) { key = value }
        init(stringInterpolation: StringInterpolation) {
            key = stringInterpolation.key; arguments = stringInterpolation.arguments
        }
        struct StringInterpolation: StringInterpolationProtocol {
            var key = ""
            var arguments: [String] = []
            init(literalCapacity: Int, interpolationCount: Int) { key.reserveCapacity(literalCapacity) }
            mutating func appendLiteral(_ literal: String) { key += literal }
            mutating func appendInterpolation<T>(_ value: T) { key += "%@"; arguments.append(String(describing: value)) }
        }
    }
    static func tr(_ message: Message) -> String {
        translate(message.key, arguments: message.arguments, language: AppLocalization.shared.selection)
    }
    static func key(_ key: String) -> String { translate(key, language: AppLocalization.shared.selection) }
    static func translate(_ key: String, arguments: [String] = [], language: AppLanguage, bundle: Bundle = .main) -> String {
        let resolved = language.resolved()
        let localizedBundle = bundle.path(forResource: resolved.rawValue, ofType: "lproj").flatMap(Bundle.init(path:)) ?? bundle
        let template = localizedBundle.localizedString(forKey: key, value: key, table: nil)
        // Substitute only our %@ placeholders. Percent signs in prose and inserted
        // strings are literal, and arguments cannot become format directives.
        var output = "", remaining = template[...]
        for argument in arguments {
            guard let range = remaining.range(of: "%@") else { break }
            output += remaining[..<range.lowerBound] + argument
            remaining = remaining[range.upperBound...]
        }
        return output + remaining
    }
}
