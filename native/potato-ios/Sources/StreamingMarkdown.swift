import SwiftUI

/// Presentation only. The complete received text and replay cursor remain in WorkspaceStore.
struct StreamingTextBuffer {
    private(set) var visible = ""
    private var characters: [Character] = []
    private var offset = 0
    private var deadline = 0.0
    var pending: Bool { offset < characters.count }

    init(visible: String = "") { self.visible = visible }

    mutating func update(_ text: String, animated: Bool, now: Double) {
        guard animated, text.hasPrefix(visible) else { reset(text); return }
        characters = Array(text)
        offset = visible.count
        deadline = now + 0.24
        // Do not make the first character wait for the display timer.
        if visible.isEmpty, let first = characters.first { visible.append(first); offset = 1 }
    }
    mutating func advance(now: Double) {
        guard pending else { return }
        let remainingTicks = max(1, Int(ceil((deadline - now) / 0.032)))
        let count = max(1, Int(ceil(Double(characters.count - offset) / Double(remainingTicks))))
        let end = min(characters.count, offset + count)
        visible.append(contentsOf: characters[offset..<end]); offset = end
        if !pending { characters.removeAll(keepingCapacity: false); offset = 0 }
    }
    mutating func reset(_ text: String) {
        visible = text; characters.removeAll(keepingCapacity: false); offset = 0
    }
}

struct StreamingMarkdown: View {
    let text: String
    let streaming: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var phase
    @State private var buffer = StreamingTextBuffer()
    private var animated: Bool { streaming && !reduceMotion && phase == .active }
    private var now: Double { ProcessInfo.processInfo.systemUptime }

    init(text: String, streaming: Bool) {
        self.text = text
        self.streaming = streaming
        // A remounted reply already has received text. Replaying it from an empty
        // buffer collapses the row and makes scroll-to-bottom jump backwards.
        _buffer = State(initialValue: StreamingTextBuffer(visible: text))
    }

    var body: some View {
        MarkdownContent(text: animated ? buffer.visible : text, streaming: streaming)
            .onChange(of: text, initial: true) { _, value in buffer.update(value, animated: animated, now: now) }
            .onChange(of: animated) { _, _ in buffer.reset(text) }
            .task(id: animated) {
                guard animated else { return }
                while !Task.isCancelled {
                    do { try await Task.sleep(for: .milliseconds(32)) } catch { return }
                    if buffer.pending { buffer.advance(now: now) }
                }
            }
    }
}

/// Reuse unchanged paragraphs' attributed text while only the trailing block grows.
/// Kept per view, with entries pruned to the current document on each parse.
final class MarkdownRenderCache {
    private var source: String?
    private var streaming = false
    private var parsed: [MarkdownBlock] = []
    private var attributed: [String: AttributedString] = [:]
    func blocks(for text: String, streaming: Bool = false) -> [MarkdownBlock] {
        guard text != source || streaming != self.streaming else { return parsed }
        source = text; self.streaming = streaming; parsed = MarkdownBlock.parse(text, streaming: streaming)
        var current = Set<String>()
        for block in parsed {
            switch block {
            case .heading(_, let value), .paragraph(let value), .list(_, let value), .quote(let value): current.insert(value)
            case .table(let rows): rows.forEach { current.formUnion($0) }
            default: break
            }
        }
        attributed = attributed.filter { current.contains($0.key) }
        return parsed
    }
    func inline(_ value: String) -> AttributedString {
        if let cached = attributed[value] { return cached }
        let result = (try? AttributedString(markdown: value, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(value)
        attributed[value] = result
        return result
    }
}
