import SwiftUI
import ImageIO
import QuickLookThumbnailing

/// Presentation only: raw tool input/output stays available behind each step.
struct ActivityStep: Identifiable {
    var id: String
    var title: String
    var symbol: String
    var state: String
    var kind = ActivityKind.other
    /// The file a step touched, so summaries count files rather than calls.
    var target: String? = nil
    var input = ""
    var language = "plaintext"
    var output = ""
    var attachments: [Attachment] = []
    var sources: [WebSource] = []
    var reasoning = false
    var rawInput: String? = nil
    /// Client-observed thinking time; nil when the source does not report one.
    var duration: TimeInterval? = nil
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
        guard activityOrder?.contains(id) == false else { return }
        activityOrder?.append(id)
        // Where the text stood when the step began, so the reply reads in order.
        if activityAnchors == nil { activityAnchors = [:] }
        activityAnchors?[id] = text.utf16.count
    }
    var activitySteps: [ActivityStep] {
        var steps: [ActivityStep] = []
        if let trace = displayReasoning, !trace.text.isEmpty {
            steps.append(ActivityStep(id: "reasoning", title: trace.state == .streaming ? L10n.tr("正在思考") : L10n.tr("已思考 \(max(1, Int(trace.elapsed.rounded()))) 秒"), symbol: "sparkle", state: trace.state.rawValue, output: trace.text, reasoning: true, duration: trace.elapsed))
        }
        steps += displaySearches.map { run in
            ActivityStep(id: "search:\(run.id)", title: L10n.tr("搜索：\(run.query)"), symbol: "magnifyingglass", state: run.state, kind: .web, input: run.query, output: run.results.isEmpty && run.state == "complete" ? L10n.tr("未找到可用的网页来源。") : "", sources: run.results)
        }
        // Recall results are shown once, as the sources row under the answer.
        steps += displayCodeRuns.map { run in
            let files = displayAttachments.filter { run.attachmentIDs?.contains($0.id) == true }
            let names = run.result?.artifacts.map(\.name) ?? []
            let description = run.title?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = description.flatMap { $0.isEmpty ? nil : $0 } ?? names.first.map { L10n.tr("生成 \($0)") } ?? L10n.tr("执行代码")
            let result = run.result
            // Name the step by what it made; a shell prompt reads as a developer tool.
            let symbol = files.first.map { FileKind(filename: $0.name, type: $0.type).symbol } ?? "gearshape"
            return ActivityStep(id: "code:\(run.id)", title: title, symbol: symbol, state: run.state, kind: .code, input: run.code, language: "python", output: [result?.stdout, result?.text, result?.stderr, result?.error, run.message].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "\n\n"), attachments: files)
        }
        if let result = displayExecution {
            steps.append(ActivityStep(id: "execution", title: L10n.tr("运行代码"), symbol: "gearshape", state: result.status, kind: .code, output: [result.stdout, result.text, result.stderr, result.error ?? ""].filter { !$0.isEmpty }.joined(separator: "\n\n")))
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

struct ActivitySummaryView: View {
    let steps: [ActivityStep]
    let running: Bool
    var confirmed = true
    var endState = "complete"
    var storage: LocalStorage? = nil
    var replyStarted = false
    var identifier = "activity-summary"
    var onExpand: () -> Void = {}
    @State private var showing = false
    private var thought: ActivityStep? { steps.last(where: \.reasoning) }
    private var tools: [ActivityStep] { steps.filter { !$0.reasoning } }
    private var current: ActivityStep? { tools.last(where: \.isRunning) }
    /// In progress until the answer itself starts; then the row settles into a summary.
    private var active: Bool { running && confirmed && (current != nil || thought?.isRunning == true || !replyStarted) }
    private var title: String {
        if active { return current?.title ?? L10n.tr("正在思考") }
        var parts: [String] = []
        if let thought, !tools.isEmpty, !thought.isRunning, let seconds = thought.duration.map({ max(1, Int($0.rounded())) }) {
            parts.append(L10n.tr("已思考 \(seconds) 秒"))
        }
        parts.append(steps.actionSummary)
        if ["stopped", "cancelled"].contains(endState) { parts.append(L10n.tr("已停止")) }
        else if endState == "failed" { parts.append(L10n.tr("已中断")) }
        return parts.joined(separator: " · ")
    }
    var body: some View {
        if !steps.isEmpty {
            Button { onExpand(); showing = true } label: {
                HStack(spacing: 6) {
                    Text(title).font(.subheadline).lineLimit(1).shimmering(active)
                    Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).accessibilityHidden(true)
                    Spacer(minLength: 0)
                }.foregroundStyle(Palette.secondary).frame(minHeight: 44).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier(identifier)
                .accessibilityLabel(L10n.tr("\(title)，查看过程"))
                .sheet(isPresented: $showing) {
                    // A lone thought opens straight into its text.
                    ActivityProcessSheet(steps: steps, running: running, confirmed: confirmed, storage: storage, initialStepID: tools.isEmpty ? thought?.id : nil)
                        .presentationDetents([.medium, .large]).presentationDragIndicator(.visible)
                        .presentationCornerRadius(32)
                }
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
                                Text(confirmed ? L10n.tr("正在整理结果") : L10n.tr("等待确认任务状态"))
                                    .font(.subheadline).foregroundStyle(Palette.secondary).shimmering(confirmed)
                                    .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading).padding(.leading, 46)
                            }
                        }
                    }
                }.padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 28)
            }.background(Palette.canvas)
                .navigationTitle(selected?.title ?? L10n.tr("过程")).navigationBarTitleDisplayMode(.inline)
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
                    Image(systemName: step.reasoning ? "circle.fill" : step.failed ? "exclamationmark.circle" : step.symbol).font(.system(size: step.reasoning ? 6 : 16, weight: .regular)).frame(width: 22, height: 24).foregroundStyle(step.failed ? Color.red : Palette.secondary)
                    if !last { Rectangle().fill(Palette.secondary.opacity(0.18)).frame(width: 1).frame(maxHeight: .infinity) }
                }.accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 5) {
                    Text(step.timelineTitle).font(.callout).foregroundStyle(Palette.ink).lineLimit(step.reasoning ? 3 : nil).fixedSize(horizontal: false, vertical: true)
                        .shimmering(step.isRunning && confirmed)
                    if ["unknown", "stopped", "cancelled", "failed", "error"].contains(step.state) {
                        Text(!confirmed && step.isRunning ? L10n.tr("状态待确认") : step.status).font(.caption).foregroundStyle(step.failed ? .red : Palette.secondary)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.bottom, 18)
                Image(systemName: "chevron.right").font(.system(size: 11, weight: .regular)).foregroundStyle(Palette.secondary).padding(.top, 5)
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
                        } else { DeliverableCard(attachment: file, storage: storage) { preview = file } }
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
    @State private var copied = false
    private var languageName: String {
        switch language.lowercased() {
        case "python": "Python"
        case "json": "JSON"
        case "bash", "sh", "shell": "Shell"
        case "plaintext", "text": ""
        default: language
        }
    }
    private var truncated: Bool { text.count > 4000 && !full }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let title { Text(title).font(.subheadline).foregroundStyle(Palette.secondary).padding(.horizontal, 14) }
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text(languageName).font(.caption).foregroundStyle(Palette.secondary)
                    Spacer()
                    Button { UIPasteboard.general.string = text; copied = true } label: {
                        Image(systemName: copied ? "checkmark" : "doc.on.doc").font(.system(size: 13)).frame(width: 44, height: 38).contentShape(Rectangle())
                    }.buttonStyle(.plain).foregroundStyle(Palette.secondary).accessibilityLabel(copyActionTitle)
                        .task(id: copied) { if copied { try? await Task.sleep(for: .seconds(1.5)); copied = false } }
                }.frame(minHeight: 38).padding(.leading, 14).padding(.trailing, 4)
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
    let storage: LocalStorage
    /// A rendered page of this file, when the reply produced one.
    var cover: Attachment? = nil
    let open: () -> Void
    var body: some View {
        Button(action: open) {
            HStack(spacing: 14) {
                FilePageThumbnail(attachment: attachment, cover: cover, storage: storage).frame(width: cover == nil ? 44 : 58, height: 44)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    .overlay { RoundedRectangle(cornerRadius: 10).stroke(Palette.line, lineWidth: 0.5) }
                VStack(alignment: .leading, spacing: 3) {
                    Text(FileKind.readableTitle(attachment.name)).font(.callout).lineLimit(2).multilineTextAlignment(.leading)
                    Text((attachment.name as NSString).pathExtension.uppercased()).font(.caption).foregroundStyle(Palette.secondary)
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.padding(10).background(Palette.canvas, in: RoundedRectangle(cornerRadius: 18))
                .overlay { RoundedRectangle(cornerRadius: 18).stroke(Palette.line, lineWidth: 0.5) }
        }.buttonStyle(.plain).accessibilityLabel(L10n.tr("预览 \(attachment.name)")).accessibilityIdentifier("deliverable-card")
    }
}

/// First page of the file when Quick Look can render it; otherwise a plain family icon.
struct FilePageThumbnail: View {
    let attachment: Attachment
    var cover: Attachment? = nil
    let storage: LocalStorage
    @State private var image: UIImage?
    var body: some View {
        ZStack {
            Palette.surface
            if let image { Image(uiImage: image).resizable().scaledToFill() }
            else { Image(systemName: FileKind(filename: attachment.name, type: attachment.type).symbol).font(.system(size: 18)).foregroundStyle(Palette.secondary) }
        }.task(id: attachment.filename) {
            // Quick Look cannot draw every format (Office files on some systems), so prefer a page the reply rendered.
            let url = storage.url(for: cover ?? attachment)
            guard FileManager.default.fileExists(atPath: url.path) else { return }
            let request = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: 88, height: 88), scale: 3, representationTypes: .thumbnail)
            let thumbnail = try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: request)
            if !Task.isCancelled { image = thumbnail?.uiImage }
        }.accessibilityHidden(true)
    }
}

