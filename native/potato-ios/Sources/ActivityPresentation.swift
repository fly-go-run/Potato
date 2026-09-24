import SwiftUI
import ImageIO

/// Presentation only: raw tool input/output stays available behind each step.
struct ActivityStep: Identifiable {
    var id: String
    var title: String
    var symbol: String
    var state: String
    var input = ""
    var language = "plaintext"
    var output = ""
    var attachments: [Attachment] = []
    var sources: [WebSource] = []
    var reasoning = false
    var rawInput: String? = nil
    // Use only an excerpt actually supplied by the model; do not invent a reasoning summary.
    var timelineTitle: String {
        guard reasoning, let paragraph = output.split(whereSeparator: \.isNewline).first else { return title }
        let excerpt = paragraph.trimmingCharacters(in: .whitespacesAndNewlines)
        return excerpt.isEmpty ? title : String(excerpt.prefix(180)) + (excerpt.count > 180 ? "…" : "")
    }
    var statusSymbol: String {
        if failed { return "exclamationmark.circle" }
        if isRunning { return "clock" }
        if ["complete", "completed", "success"].contains(state) { return "checkmark.circle" }
        if ["stopped", "cancelled"].contains(state) { return "stop.circle" }
        return "questionmark.circle"
    }
    var isRunning: Bool { ["running", "searching", "streaming", "in_progress"].contains(state) }
    var failed: Bool { ["failed", "error"].contains(state) }
    var status: String {
        if isRunning { return L10n.tr("进行中") }
        switch state {
        case "complete", "completed", "success": return L10n.tr("已完成")
        case "failed", "error": return L10n.tr("未成功")
        case "stopped", "cancelled": return L10n.tr("已停止")
        default: return L10n.tr("状态待确认")
        }
    }
}

extension ChatMessage {
    mutating func recordActivity(_ id: String) {
        if activityOrder == nil { activityOrder = [] }
        if activityOrder?.contains(id) == false { activityOrder?.append(id) }
    }
    var activitySteps: [ActivityStep] {
        var steps: [ActivityStep] = []
        if let trace = displayReasoning, !trace.text.isEmpty {
            steps.append(ActivityStep(id: "reasoning", title: trace.state == .streaming ? L10n.tr("正在思考") : L10n.tr("思考记录 · \(Int(trace.elapsed)) 秒"), symbol: "sparkle", state: trace.state.rawValue, output: trace.text, reasoning: true))
        }
        steps += displaySearches.map { run in
            ActivityStep(id: "search:\(run.id)", title: L10n.tr("搜索：\(run.query)"), symbol: "magnifyingglass", state: run.state, input: run.query, output: run.results.isEmpty && run.state == "complete" ? L10n.tr("未找到可用的网页来源。") : "", sources: run.results)
        }
        steps += displayRecalls.map { run in
            ActivityStep(id: "recall:\(run.id)", title: L10n.tr("检索记忆与历史"), symbol: "clock.arrow.circlepath", state: run.state, output: ([run.message ?? ""] + run.sources.map { "\($0.title)\n\($0.text)" }).filter { !$0.isEmpty }.joined(separator: "\n\n"))
        }
        steps += displayCodeRuns.map { run in
            let files = displayAttachments.filter { run.attachmentIDs?.contains($0.id) == true }
            let names = run.result?.artifacts.map(\.name) ?? []
            let description = run.title?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = description.flatMap { $0.isEmpty ? nil : $0 } ?? names.first.map { L10n.tr("生成 \($0)") } ?? L10n.tr("执行代码")
            let result = run.result
            return ActivityStep(id: "code:\(run.id)", title: title, symbol: "terminal", state: run.state, input: run.code, language: "python", output: [result?.stdout, result?.text, result?.stderr, result?.error, run.message].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "\n\n"), attachments: files)
        }
        if let result = displayExecution {
            steps.append(ActivityStep(id: "execution", title: L10n.tr("运行代码"), symbol: "terminal", state: result.status, output: [result.stdout, result.text, result.stderr, result.error ?? ""].filter { !$0.isEmpty }.joined(separator: "\n\n")))
        }
        let order = selectedVersion != nil ? selectedVersion?.activityOrder : activityOrder
        // Old records lack interleaving information. Keep their stable order;
        // never infer an execution timestamp from content or file names.
        guard let order, !order.isEmpty else { return steps }
        return steps.enumerated().sorted { lhs, rhs in
            (order.firstIndex(of: lhs.element.id) ?? (order.count + lhs.offset)) < (order.firstIndex(of: rhs.element.id) ?? (order.count + rhs.offset))
        }.map(\.element)
    }
}

