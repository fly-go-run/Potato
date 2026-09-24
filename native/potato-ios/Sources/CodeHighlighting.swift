import SwiftUI

/// A display-only lexer. Never rewrites source, evaluates code, or guesses an unknown language.
enum CodeHighlighting {
    enum Kind: String { case comment, string, keyword, number, function, type, markup }
    struct Token: Equatable { let range: NSRange; let kind: Kind }
    struct Rule { let kind: Kind; let pattern: String }
    struct Grammar {
        let rules: [Rule]
        let expression: NSRegularExpression
        init(_ rules: [Rule]) {
            self.rules = rules
            // Named outer groups let individual rules use noncapturing groups freely.
            let pattern = rules.enumerated().map { "(?<t\($0.offset)>\($0.element.pattern))" }.joined(separator: "|")
            expression = try! NSRegularExpression(pattern: pattern, options: [.anchorsMatchLines])
        }
    }

    static func language(_ info: String) -> String {
        let name = info.split(whereSeparator: \.isWhitespace).first.map(String.init)?.lowercased() ?? ""
        return aliases[name] ?? name
    }
    private static let aliases = ["py": "python", "python3": "python", "js": "javascript", "jsx": "javascript",
                                  "ts": "typescript", "tsx": "typescript", "md": "markdown", "mdown": "markdown",
                                  "sh": "shell", "bash": "shell", "zsh": "shell", "yml": "yaml",
                                  "htm": "html", "c++": "cpp", "cc": "cpp", "c#": "csharp", "rs": "rust"]
    static func tokens(_ code: String, language info: String) -> [Token] {
        // Bound synchronous work while a model streams; oversized/unknown blocks stay readable.
        guard code.utf8.count <= 64_000, let grammar = grammars[language(info)] else { return [] }
        return grammar.expression.matches(in: code, range: NSRange(code.startIndex..., in: code)).compactMap { match in
            for (index, rule) in grammar.rules.enumerated() {
                let range = match.range(withName: "t\(index)")
                if range.location != NSNotFound { return Token(range: range, kind: rule.kind) }
            }
            return nil
        }
    }
    static func attributed(_ code: String, language: String, dark: Bool) -> AttributedString {
        let source = code as NSString
        var result = AttributedString(), offset = 0
        for token in tokens(code, language: language) {
            result.append(AttributedString(source.substring(with: NSRange(location: offset, length: token.range.location - offset))))
            var part = AttributedString(source.substring(with: token.range))
            part.foregroundColor = color(token.kind, dark: dark)
            result.append(part); offset = NSMaxRange(token.range)
        }
        result.append(AttributedString(source.substring(from: offset)))
        return result
    }
    private static func color(_ kind: Kind, dark: Bool) -> Color {
        let hex: UInt32
        switch kind {
        case .comment: hex = dark ? 0xA0A7B2 : 0x606976
        case .string: hex = dark ? 0xA5D6A7 : 0x28623B
        case .keyword: hex = dark ? 0xD8ABF3 : 0x8035A6
        case .number: hex = dark ? 0xF0BC80 : 0x935017
        case .function: hex = dark ? 0x91C6FF : 0x225FA6
        case .type: hex = dark ? 0x81D5DA : 0x1B6872
        case .markup: hex = dark ? 0xF49AA6 : 0xA62E49
        }
        return Color(red: Double((hex >> 16) & 255) / 255, green: Double((hex >> 8) & 255) / 255, blue: Double(hex & 255) / 255)
    }

