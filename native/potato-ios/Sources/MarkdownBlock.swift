import Foundation

enum MarkdownBlock: Equatable {
    case heading(Int, String), paragraph(String), list(String, String, level: Int = 0), quote(String), code(String, String), table([[String]]), divider
    static func parse(_ text: String, streaming: Bool = false) -> [MarkdownBlock] {
        let lines = text.components(separatedBy: .newlines)
        var result: [MarkdownBlock] = [], paragraph: [String] = [], listIndents: [Int] = []
        var index = 0
        func flush() { if !paragraph.isEmpty { result.append(.paragraph(paragraph.joined(separator: "\n"))); paragraph = [] } }
        while index < lines.count {
            let line = lines[index], trimmed = line.trimmingCharacters(in: .whitespaces), indent = indentation(line)
            // Don't append an unfinished block marker to the preceding paragraph:
            // it would disappear from that paragraph as soon as its syntax resolves.
            if streaming, index == lines.count - 1, pendingBlockMarker(trimmed) { break }
            let item = listItem(trimmed)
            if item == nil, indent == 0, !trimmed.isEmpty { listIndents = [] }
            if let fence = openingFence(trimmed) {
                flush(); let language = String(trimmed.dropFirst(fence.count)).trimmingCharacters(in: .whitespaces); var code: [String] = []; index += 1
                while index < lines.count && !closesFence(lines[index], character: fence.character, count: fence.count) {
                    let tail = lines[index].trimmingCharacters(in: .whitespaces)
                    // A split closing fence must not briefly become an extra code
                    // line. A following non-marker character makes it ordinary code.
                    if streaming, index == lines.count - 1,
                       tail.isEmpty || tail.allSatisfy({ $0 == fence.character }) { break }
                    // A fence nested in a list item is indented; its code is not.
                    code.append(String(lines[index].dropFirst(min(indent, indentation(lines[index]))))); index += 1
                }
                result.append(.code(language, code.joined(separator: "\n")))
            } else if trimmed.isEmpty { flush() }
            else if trimmed == "---" || trimmed == "***" { flush(); result.append(.divider) }
            else if trimmed.hasPrefix("#"), let space = trimmed.firstIndex(of: " "), trimmed[..<space].allSatisfy({ $0 == "#" }) {
                flush(); result.append(.heading(trimmed.distance(from: trimmed.startIndex, to: space), String(trimmed[trimmed.index(after: space)...])))
            } else if let item {
                flush()
                while let last = listIndents.last, last > indent { listIndents.removeLast() }
                if listIndents.last != indent { listIndents.append(indent) }
                result.append(.list(item.marker, item.text, level: min(listIndents.count - 1, 3)))
            } else if trimmed.hasPrefix(">") {
                flush(); var quoted = [quoteLine(trimmed)]
                while index + 1 < lines.count, case let next = lines[index + 1].trimmingCharacters(in: .whitespaces), next.hasPrefix(">") {
                    quoted.append(quoteLine(next)); index += 1
                }
                result.append(.quote(quoted.joined(separator: "\n")))
            } else if trimmed.contains("|"), index + 1 < lines.count, lines[index + 1].contains("---"), lines[index + 1].contains("|") {
                flush(); var rows = [cells(trimmed)]; index += 2
                while index < lines.count && lines[index].contains("|") { rows.append(cells(lines[index])); index += 1 }
                index -= 1
                // GFM: the header fixes the column count; short rows get empty cells, extra cells are dropped.
                let columns = rows[0].count
                result.append(.table(rows.map { Array(($0 + Array(repeating: "", count: max(0, columns - $0.count))).prefix(columns)) }))
            } else if paragraph.isEmpty, indent > 0, case .list(let marker, let value, let level)? = result.last {
                // An indented line after a list item continues that item.
                result[result.count - 1] = .list(marker, value + "\n" + trimmed, level: level)
            } else { paragraph.append(line) }
            index += 1
        }
        flush(); return result
    }
    private static func listItem(_ line: String) -> (marker: String, text: String)? {
        if line.hasPrefix("- [x] ") || line.hasPrefix("- [ ] ") { return (line.hasPrefix("- [x]") ? "☑" : "☐", String(line.dropFirst(6))) }
        if line.hasPrefix("- ") || line.hasPrefix("* ") || line.hasPrefix("+ ") { return ("•", String(line.dropFirst(2))) }
        guard let range = line.range(of: #"^\d+[.)] "#, options: .regularExpression) else { return nil }
        return (String(line[range]).trimmingCharacters(in: .whitespaces), String(line[range.upperBound...]))
    }
    private static func pendingBlockMarker(_ line: String) -> Bool {
        guard let first = line.first else { return false }
        if first == "`" || first == "~" {
            return line.allSatisfy { $0 == first } || openingFence(line) != nil
        }
        if first == "#", line.count <= 6, line.allSatisfy({ $0 == "#" }) { return true }
        if ["-", "--", "---", "*", "**", "***", "+", ">", "- [", "- [x", "- [x]", "- [ ", "- [ ]"].contains(line) { return true }
        return line.range(of: #"^\d+[.)]?$"#, options: .regularExpression) != nil
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
    private static func indentation(_ line: String) -> Int { line.prefix(while: { $0 == " " || $0 == "\t" }).count }
    private static func quoteLine(_ line: String) -> String {
        let body = line.dropFirst()
        return String(body.hasPrefix(" ") ? body.dropFirst() : body)
    }
    /// Only the outer pipes are borders, so empty first and last cells survive; `\|` is a literal pipe.
    private static func cells(_ line: String) -> [String] {
        var row = Substring(line.trimmingCharacters(in: .whitespaces))
        if row.hasPrefix("|") { row = row.dropFirst() }
        if row.hasSuffix("|") && !row.hasSuffix("\\|") { row = row.dropLast() }
        var cells = [""]
        while let character = row.popFirst() {
            if character == "\\", row.first == "|" { cells[cells.count - 1].append(row.removeFirst()) }
            else if character == "|" { cells.append("") }
            else { cells[cells.count - 1].append(character) }
        }
        return cells.map { $0.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: #"<br\s*/?>"#, with: "\n", options: [.regularExpression, .caseInsensitive]) }
    }
}
