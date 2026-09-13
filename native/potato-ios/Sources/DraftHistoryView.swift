import SwiftUI

struct DraftHistoryView: View {
    @Binding var draft: WorkingDraft
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                Section {
                    Text("查看以前的内容，再决定是否恢复。恢复前也会保留当前文稿。").font(.subheadline).foregroundStyle(Palette.secondary)
                }
                if draft.versionHistory.isEmpty {
                    ContentUnavailableView("还没有历史版本", systemImage: "clock.arrow.circlepath", description: Text("编辑、勾选或替换文稿时，会在本机保留之前的内容。"))
                } else {
                    Section("以前的版本") {
                        ForEach(draft.versionHistory.reversed()) { version in
                            NavigationLink(value: version.id) {
                                VStack(alignment: .leading, spacing: 6) {
                                    Text(version.title).font(.headline).lineLimit(2)
                                    Text(version.reason + " · " + version.date.formatted(.dateTime.year().month().day().hour().minute().second().locale(Locale(identifier: "zh_CN")))).font(.caption).foregroundStyle(Palette.secondary)
                                    Text(version.markdown.components(separatedBy: .newlines).filter { !$0.isEmpty && !$0.hasPrefix("#") }.joined(separator: " ")).font(.subheadline).foregroundStyle(Palette.secondary).lineLimit(2)
                                }.padding(.vertical, 6)
                            }.accessibilityIdentifier("draft-version")
                        }
                    }
                }
            }.scrollContentBackground(.hidden).background(Palette.canvas)
                .navigationTitle("文稿版本").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("完成") { dismiss() } } }
                .navigationDestination(for: UUID.self) { id in
                    if let version = draft.versionHistory.first(where: { $0.id == id }) {
                        ScrollView { MarkdownContent(text: version.markdown).padding(22) }
                            .background(Palette.canvas).navigationTitle("版本预览").navigationBarTitleDisplayMode(.inline)
                            .safeAreaInset(edge: .bottom) {
                                Button("恢复此版本") { draft.restore(version); dismiss() }
                                    .buttonStyle(.borderedProminent).foregroundStyle(.white).controlSize(.large).frame(maxWidth: .infinity).padding(16).background(.regularMaterial)
                                    .accessibilityIdentifier("restore-draft-version")
                            }
                    }
                }
        }
    }
}