struct ReplyActivityView: View {
    let message: ChatMessage
    let storage: LocalStorage
    var onExpand: () -> Void
    var body: some View {
        ActivitySummaryView(steps: message.activitySteps, running: message.displayState == .streaming,
                            endState: message.displayState.rawValue, storage: storage, onExpand: onExpand)
    }
}

struct ActivitySummaryView: View {
    let steps: [ActivityStep]
    let running: Bool
    var confirmed = true
    var endState = "complete"
    var storage: LocalStorage? = nil
    var showsSpinner = true
    var identifier = "activity-summary"
    var onExpand: () -> Void = {}
    @State private var showing = false
    @State private var initialStepID: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private var thoughts: [ActivityStep] { steps.filter(\.reasoning) }
    private var tools: [ActivityStep] { steps.filter { !$0.reasoning } }
    private var current: ActivityStep? { tools.last(where: \.isRunning) }
    private func thoughtStatus(_ step: ActivityStep) -> String {
        if !confirmed { return L10n.tr("状态待确认") }
        if step.isRunning { return L10n.tr("正在思考") }
        if step.failed { return L10n.tr("已中断") }
        return step.status
    }
    private var title: String {
        if !confirmed { return L10n.tr("\(tools.count) 个步骤 · 状态待确认") }
        if running, showsSpinner, let current { return current.title }
        if endState == "stopped" || endState == "cancelled" { return L10n.tr("\(tools.count) 个步骤 · 已停止") }
        if endState == "failed" { return L10n.tr("\(tools.count) 个步骤 · 已中断") }
        return L10n.tr("\(tools.count) 个步骤")
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let thought = thoughts.last {
                Button { onExpand(); initialStepID = thought.id; showing = true } label: {
                    HStack(spacing: 9) {
                        Image(systemName: "sparkle").font(.system(size: 15)).accessibilityHidden(true)
                        Text((thought.isRunning || thought.failed || !confirmed || ["stopped", "cancelled"].contains(thought.state) ? thoughtStatus(thought) + " · " : "") + thought.timelineTitle)
                            .font(.subheadline).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: "chevron.right").font(.system(size: 11)).accessibilityHidden(true)
                    }.foregroundStyle(Palette.secondary).frame(minHeight: 44).contentShape(Rectangle())
                }.buttonStyle(.plain).accessibilityIdentifier(identifier + "-reasoning")
                    .accessibilityLabel(L10n.tr("\(thoughtStatus(thought))，\(thought.timelineTitle)，查看完整思考过程"))
            }
            if !tools.isEmpty {
                Button { onExpand(); initialStepID = nil; showing = true } label: {
                    HStack(spacing: 9) {
                        if current != nil && running && confirmed && showsSpinner && !reduceMotion { ProgressView().controlSize(.mini).accessibilityHidden(true) }
                        else if current != nil || !confirmed { Image(systemName: "clock").font(.system(size: 16)).accessibilityHidden(true) }
                        Text(title).font(.subheadline).lineLimit(2).multilineTextAlignment(.leading)
                        Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).accessibilityHidden(true)
                        Spacer(minLength: 0)
                    }.foregroundStyle(Palette.secondary).frame(minHeight: 44).contentShape(Rectangle())
                }.buttonStyle(.plain).accessibilityIdentifier(identifier)
                    .accessibilityLabel(L10n.tr("\(title)，查看工具过程"))
            }
        }.sheet(isPresented: $showing) {
            ActivityProcessSheet(steps: steps, running: running, confirmed: confirmed, storage: storage, initialStepID: initialStepID)
                .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
                .presentationCornerRadius(32)
        }
    }
}

