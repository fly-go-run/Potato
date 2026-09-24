import Foundation

enum MarkdownBlock: Equatable {
    case heading(Int, String), paragraph(String), list(String, String), quote(String), code(String, String), table([[String]]), divider
    static func parse(_ text: String, streaming: Bool = false) -> [MarkdownBlock] {
        let lines = text.components(separatedBy: .newlines)
        var result: [MarkdownBlock] = [], paragraph: [String] = []
        var index = 0
        func flush() { if !paragraph.isEmpty { result.append(.paragraph(paragraph.joined(separator: "\n"))); paragraph = [] } }
        while index < lines.count {
            let line = lines[index], trimmed = line.trimmingCharacters(in: .whitespaces)
            // Don't append an unfinished block marker to the preceding paragraph:
            // it would disappear from that paragraph as soon as its syntax resolves.
            if streaming, index == lines.count - 1, pendingBlockMarker(trimmed) { break }
            if let fence = openingFence(trimmed) {
                flush(); let language = String(trimmed.dropFirst(fence.count)).trimmingCharacters(in: .whitespaces); var code: [String] = []; index += 1
                while index < lines.count && !closesFence(lines[index], character: fence.character, count: fence.count) {
                    let tail = lines[index].trimmingCharacters(in: .whitespaces)
                    // A split closing fence must not briefly become an extra code
                    // line. A following non-marker character makes it ordinary code.
                    if streaming, index == lines.count - 1,
                       tail.isEmpty || tail.allSatisfy({ $0 == fence.character }) { break }
                    code.append(lines[index]); index += 1
                }
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
    private static func pendingBlockMarker(_ line: String) -> Bool {
        guard let first = line.first else { return false }
        if first == "`" || first == "~" {
            return line.allSatisfy { $0 == first } || openingFence(line) != nil
        }
        if first == "#", line.count <= 6, line.allSatisfy({ $0 == "#" }) { return true }
        if ["-", "--", "---", "*", "**", "***", ">", "- [", "- [x", "- [x]", "- [ ", "- [ ]"].contains(line) { return true }
        return line.range(of: #"^\d+\.?$"#, options: .regularExpression) != nil
    }
    private static func openingFence(_ line: String) -> (character: Character, count: Int)? {
        guard let character = line.first, character == "`" || character == "~" else { return nil }
        let count = line.prefix(while: { $0 == character }).count
        guard count >= 3 else { return nil }
        return (character, count)
    }
    private static func closesFence(_ line: String, character: Character, count: Int) -> Bool {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        let markers = trimmed.prefix(while: { $0 == character }).count
        return markers >= count && trimmed.dropFirst(markers).trimmingCharacters(in: .whitespaces).isEmpty
    }
    private static func cells(_ line: String) -> [String] {
        line.trimmingCharacters(in: CharacterSet(charactersIn: " |" )).components(separatedBy: "|").map { $0.trimmingCharacters(in: .whitespaces) }
    }
}