    private static let quoted = #""(?:\\.|[^"\\\r\n])*(?:"|$)|'(?:\\.|[^'\\\r\n])*(?:'|$)"#
    private static let tripleQuoted = #""""[\s\S]*?(?:"""|\z)|'''[\s\S]*?(?:'''|\z)"#
    private static let number = #"\b(?:0[xX][\da-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)\b"#
    private static func words(_ value: String) -> String { "\\b(?:" + value.split(separator: " ").joined(separator: "|") + ")\\b" }
    private static func programming(_ keywords: String, comments: String = #"//[^\r\n]*|/\*[\s\S]*?(?:\*/|\z)"#, strings: String = quoted, types: String = "") -> Grammar {
        var rules = [Rule(kind: .comment, pattern: comments), Rule(kind: .string, pattern: strings),
                     Rule(kind: .keyword, pattern: words(keywords)), Rule(kind: .number, pattern: number)]
        if !types.isEmpty { rules.append(Rule(kind: .type, pattern: words(types))) }
        rules.append(Rule(kind: .function, pattern: #"\b[A-Za-z_][A-Za-z_0-9]*(?=\s*\()"#))
        return Grammar(rules)
    }
    private static let grammars: [String: Grammar] = {
        let cWords = "if else for while do return break continue switch case default const static void int float double char bool struct enum typedef sizeof null true false"
        let jsWords = "async await break case catch class const continue debugger default delete do else export extends false finally for from function get if import in instanceof let new null of return set static super switch this throw true try typeof undefined var void while yield interface type implements public private protected readonly declare namespace as keyof"
        let js = programming(jsWords, strings: #"`(?:\\.|[^`\\])*(?:`|\z)|"# + quoted)
        let xml = Grammar([Rule(kind: .comment, pattern: #"<!--[\s\S]*?(?:-->|\z)"#),
                           Rule(kind: .string, pattern: quoted), Rule(kind: .markup, pattern: #"</?[A-Za-z][\w:.-]*|/?>|<!DOCTYPE\b"#),
                           Rule(kind: .type, pattern: #"[\w:-]+(?=\s*=)"#), Rule(kind: .number, pattern: #"&(?:#\w+|\w+);"#)])
        return [
            "python": programming("and as assert async await break class continue def del elif else except False finally for from global if import in is lambda None nonlocal not or pass raise return True try while with yield match case", comments: #"#[^\r\n]*"#, strings: tripleQuoted + "|" + quoted, types: "str int float bool list dict set tuple bytes object self"),
            "javascript": js, "typescript": js,
            "swift": programming("actor any as associatedtype async await break case catch class continue convenience default defer deinit didSet do else enum extension false fileprivate final for func guard if import in indirect init inout internal is lazy let mutating nil nonisolated open operator override private protocol public repeat required rethrows return self some static struct subscript super switch throw throws true try typealias var weak where while willSet", strings: tripleQuoted + "|" + quoted, types: "String Int Double Float Bool Array Dictionary Set Optional Self"),
            "rust": programming("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while", types: "String Vec Option Result Some None Ok Err bool i32 i64 u32 u64 usize"),
            "go": programming("break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var true false nil", strings: #"`[^`]*(?:`|\z)|"# + quoted),
            "c": programming(cWords), "cpp": programming(cWords + " auto class namespace public private protected virtual template typename using new delete nullptr include"),
            "java": programming(cWords + " abstract assert boolean byte catch class extends final finally implements import instanceof interface long native new package private protected public short super synchronized this throw throws transient try volatile"),
            "csharp": programming(cWords + " abstract as async await base catch class decimal delegate event finally foreach get in interface internal is lock namespace new object out override params private protected public readonly ref sealed set string this throw try using var virtual"),
            "shell": programming("if then else elif fi for in do done while until case esac function select time export local readonly return", comments: #"#[^\r\n]*"#, strings: quoted + #"|\$\{[^}]*\}|\$[A-Za-z_][A-Za-z_0-9]*"#),
            "json": Grammar([Rule(kind: .type, pattern: #""(?:\\.|[^"\\])*"(?=\s*:)"#), Rule(kind: .string, pattern: quoted), Rule(kind: .keyword, pattern: words("true false null")), Rule(kind: .number, pattern: number)]),
            "yaml": Grammar([Rule(kind: .comment, pattern: #"#[^\r\n]*"#), Rule(kind: .string, pattern: quoted), Rule(kind: .type, pattern: #"[\w.-]+(?=\s*:(?:\s|$))"#), Rule(kind: .keyword, pattern: #"(?i:\b(?:true|false|null|yes|no)\b)|^[ \t]*---"#), Rule(kind: .number, pattern: number)]),
            "sql": programming("(?i:select|from|where|insert|into|values|update|set|delete|create|alter|drop|table|index|join|left|right|inner|outer|on|as|and|or|not|null|true|false|order|by|group|having|limit|offset|distinct|union|all|case|when|then|else|end|with|asc|desc|count|sum|avg)", comments: #"--[^\r\n]*|/\*[\s\S]*?(?:\*/|\z)"#),
            "html": xml, "xml": xml,
            "css": Grammar([Rule(kind: .comment, pattern: #"/\*[\s\S]*?(?:\*/|\z)"#), Rule(kind: .string, pattern: quoted), Rule(kind: .type, pattern: #"(?:--)?[A-Za-z-]+(?=\s*:)"#), Rule(kind: .number, pattern: #"#[\da-fA-F]{3,8}\b|\b\d+(?:\.\d+)?(?:px|em|rem|vh|vw|s|%)?"#), Rule(kind: .keyword, pattern: #"@[\w-]+|!important\b"#), Rule(kind: .function, pattern: #"[\w-]+(?=\()"#)]),
            "markdown": Grammar([Rule(kind: .string, pattern: #"(?m:^[ \t]*```[^\r\n]*\r?\n[\s\S]*?(?:^[ \t]*```[ \t]*$|\z))|`[^`\r\n]+`"#),
                                 Rule(kind: .markup, pattern: #"^[ \t]{0,3}#{1,6}(?=\s).*|\*\*[^*\r\n]+\*\*|__[^_\r\n]+__|^[ \t]*(?:[-+*]|\d+\.)[ \t]+|^[ \t]*>+"#),
                                 Rule(kind: .function, pattern: #"!?\[[^\]\r\n]*\]\([^\)\r\n]*\)"#), Rule(kind: .comment, pattern: #"<!--[\s\S]*?(?:-->|\z)"#)])
        ]
    }()
}

/// One cached value per mounted code view, so unchanged blocks are not lexed on every stream tick.
private final class CodeHighlightCache {
    var source = "", language = "", dark = false
    var value = AttributedString()
    func render(_ source: String, language: String, dark: Bool) -> AttributedString {
        if self.source != source || self.language != language || self.dark != dark {
            value = CodeHighlighting.attributed(source, language: language, dark: dark)
            self.source = source; self.language = language; self.dark = dark
        }
        return value
    }
}

struct HighlightedCode: View {
    let code: String
    let language: String
    @Environment(\.colorScheme) private var scheme
    @State private var cache = CodeHighlightCache()
    var body: some View {
        Text(cache.render(code, language: language, dark: scheme == .dark))
            .font(.system(.footnote, design: .monospaced)).textSelection(.enabled)
    }
}