struct ActivityProcessSheet: View {
    let steps: [ActivityStep]
    let running: Bool
    let confirmed: Bool
    let storage: LocalStorage?
    @State private var selectedID: String?
    @State private var preview: Attachment?
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private var selected: ActivityStep? { steps.first { $0.id == selectedID } }
    init(steps: [ActivityStep], running: Bool, confirmed: Bool, storage: LocalStorage?, initialStepID: String? = nil) {
        self.steps = steps; self.running = running; self.confirmed = confirmed; self.storage = storage
        _selectedID = State(initialValue: initialStepID)
    }
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    if let selected { detail(selected) }
                    else {
                        VStack(spacing: 0) {
                            ForEach(Array(steps.enumerated()), id: \.element.id) { index, step in
                                timelineRow(step, last: index == steps.count - 1 && !running)
                            }
                            if running && !steps.contains(where: \.isRunning) {
                                Label(confirmed ? L10n.tr("正在整理结果…") : L10n.tr("等待确认任务状态"), systemImage: "ellipsis")
                                    .font(.subheadline).foregroundStyle(Palette.secondary).frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                            }
                        }
                    }
                }.padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 28)
            }.background(Palette.canvas)
                .navigationTitle(selected?.title ?? L10n.tr("过程概览")).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Button { if selectedID == nil { dismiss() } else { selectedID = nil } } label: {
                            Image(systemName: selectedID == nil ? "xmark" : "chevron.left").font(.system(size: 19, weight: .regular)).frame(width: 28, height: 28)
                        }.accessibilityLabel(selectedID == nil ? L10n.tr("关闭过程") : L10n.tr("返回过程概览"))
                            .accessibilityIdentifier(selectedID == nil ? "activity-close" : "activity-back")
                    }
                }
                .sheet(item: $preview) { file in
                    if let storage { AttachmentPreview(attachments: [file], storage: storage, index: 0) }
                }
        }.tint(Palette.ink)
            .accessibilityAction(.escape) { if selectedID != nil { selectedID = nil } else { dismiss() } }
    }
    private func timelineRow(_ step: ActivityStep, last: Bool) -> some View {
        Button { selectedID = step.id } label: {
            HStack(alignment: .top, spacing: 16) {
                VStack(spacing: 9) {
                    if step.isRunning && confirmed && !reduceMotion { ProgressView().controlSize(.small).frame(width: 22, height: 24) }
                    else { Image(systemName: step.reasoning ? "circle.fill" : step.failed ? "exclamationmark.circle" : step.symbol).font(.system(size: step.reasoning ? 6 : 16, weight: .regular)).frame(width: 22, height: 24).foregroundStyle(step.failed ? Color.red : Palette.secondary) }
                    if !last { Rectangle().fill(Palette.secondary.opacity(0.18)).frame(width: 1).frame(maxHeight: .infinity) }
                }.accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 5) {
                    Text(step.timelineTitle).font(.callout).foregroundStyle(Palette.ink).lineLimit(step.reasoning ? 3 : nil).fixedSize(horizontal: false, vertical: true)
                    if ["unknown", "stopped", "cancelled", "failed", "error"].contains(step.state) {
                        Text(!confirmed && step.isRunning ? L10n.tr("状态待确认") : step.status).font(.caption).foregroundStyle(step.failed ? .red : Palette.secondary)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.bottom, 18)
                if !step.reasoning { Image(systemName: "chevron.right").font(.system(size: 11, weight: .regular)).foregroundStyle(Palette.secondary).padding(.top, 5) }
            }.frame(minHeight: 52).padding(.horizontal, 8).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier("activity-step-\(step.id)")
            .accessibilityLabel(L10n.tr("\(step.title)，\(step.reasoning ? step.timelineTitle + "，" : "")\(!confirmed && step.isRunning ? L10n.tr("状态待确认") : step.status)，查看详情"))
    }
    @ViewBuilder private func detail(_ step: ActivityStep) -> some View {
        if step.reasoning {
            status(step)
            Text(step.output).font(.body).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("activity-reasoning")
        } else {
            if !step.input.isEmpty {
                if step.attachments.contains(where: \.isImage) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(L10n.tr("输入摘要")).font(.subheadline).foregroundStyle(Palette.secondary).padding(.horizontal, 14)
                        Text(step.title).font(.callout).frame(maxWidth: .infinity, alignment: .leading).padding(14)
                            .background(.background, in: RoundedRectangle(cornerRadius: 20))
                            .overlay { RoundedRectangle(cornerRadius: 20).stroke(Palette.secondary.opacity(0.18), lineWidth: 0.5) }
                    }
                } else { ActivityCodeBlock(title: L10n.tr("输入"), language: step.language, text: step.input) }
            }
            if let raw = step.rawInput { DisclosureGroup(L10n.tr("完整参数")) { ActivityCodeBlock(title: nil, language: "json", text: raw, copyLabel: L10n.tr("复制完整参数")) }.font(.footnote) }
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text(L10n.tr("输出")).font(.subheadline).foregroundStyle(Palette.secondary)
                    Spacer()
                    status(step)
                }.padding(.horizontal, 14)
                if !step.output.isEmpty && !step.attachments.contains(where: \.isImage) { ActivityCodeBlock(title: nil, language: "plaintext", text: step.output) }
                ForEach(step.sources) { source in
                    if let url = source.safeURL {
                        Link(destination: url) { VStack(alignment: .leading, spacing: 6) {
                            Text(source.displayTitle).font(.subheadline)
                            Text(url.host ?? "").font(.caption).foregroundStyle(Palette.secondary)
                            if !source.content.isEmpty { Text(source.content).font(.footnote).foregroundStyle(Palette.secondary).lineLimit(4) }
                        }.frame(maxWidth: .infinity, alignment: .leading).padding(14).background(.background, in: RoundedRectangle(cornerRadius: 18)) }
                    }
                }
                if let storage {
                    ForEach(step.attachments) { file in
                        if file.isImage {
                            Button { preview = file } label: { ActivityImage(url: storage.url(for: file)).padding(12).background(.background, in: RoundedRectangle(cornerRadius: 20)) }
                                .buttonStyle(.plain).accessibilityLabel(L10n.tr("预览 \(file.name)")).accessibilityIdentifier("activity-image")
                        } else { DeliverableCard(attachment: file) { preview = file } }
                    }
                }
                if !step.input.isEmpty && step.attachments.contains(where: \.isImage) {
                    DisclosureGroup(L10n.tr("原始输入代码")) { ActivityCodeBlock(title: nil, language: step.language, text: step.input, copyLabel: L10n.tr("复制输入代码")) }.font(.footnote).padding(.horizontal, 14)
                }
                if !step.output.isEmpty && step.attachments.contains(where: \.isImage) {
                    DisclosureGroup(L10n.tr("执行输出")) { ActivityCodeBlock(title: nil, language: "plaintext", text: step.output) }.font(.footnote).padding(.horizontal, 14)
                }
                if step.output.isEmpty && step.attachments.isEmpty && step.sources.isEmpty {
                    Text(step.isRunning ? L10n.tr("等待执行结果…") : L10n.tr("没有文字输出")).font(.subheadline).foregroundStyle(Palette.secondary)
                }
            }
        }
    }
    @ViewBuilder private func status(_ step: ActivityStep) -> some View {
        if step.failed || step.isRunning || ["unknown", "stopped", "cancelled"].contains(step.state) {
            Label(!confirmed && step.isRunning ? L10n.tr("状态待确认") : step.status, systemImage: step.statusSymbol)
                .font(.caption).foregroundStyle(step.failed ? .red : Palette.secondary)
        }
    }
}

