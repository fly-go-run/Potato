import SwiftUI

struct DraftHistoryView: View {
    @Binding var draft: WorkingDraft
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                if draft.versionHistory.isEmpty {
                    ContentUnavailableView(L10n.tr("还没有历史版本"), systemImage: "clock.arrow.circlepath")
                } else {
                    Section(L10n.tr("以前的版本")) {
                        ForEach(draft.versionHistory.reversed()) { version in
                            NavigationLink(value: version.id) {
                                VStack(alignment: .leading, spacing: 6) {
                                    Text(version.title).font(.headline).lineLimit(2)
                                    Text(L10n.key(version.reason) + " · " + version.date.formatted(.dateTime.year().month().day().hour().minute().second().locale(AppLocalization.shared.locale))).font(.caption).foregroundStyle(Palette.secondary)
                                    Text(version.markdown.components(separatedBy: .newlines).filter { !$0.isEmpty && !$0.hasPrefix("#") }.joined(separator: " ")).font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(2)
                                }.padding(.vertical, 6)
                            }.accessibilityIdentifier("draft-version")
                        }
                    }
                }
            }.scrollContentBackground(.hidden).background(Palette.canvas)
                .navigationTitle(L10n.tr("文稿版本")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { dismiss() } } }
                .navigationDestination(for: UUID.self) { id in
                    if let version = draft.versionHistory.first(where: { $0.id == id }) {
                        ScrollView { MarkdownContent(text: version.markdown).padding(22) }
                            .background(Palette.canvas).navigationTitle(L10n.tr("版本预览")).navigationBarTitleDisplayMode(.inline)
                            .safeAreaInset(edge: .bottom) {
                                Button(L10n.tr("恢复此版本")) { draft.restore(version); dismiss() }
                                    .buttonStyle(.borderedProminent).foregroundStyle(Palette.onAccent).controlSize(.large).frame(maxWidth: .infinity).padding(16).background(.regularMaterial)
                                    .accessibilityIdentifier("restore-draft-version")
                            }
                    }
                }
        }
    }
}
