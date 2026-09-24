import SwiftUI

struct RemoteModelChoice: Codable, Equatable {
    let provider_id: String
    let model: String
    var reasoning_effort: String?
    var arguments: [String: Any] { ["provider_id": provider_id, "model": model, "reasoning_effort": reasoning_effort as Any? ?? NSNull()] }
}
struct RemoteModelEntry: Decodable {
    let provider_id: String; let provider_name: String; let id: String; let name: String
    let effort_options: [String]; let default_effort: String?
    var identity: String { "\(provider_id.utf8.count):\(provider_id)\(id)" }
    var efforts: [String] { effort_options.reduce(into: []) { if !$0.contains($1) { $0.append($1) } } }
    var choice: RemoteModelChoice { RemoteModelChoice(provider_id: provider_id, model: id, reasoning_effort: default_effort) }
    func matches(_ choice: RemoteModelChoice) -> Bool { provider_id == choice.provider_id && id == choice.model }
}
struct RemoteModelCatalog: Decodable {
    let version: Int
    let models: [RemoteModelEntry]
    let active: RemoteModelChoice?
    func resolve(_ preferred: RemoteModelChoice?) throws -> RemoteModelChoice {
        guard version == 1 else { throw LocalFailure.message(L10n.tr("请更新手机与电脑，以使用模型设置。")) }
        guard let choice = preferred ?? active,
              let entry = models.first(where: { $0.matches(choice) }) else { throw LocalFailure.message(L10n.tr("所选模型不可用，请重新选择或在电脑配置模型。")) }
        if let effort = choice.reasoning_effort, !entry.effort_options.contains(effort), entry.default_effort != effort {
            throw LocalFailure.message(L10n.tr("所选思考设置已改变，请重新选择。"))
        }
        return choice
    }
}
struct RemoteModelOverview: Decodable { let model_catalog: RemoteModelCatalog? }

func remoteEffortName(_ value: String?) -> String {
    guard let value else { return L10n.tr("服务默认") }
    return ["none": L10n.tr("关闭"), "minimal": L10n.tr("极低"), "low": L10n.tr("低"), "medium": L10n.tr("中"), "high": L10n.tr("高"), "xhigh": L10n.tr("更高"), "max": L10n.tr("最高"), "ultra": L10n.tr("超高")][value] ?? value
}

struct RemoteModelPicker: View {
    let deviceName: String
    let catalog: RemoteModelCatalog?
    let issue: String?
    let loading: Bool
    let selection: RemoteModelChoice?
    let choose: (RemoteModelChoice?) -> Void
    let reload: () -> Void
    @Environment(\.dismiss) private var dismiss
    private var effective: RemoteModelChoice? { selection ?? catalog?.active }
    private var entry: RemoteModelEntry? { effective.flatMap { value in catalog?.models.first(where: { $0.matches(value) }) } }
    @State private var page = ModelPickerPage.models
    @State private var query = ""
    private var featured: [RemoteModelEntry] {
        var values = Array((catalog?.models ?? []).prefix(3))
        if let entry, !values.contains(where: { $0.identity == entry.identity }) {
            if values.count == 3 { values.removeLast() }; values.insert(entry, at: 0)
        }
        return values
    }
    var body: some View {
        ModelPickerSheet(page: $page, prefix: "remote", compactHeight: min(530, 300 + CGFloat(featured.count + 1) * 64), close: { dismiss() }) {
            if loading { ProgressView(L10n.tr("正在读取电脑上的模型…")) }
            if let issue { Text(issue).font(.footnote).foregroundStyle(.red) }
            if let catalog, catalog.version == 1 {
                switch page {
                case .models:
                    Section {
                        option(L10n.tr("跟随电脑"), detail: deviceName, selected: selection == nil, id: "remote-model-follow") { choose(nil) }
                        ForEach(featured, id: \.identity) { model in modelRow(model) }
                    }
                    if let effective, entry != nil {
                        Section { ModelPickerDisclosure(title: L10n.tr("思考"), value: remoteEffortName(effective.reasoning_effort), id: "remote-thinking-open") { page = .thinking } }
                    }
                    Section { ModelPickerDisclosure(title: L10n.tr("更多模型"), id: "remote-more-models") { page = .more } }
                case .thinking:
                    if let entry, let effective {
                        Section {
                            option(L10n.tr("服务默认"), selected: effective.reasoning_effort == nil, id: "remote-effort-default") { var value = effective; value.reasoning_effort = nil; choose(value) }
                            ForEach(entry.efforts, id: \.self) { effort in
                                option(remoteEffortName(effort), selected: effective.reasoning_effort == effort, id: "remote-effort-\(effort)") { var value = effective; value.reasoning_effort = effort; choose(value) }
                            }
                            if let effort = effective.reasoning_effort, !entry.effort_options.contains(effort) { Label(L10n.tr("电脑已配置：\(effort)"), systemImage: "checkmark").font(.footnote) }
                        } footer: { Text(entry.efforts.isEmpty ? L10n.tr("这台电脑未提供可选档位。使用服务默认，或保留电脑已有配置。") : L10n.tr("用于下一轮任务。已发送的任务保留原配置。")) }
                    }
                case .more:
                    Section {
                        TextField(L10n.tr("搜索模型"), text: $query).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("remote-model-search")
                        ForEach(catalog.models.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) || $0.id.localizedCaseInsensitiveContains(query) || $0.provider_name.localizedCaseInsensitiveContains(query) }, id: \.identity) { model in modelRow(model) }
                    }
                    Section { Button(L10n.tr("刷新模型列表"), systemImage: "arrow.clockwise", action: reload).disabled(loading).accessibilityIdentifier("remote-model-refresh") }
                }
            } else if !loading && issue == nil {
                Text(L10n.tr("这台电脑尚不支持手机选择模型，请更新电脑端。发送时仍跟随电脑设置。")).font(.footnote).foregroundStyle(.secondary)
            }
            if page == .models && (issue != nil || catalog == nil) {
                Button(L10n.tr("重新读取"), action: reload).disabled(loading).accessibilityIdentifier("remote-model-refresh")
            }
        }
    }
    private func modelRow(_ model: RemoteModelEntry) -> some View {
        option(model.name, detail: model.provider_name, selected: selection.map(model.matches) ?? false, id: "remote-model-\(model.id)") {
            if selection.map(model.matches) != true { choose(model.choice) }
            if page == .more { query = ""; page = .models }
        }
    }
    private func option(_ title: String, detail: String? = nil, selected: Bool, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                VStack(alignment: .leading, spacing: 4) { Text(title); if let detail { Text(detail).font(.caption).foregroundStyle(.secondary) } }
                Spacer(minLength: 8)
                if selected { Image(systemName: "checkmark").fontWeight(.semibold).foregroundStyle(Palette.accent).accessibilityHidden(true) }
            }.padding(.vertical, 5).frame(minHeight: 44).contentShape(Rectangle())
        }.accessibilityIdentifier(id).accessibilityValue(selected ? L10n.tr("已选择") : L10n.tr("未选择"))
    }
}