private struct ActivityCodeBlock: View {
    var title: String?
    var language: String
    var text: String
    var copyLabel: String? = nil
    private var copyActionTitle: String { copyLabel ?? L10n.tr("复制\(title ?? L10n.tr("输出"))") }
    @State private var full = false
    private var truncated: Bool { text.count > 4000 && !full }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let title { Text(title).font(.subheadline).foregroundStyle(Palette.secondary).padding(.horizontal, 14) }
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text(language).font(.system(.caption, design: .monospaced)).foregroundStyle(Palette.secondary)
                    Spacer()
                }.frame(minHeight: 38).padding(.horizontal, 14)
                Divider()
                HighlightedCode(code: truncated ? String(text.prefix(4000)) : text, language: language).frame(maxWidth: .infinity, alignment: .leading).padding(16)
                if truncated { Button(L10n.tr("显示完整内容")) { full = true }.font(.footnote).padding(.horizontal, 16).padding(.bottom, 16) }
            }.background(.background, in: RoundedRectangle(cornerRadius: 20))
                .overlay { RoundedRectangle(cornerRadius: 20).stroke(Palette.secondary.opacity(0.18), lineWidth: 0.5) }
                .contextMenu { Button(copyActionTitle, systemImage: "doc.on.doc") { UIPasteboard.general.string = text } }
                .accessibilityAction(named: copyActionTitle) { UIPasteboard.general.string = text }
        }
    }
}

