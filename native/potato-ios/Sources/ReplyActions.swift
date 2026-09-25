import SwiftUI
import AVFoundation

@MainActor
final class ReplySpeech: NSObject, ObservableObject, AVSpeechSynthesizerDelegate {
    @Published var messageID: UUID?
    private let synthesizer = AVSpeechSynthesizer()
    override init() { super.init(); synthesizer.delegate = self }
    func toggle(_ message: ChatMessage) {
        if messageID == message.id { stop(); return }
        stop()
        messageID = message.id
        let utterance = AVSpeechUtterance(string: message.displayText)
        synthesizer.speak(utterance)
    }
    func stop() { messageID = nil; synthesizer.stopSpeaking(at: .immediate) }
    nonisolated func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        Task { @MainActor in if !self.synthesizer.isSpeaking { self.messageID = nil } }
    }
}

/// Quiet enough to sit under every reply: a small glyph, lighter than the text,
/// still a full-height touch target. The row lines its first glyph up with the reply.
struct ReplyActionGlyph: View {
    let symbol: String
    static let width: CGFloat = 36
    static let inset: CGFloat = (width - 15) / 2
    var body: some View {
        Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(Palette.secondary.opacity(0.75))
            .frame(width: Self.width, height: 44).contentShape(Rectangle())
    }
}

struct ReplyActions: View {
    let message: ChatMessage
    let isLast: Bool
    let busy: Bool
    @ObservedObject var store: WorkspaceStore
    @ObservedObject var speech: ReplySpeech
    let retry: () -> Void
    let save: () -> Void
    let share: () -> Void
    @State private var copied = false
    @State private var librarySaved = false
    @State private var panel: Panel?
    @State private var afterDismiss: (() -> Void)?
    @Environment(\.dynamicTypeSize) private var typeSize
    private enum Panel: String, Identifiable {
        case more, models, selection
        var id: String { rawValue }
    }
    private var empty: Bool { message.displayText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var body: some View {
        HStack(spacing: 0) {
            action(symbol: copied ? "checkmark" : "doc.on.doc", label: copied ? L10n.tr("已复制回复") : L10n.tr("复制回复"), id: "copy-reply") {
                UIPasteboard.general.string = message.displayText; copied = true
            }.disabled(empty)
            if isLast { action(symbol: "arrow.clockwise", label: L10n.tr("重新生成"), id: "retry-message", perform: retry).disabled(busy) }
            action(symbol: "ellipsis", label: L10n.tr("回复更多操作"), id: "reply-more") { panel = .more }
            Spacer(minLength: 0)
        }.padding(.leading, -ReplyActionGlyph.inset)
            .alert(L10n.tr("已保存到资料库"), isPresented: $librarySaved) { Button(L10n.tr("好"), role: .cancel) {} }
            .onChange(of: message.displayText) { _, _ in copied = false }
            .task(id: copied) { if copied { try? await Task.sleep(for: .seconds(2)); if !Task.isCancelled { copied = false } } }
            .sheet(item: $panel, onDismiss: {
                let action = afterDismiss; afterDismiss = nil; action?()
            }) { current in
                switch current {
                case .more: morePanel
                case .selection: ReplyTextSelection(text: message.displayText)
                case .models:
                    LocalModelPicker(store: store, openConnection: {}, retryMessage: message, regenerate: { choice in
                        speech.stop(); return store.retry(messageID: message.id, using: choice)
                    })
                }
            }
    }
    private func action(symbol: String, label: String, id: String, perform: @escaping () -> Void) -> some View {
        Button(action: perform) { ReplyActionGlyph(symbol: symbol) }
            .buttonStyle(.plain).accessibilityLabel(label).accessibilityIdentifier(id)
    }
    private var morePanel: some View {
        NavigationStack {
            List {
                if isLast {
                    Section {
                        Button { panel = .models } label: {
                            HStack(spacing: 12) {
                                Image(systemName: "arrow.triangle.2.circlepath").font(.system(size: 20)).frame(width: 22).accessibilityHidden(true)
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(L10n.tr("换模型重新回答"))
                                    if let choice = message.displayModelChoice {
                                        Text("\(store.settings.modelEntry(choice.model).name) · \(choice.thinkingLabel)")
                                            .font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                                Spacer(minLength: 8)
                                Image(systemName: "chevron.right").font(.footnote).foregroundStyle(.secondary)
                            }.frame(minHeight: 44)
                        }.disabled(busy).accessibilityIdentifier("reply-change-model")
                    }
                    .listRowBackground(Palette.surface)
                    .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
                }
                Section {
                    row(L10n.tr("分享回复"), symbol: "square.and.arrow.up", id: "share-reply") { finishPanel(share) }
                    row(speech.messageID == message.id ? L10n.tr("停止朗读") : L10n.tr("朗读回复"), symbol: speech.messageID == message.id ? "stop.circle" : "speaker.wave.2", id: "read-reply") { finishPanel { speech.toggle(message) } }
                    row(L10n.tr("选择文字"), symbol: "text.cursor", id: "select-reply-text") { panel = .selection }
                    row(L10n.tr("保存到资料库"), symbol: "books.vertical", id: "save-reply-library") {
                        finishPanel {
                            do {
                                _ = try store.saveLibraryText(message.displayText, source: LibrarySource(conversationID: store.selectedID, messageID: message.id))
                                librarySaved = true
                            } catch { store.error = error.localizedDescription }
                        }
                    }
                    row(L10n.tr("存为工作文稿"), symbol: "doc.badge.plus", id: "save-reply-document") { finishPanel(save) }
                }.disabled(empty)
                    .listRowBackground(Palette.surface)
                    .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
            }
            .contentMargins(.top, 16, for: .scrollContent)
            .listStyle(.insetGrouped).listSectionSpacing(16)
            .scrollContentBackground(.hidden).background(Palette.grouped)
            .navigationTitle(L10n.tr("回复操作")).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) {
                Button(L10n.tr("完成")) { panel = nil }.accessibilityIdentifier("reply-actions-done")
            } }
        }
        .tint(Palette.ink)
        .presentationDetents(typeSize.isAccessibilitySize ? [.large] : [.height(isLast ? 524 : 424), .large])
        .presentationDragIndicator(.visible).presentationCornerRadius(32)
    }
    private func row(_ title: String, symbol: String, id: String, perform: @escaping () -> Void) -> some View {
        Button(action: perform) {
            Label { Text(title) } icon: { Image(systemName: symbol).font(.system(size: 20)).frame(width: 22).accessibilityHidden(true) }.frame(minHeight: 44)
        }.accessibilityIdentifier(id)
    }
    private func finishPanel(_ action: @escaping () -> Void) { afterDismiss = action; panel = nil }
}

struct ReplyTextSelection: View {
    let text: String
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            SelectableReply(text: text).padding(.horizontal, 12)
                .navigationTitle(L10n.tr("选择文字")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { dismiss() }.accessibilityIdentifier("close-text-selection") } }
        }
    }
}
private struct SelectableReply: UIViewRepresentable {
    let text: String
    func makeUIView(context: Context) -> UITextView {
        let view = UITextView(); view.isEditable = false; view.isSelectable = true
        view.font = .preferredFont(forTextStyle: .body); view.adjustsFontForContentSizeCategory = true
        view.backgroundColor = .clear; view.textContainerInset = UIEdgeInsets(top: 20, left: 8, bottom: 24, right: 8)
        view.accessibilityIdentifier = "selectable-reply"; return view
    }
    func updateUIView(_ view: UITextView, context: Context) { if view.text != text { view.text = text } }
}
