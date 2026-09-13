import Foundation

enum MarkdownBlock: Equatable {
    case heading(Int, String), paragraph(String), list(String, String), quote(String), code(String, String), table([[String]]), divider
    static func parse(_ text: String) -> [MarkdownBlock] {
        let lines = text.components(separatedBy: .newlines)
        var result: [MarkdownBlock] = [], paragraph: [String] = []
        var index = 0
        func flush() { if !paragraph.isEmpty { result.append(.paragraph(paragraph.joined(separator: "\n"))); paragraph = [] } }
        while index < lines.count {
            let line = lines[index], trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                flush(); let language = String(trimmed.dropFirst(3)); var code: [String] = []; index += 1
                while index < lines.count && !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") { code.append(lines[index]); index += 1 }
                result.append(.code(language, code.joined(separator: "\n")))
            } else if trimmed.isEmpty { flush() }
            else if trimmed == "---" || trimmed == "***" { flush(); result.append(.divider) }
            else if trimmed.hasPrefix("#"), let space = trimmed.firstIndex(of: " "), trimmed[..<space].allSatisfy({ $0 == "#" }) {
                flush(); result.append(.heading(trimmed.distance(from: trimmed.startIndex, to: space), String(trimmed[trimmed.index(after: space)...])))
            } else if trimmed.hasPrefix("- [x] ") || trimmed.hasPrefix("- [ ] ") {
                flush(); result.append(.list(trimmed.hasPrefix("- [x]") ? "☑" : "☐", String(trimmed.dropFirst(6))))
            } else if trimmed.hasPrefix("- ") || trimmed.hasPrefix("* ") { flush(); result.append(.list("•", String(trimmed.dropFirst(2)))) }
            else if let range = trimmed.range(of: #"^\d+\. "#, options: .regularExpression) { flush(); result.append(.list(String(trimmed[range]).trimmingCharacters(in: .whitespaces), String(trimmed[range.upperBound...]))) }
            else if trimmed.hasPrefix("> ") { flush(); result.append(.quote(String(trimmed.dropFirst(2)))) }
            else if trimmed.contains("|"), index + 1 < lines.count, lines[index + 1].contains("---"), lines[index + 1].contains("|") {
                flush(); var rows = [cells(trimmed)]; index += 2
                while index < lines.count && lines[index].contains("|") { rows.append(cells(lines[index])); index += 1 }
                index -= 1; result.append(.table(rows))
            } else { paragraph.append(line) }
            index += 1
        }
        flush(); return result
    }
    private static func cells(_ line: String) -> [String] {
        line.trimmingCharacters(in: CharacterSet(charactersIn: " |" )).components(separatedBy: "|").map { $0.trimmingCharacters(in: .whitespaces) }
    }
}
