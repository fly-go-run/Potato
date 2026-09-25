import SwiftUI
import SafariServices

struct WebSource: Codable, Equatable, Identifiable {
    var title: String
    var url: String
    var content: String
    var publishedDate: String?
    var id: String { url }
    var safeURL: URL? { guard let value = URL(string: url), value.scheme == "https", value.host != nil, value.user == nil, value.password == nil else { return nil }; return value }
    var displayTitle: String { title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? (safeURL?.lastPathComponent.isEmpty == false ? safeURL!.lastPathComponent : safeURL?.host ?? L10n.tr("网页来源")) : title }
}
struct WebSearchRun: Codable, Equatable, Identifiable {
    var id: String
    var query: String
    var state: String
    var results: [WebSource]
}
struct SearchSourcesButton: View {
    let runs: [WebSearchRun]
    var showsActivity = true
    @State private var showing = false
    private var sources: [WebSource] {
        var seen = Set<String>()
        return runs.flatMap(\.results).filter { $0.safeURL != nil && seen.insert($0.url).inserted }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let active = runs.last, active.state == "searching", showsActivity {
                Text(L10n.tr("正在搜索网页")).font(.subheadline).foregroundStyle(Palette.secondary).shimmering().accessibilityIdentifier("web-search-progress")
                Text(active.query).font(.caption).foregroundStyle(Palette.secondary).lineLimit(2)
            }
            if showsActivity && runs.last?.state == "failed" { Label(L10n.tr("搜索未成功"), systemImage: "exclamationmark.circle").font(.caption).foregroundStyle(Palette.secondary) }
            if showsActivity && runs.last?.state == "stopped" { Text(L10n.tr("搜索已停止")).font(.caption).foregroundStyle(Palette.secondary) }
            if !sources.isEmpty {
                Button { showing = true } label: { Label(L10n.tr("来源"), systemImage: "globe").font(.subheadline).frame(minHeight: 44) }.accessibilityIdentifier("search-sources")
            } else if showsActivity && runs.last?.state == "complete" { Text(L10n.tr("未找到可用的网页来源")).font(.caption).foregroundStyle(Palette.secondary) }
        }.sheet(isPresented: $showing) { SearchSourcesSheet(runs: runs, sources: sources) }
    }
}
private struct SearchSourcesSheet: View {
    let runs: [WebSearchRun]
    let sources: [WebSource]
    @State private var selected: WebSource?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                Section(L10n.tr("搜索记录")) { ForEach(runs) { run in Text(run.query).font(.subheadline) } }
                Section(L10n.tr("网页来源")) {
                    ForEach(sources) { source in
                        Button { selected = source } label: {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(source.displayTitle).font(.headline).foregroundStyle(Palette.ink)
                                Text(source.safeURL?.host ?? "").font(.caption).foregroundStyle(Palette.secondary)
                                if !source.content.isEmpty { Text(source.content).font(.footnote).foregroundStyle(Palette.secondary).lineLimit(4) }
                            }.padding(.vertical, 4).frame(maxWidth: .infinity, alignment: .leading)
                        }.accessibilityIdentifier("search-source-link")
                    }
                }
            }.navigationTitle(L10n.tr("网页来源")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { dismiss() }.accessibilityIdentifier("close-search-sources") } }
                .sheet(item: $selected) { source in if let url = source.safeURL { SourceBrowser(url: url).ignoresSafeArea() } }
        }
    }
}
private struct SourceBrowser: UIViewControllerRepresentable {
    let url: URL
    func makeUIViewController(context: Context) -> SFSafariViewController { SFSafariViewController(url: url) }
    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}
