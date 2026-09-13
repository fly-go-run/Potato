import Foundation

struct DraftItem: Identifiable, Codable {
    var id = UUID()
    var text: String
    var isDone = false
}

struct DraftSection: Identifiable, Codable {
    var id = UUID()
    var title: String
    var items: [DraftItem]
}

struct DraftRevision: Identifiable, Codable {
    var id = UUID()
    var date = Date()
    var reason: String
    var title: String
    var customMarkdown: String?
    var sections: [DraftSection]
    var notes: [String]
    var markdown: String {
        var draft = WorkingDraft(); draft.title = title; draft.customMarkdown = customMarkdown
        draft.sections = sections; draft.notes = notes; return draft.markdown
    }
}

struct WorkingDraft: Codable {
    // Optional preserves decoding compatibility with the first persisted release.
    var revisions: [DraftRevision]? = nil
    var versionHistory: [DraftRevision] { revisions ?? [] }
    var customMarkdown: String? = nil
    var title = "给周末留一点空白"
    var sections = [
        DraftSection(title: "周六 · 出门走走", items: [
            DraftItem(text: "去附近的公园走走，晒晒太阳"),
            DraftItem(text: "找一家喜欢的咖啡店，带上一本书")
        ]),
        DraftSection(title: "周日 · 慢慢收尾", items: [
            DraftItem(text: "整理一下房间，让下周更轻松"),
            DraftItem(text: "做一顿喜欢的饭，早点休息")
        ])
    ]
    var notes: [String] = []

    var markdown: String {
        if let customMarkdown { return customMarkdown }
        var result = "# \(title)\n\n只安排两件想做的事，给临时起意留点余地。\n"
        for section in sections {
            result += "\n## \(section.title)\n"
            for item in section.items {
                result += "\n- [\(item.isDone ? "x" : " ")] \(item.text)"
            }
            result += "\n"
        }
        if !notes.isEmpty {
            result += "\n## 补充想法\n" + notes.map { "\n- \($0)" }.joined()
        }
        return result
    }

    mutating func recordVersion(reason: String) {
        var versions = versionHistory
        versions.append(DraftRevision(reason: reason, title: title, customMarkdown: customMarkdown, sections: sections, notes: notes))
        revisions = versions
    }
    mutating func replaceContent(_ text: String, reason: String = "编辑前") {
        guard text != markdown else { return }
        recordVersion(reason: reason)
        customMarkdown = text
        title = String((text.components(separatedBy: .newlines).first(where: { !$0.trimmingCharacters(in: .whitespaces).isEmpty })?.trimmingCharacters(in: CharacterSet(charactersIn: "# ")) ?? "工作文稿").prefix(100))
    }
    mutating func restore(_ revision: DraftRevision) {
        guard revision.markdown != markdown else { return }
        recordVersion(reason: "恢复版本前")
        title = revision.title; customMarkdown = revision.customMarkdown
        sections = revision.sections; notes = revision.notes
    }
    mutating func toggleItem(sectionID: UUID, itemID: UUID) {
        guard let section = sections.firstIndex(where: { $0.id == sectionID }),
              let item = sections[section].items.firstIndex(where: { $0.id == itemID }) else { return }
        recordVersion(reason: "勾选前")
        sections[section].items[item].isDone.toggle()
    }
    mutating func toggleMarkdownCheckbox(_ ordinal: Int) {
        guard let customMarkdown else { return }
        var lines = customMarkdown.components(separatedBy: .newlines)
        var index = 0
        var inCode = false
        for position in lines.indices {
            let line = lines[position].trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("```") { inCode.toggle(); continue }
            guard !inCode && (line.hasPrefix("- [x] ") || line.hasPrefix("- [ ] ")) else { continue }
            if index == ordinal {
                recordVersion(reason: "勾选前")
                let from = line.hasPrefix("- [x] ") ? "- [x] " : "- [ ] "
                let to = line.hasPrefix("- [x] ") ? "- [ ] " : "- [x] "
                if let range = lines[position].range(of: from) { lines[position].replaceSubrange(range, with: to) }
                self.customMarkdown = lines.joined(separator: "\n"); return
            }
            index += 1
        }
    }
}