extension ActivityStep {
    static func remote(_ messages: [RemoteMessage], running: Bool, confirmed: Bool, activeID: String?) -> [ActivityStep] {
        var steps: [ActivityStep] = []
        var callIndices: [String: Int] = [:]
        // Older desktops send no call IDs; there an output belongs to the call just before it.
        var legacyCall: Int?
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
            if isOutput, message.callID?.isEmpty != false, let index = legacyCall {
                steps[index].output = output; steps[index].state = state; legacyCall = nil
                continue
            }
            let isReasoning = message.kind == "reasoning"
            let description = (args["description"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
            let path = args["file_path"] as? String ?? args["path"] as? String
            let filename = path.map { ($0 as NSString).lastPathComponent }
            var title = isReasoning ? (state == "running" ? L10n.tr("正在思考") : L10n.tr("思考记录")) : message.processTitle
            let commandLine = command?.split(whereSeparator: \.isNewline).first.map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
            if !isReasoning, let description, !description.isEmpty { title = description }
            // The command itself says more than "执行命令".
            else if !isReasoning, !commandLine.isEmpty { title = commandLine.count > 80 ? String(commandLine.prefix(80)) + "…" : commandLine }
            else if !isReasoning, let filename, !filename.isEmpty {
                if filename == "SKILL.md" { title = L10n.tr("读取技能指南") }
                else { title += " · \(filename)" }
            }
            let symbol = isReasoning ? "sparkle" : command != nil ? "terminal" : (message.name?.contains("read") == true ? "book" : "doc.text")
            var kind = ActivityKind(tool: message.toolName)
            if kind == .other && command != nil { kind = .command }
            steps.append(ActivityStep(id: message.id, title: title, symbol: symbol, state: state, kind: kind, target: path,
                                      input: command ?? message.arguments ?? "", language: command == nil ? "json" : "bash",
                                      output: isReasoning ? message.text : output.isEmpty && message.arguments == nil ? message.text : output, reasoning: isReasoning, rawInput: command != nil ? message.arguments : nil))
            if !isOutput, let call = message.callID, !call.isEmpty { callIndices[call] = steps.count - 1 }
            legacyCall = !isOutput && !isReasoning && message.callID?.isEmpty != false ? steps.count - 1 : nil
        }
        return steps
    }
}