private struct ActivityImage: View {
    let url: URL
    @State private var image: UIImage?
    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().scaledToFit().clipShape(RoundedRectangle(cornerRadius: 12)) }
            else { Label(L10n.tr("正在加载图片"), systemImage: "photo").frame(minHeight: 100) }
        }.frame(maxWidth: .infinity).task(id: url) {
            let loaded = await Task.detached(priority: .utility) {
                guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
                      let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 1600, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return UIImage?.none }
                return UIImage(cgImage: cg)
            }.value
            if !Task.isCancelled { image = loaded }
        }
    }
}

struct DeliverableCard: View {
    let attachment: Attachment
    let open: () -> Void
    private var kind: FileKind { FileKind(filename: attachment.name, type: attachment.type) }
    var body: some View {
        Button(action: open) {
            HStack(spacing: 16) {
                Image(systemName: kind.symbol).font(.system(size: 20, weight: .regular)).foregroundStyle(kind.tint)
                    .frame(width: 48, height: 48).background(kind.tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))
                VStack(alignment: .leading, spacing: 5) {
                    Text(FileKind.readableTitle(attachment.name)).font(.callout).lineLimit(2).multilineTextAlignment(.leading)
                    Text("\(kind.label) · \((attachment.name as NSString).pathExtension.uppercased()) · \(ByteCountFormatter.string(fromByteCount: Int64(attachment.size), countStyle: .file))").font(.caption).foregroundStyle(Palette.secondary)
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.padding(10).background(Palette.canvas, in: RoundedRectangle(cornerRadius: 20))
                .overlay { RoundedRectangle(cornerRadius: 20).stroke(Palette.secondary.opacity(0.18), lineWidth: 0.5) }
        }.buttonStyle(.plain).accessibilityLabel(L10n.tr("预览 \(attachment.name)")).accessibilityIdentifier("deliverable-card")
    }
}

extension ActivityStep {
    static func remote(_ messages: [RemoteMessage], running: Bool, confirmed: Bool, activeID: String?) -> [ActivityStep] {
        var steps: [ActivityStep] = []
        var callIndices: [String: Int] = [:]
        for message in messages {
            let isOutput = message.kind?.contains("output") == true || (message.role == "tool" && message.kind?.contains("call") != true)
            let args = message.arguments?.data(using: .utf8).flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any] ?? [:]
            let command = args["command"] as? String ?? args["cmd"] as? String
            var state = message.state ?? message.status ?? "unknown"
            let output = message.output ?? (isOutput ? message.text : "")
            if message.isStreaming && message.state == nil {
                state = confirmed && running && message.id == activeID ? "running" : "unknown"
                if message.kind == "reasoning" && confirmed && message.id != activeID { state = "complete" }
            }
            if message.status == "failed" || message.state == "error" { state = "failed" }
            if let bytes = output.data(using: .utf8), let value = (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any], let code = value["exit_code"] as? Int, code != 0 { state = "failed" }
            if isOutput, let call = message.callID, !call.isEmpty, let index = callIndices[call] {
                steps[index].output = output
                steps[index].state = state
                continue
            }
            let isReasoning = message.kind == "reasoning"
            let description = (args["description"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
            let path = args["file_path"] as? String ?? args["path"] as? String
            let filename = path.map { ($0 as NSString).lastPathComponent }
            var title = isReasoning ? (state == "running" ? L10n.tr("正在思考") : L10n.tr("思考记录")) : message.processTitle
            if !isReasoning, let description, !description.isEmpty { title = description }
            else if !isReasoning, let filename, !filename.isEmpty {
                if filename == "SKILL.md" { title = L10n.tr("读取技能指南") }
                else { title += " · \(filename)" }
            }
            let symbol = isReasoning ? "sparkle" : command != nil ? "terminal" : (message.name?.contains("read") == true ? "book" : "doc.text")
            steps.append(ActivityStep(id: message.id, title: title, symbol: symbol, state: state,
                                      input: command ?? message.arguments ?? "", language: command == nil ? "json" : "bash",
                                      output: isReasoning ? message.text : output.isEmpty && message.arguments == nil ? message.text : output, reasoning: isReasoning, rawInput: command != nil ? message.arguments : nil))
            if !isOutput, let call = message.callID, !call.isEmpty { callIndices[call] = steps.count - 1 }
        }
        return steps
    }
}
