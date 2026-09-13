import SwiftUI

struct DictationInsertion {
    let original: String
    let range: NSRange
    init(text: String, selection: NSRange?) {
        original = text
        if let selection, Range(selection, in: text) != nil { range = selection }
        else { range = NSRange(location: (text as NSString).length, length: 0) }
    }
    func replacing(with text: String) -> String { (original as NSString).replacingCharacters(in: range, with: text) }
}

@MainActor
final class VoiceComposer: ObservableObject {
    enum Phase: Equatable { case idle, connecting, recording, finishingSend, finishingEdit }
    @Published private(set) var phase: Phase = .idle
    @Published private(set) var elapsed = 0
    @Published private(set) var levels = Array(repeating: Float(0), count: 21)
    @Published private(set) var notice: String?
    @Published private(set) var editRevision = 0
    @Published private(set) var sentRevision = 0
    @Published private(set) var finalSelection: NSRange?
    @Published private(set) var transcript = ""
    private let capture: VoiceCapture
    private weak var store: WorkspaceStore?
    private var conversationID: UUID?
    private var identity: UUID?
    private var insertion: DictationInsertion?
    private var startTask: Task<Void, Never>?
    private var clock: Task<Void, Never>?
    var foreground = true
    var active: Bool { phase != .idle }
    var finishing: Bool { phase == .finishingSend || phase == .finishingEdit }
    var canSubmit: Bool { phase == .recording && !transcript.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    init(capture: VoiceCapture? = nil) { self.capture = capture ?? VoiceRecorder() }

    func start(store: WorkspaceStore, selection: NSRange?) {
        guard !active, store.generatingID == nil else { return }
        self.store = store; conversationID = store.selectedID; insertion = DictationInsertion(text: store.selected.input, selection: selection)
        let id = UUID(); identity = id; phase = .connecting; notice = nil; transcript = ""; elapsed = 0; levels = Array(repeating: 0, count: 21); finalSelection = nil
        let settings = store.settings
        startTask = Task { [weak self] in
            guard let self else { return }
            await self.capture.start(settings: settings) { [weak self] event in self?.receive(event, id: id) }
        }
    }
    private func receive(_ event: VoiceCaptureEvent, id: UUID) {
        guard identity == id, let store, let conversationID else { return }
        guard store.selectedID == conversationID else { interrupt(); return }
        switch event {
        case .ready:
            guard phase == .connecting else { return }; phase = .recording
            if store.settings.haptics { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
            clock = Task { [weak self] in
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(1)); guard !Task.isCancelled, let self, self.identity == id else { return }; self.elapsed += 1
                }
            }
        case .level(let value):
            guard phase == .recording else { return }; levels.removeFirst(); levels.append(min(1, max(0, value)))
        case .text(let text, let final):
            guard phase != .connecting else { return }
            // Empty final must not erase a useful partial; it still must never be auto-sent.
            if !text.isEmpty { transcript = text; writeDraft(text) }
            if final {
                let shouldSend = foreground && phase == .finishingSend && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                let shouldEdit = phase == .finishingEdit
                if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { notice = transcript.isEmpty ? "没有听清，请再说一次。" : "未收到完整转写，文字已保留，可修改后发送。" }
                release()
                if shouldSend, store.selectedID == conversationID, store.generatingID == nil {
                    store.send(); sentRevision += 1
                } else if shouldEdit { editRevision += 1 }
            }
        case .failed(let message):
            notice = message + " 未发送，已有文字已保留。"; release()
        case .limit:
            guard phase == .recording else { return }; notice = "已录满60秒，文字会保留在输入框。"; finish(send: false)
        case .interrupted: interrupt()
        }
    }
    private func writeDraft(_ text: String) {
        guard let store, let conversationID, let insertion else { return }
        store.update(conversationID) { $0.input = insertion.replacing(with: text) }
        finalSelection = NSRange(location: insertion.range.location + (text as NSString).length, length: 0)
    }
    func finish(send: Bool) {
        guard phase == .recording, !send || canSubmit else { return }
        phase = send ? .finishingSend : .finishingEdit; clock?.cancel(); capture.finish()
    }
    func cancel() {
        guard active else { return }
        if let store, let conversationID, let insertion { store.update(conversationID) { $0.input = insertion.original } }
        finalSelection = insertion?.range; notice = nil; release()
    }
    func interrupt() {
        guard active else { return }
        notice = "录音已中断，已有文字已保留，未发送。"; release()
    }
    func dismissNotice() { notice = nil }
    private func release() {
        identity = nil; capture.cancel(); startTask?.cancel(); startTask = nil; clock?.cancel(); clock = nil
        phase = .idle; store?.persist(); insertion = nil; conversationID = nil
    }
}

struct VoiceComposerPanel: View {
    @ObservedObject var voice: VoiceComposer
    let text: String
    var maximumHeight: CGFloat = 220
    var expand: () -> Void = {}
    @State private var following = true
    @State private var followRevision = 0
    @State private var overflowing = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        VStack(spacing: 2) {
            if overflowing {
                HStack {
                    if !following {
                        Button("回到最新", systemImage: "arrow.down") { following = true; followRevision += 1 }
                            .accessibilityIdentifier("follow-voice")
                    }
                    Spacer(minLength: 0)
                    Button("展开", systemImage: "arrow.up.left.and.arrow.down.right", action: expand)
                        .disabled(voice.phase != .recording).accessibilityIdentifier("expand-voice-input")
                }.font(.caption).frame(minHeight: 44).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            }
            ZStack(alignment: .topLeading) {
                if text.isEmpty {
                    Text(voice.phase == .connecting ? "正在启动麦克风…" : "开始说话吧…").font(.body).foregroundStyle(Palette.secondary).padding(.top, 2).accessibilityHidden(true)
                }
                ComposerTextInput(text: .constant(text), selection: .constant(nil), focused: .constant(false), placeholder: "语音转写", maximumHeight: maximumHeight, readOnly: true, followsInsertion: following, insertionRange: voice.finalSelection, followRevision: followRevision, identifier: "voice-transcript", onTap: { voice.finish(send: false) }, onOverflow: { overflowing = $0 }, onManualScroll: { following = false })
            }.padding(.horizontal, 12)
            HStack(spacing: 0) {
                Button { voice.cancel() } label: {
                    Image(systemName: "xmark").font(.system(size: 16)).frame(width: 33, height: 33)
                        .background(Color(uiColor: .systemGray6), in: Circle()).frame(width: 44, height: 44)
                }.accessibilityLabel(voice.finishing ? "取消本次语音发送" : "取消本次语音").accessibilityIdentifier("cancel-voice")
                HStack(spacing: 4) {
                    if voice.phase == .recording {
                        Label("左滑取消", systemImage: "chevron.left").font(.system(size: 10)).foregroundStyle(.secondary)
                            .fixedSize().padding(.horizontal, 5).padding(.vertical, 3).background(Color(uiColor: .systemGray6), in: Capsule())
                    } else {
                        Text(voice.phase == .connecting ? "准备中" : "正在收尾…").font(.caption2).foregroundStyle(.secondary)
                    }
                    GeometryReader { geometry in
                        HStack(spacing: 2.5) {
                            ForEach(0..<max(1, Int(geometry.size.width / 4.5)), id: \.self) { index in
                                Capsule().fill(index < 21 ? Color.primary : Color.primary.opacity(0.12))
                                    .frame(width: 2, height: reduceMotion || index >= 21 ? 3 : CGFloat(3 + voice.levels[min(index, 20)] * 11))
                            }
                        }.frame(height: 18)
                    }.frame(height: 18).clipped().accessibilityHidden(true)
                }.frame(maxWidth: .infinity, minHeight: 44).contentShape(Rectangle())
                    .accessibilityElement(children: .ignore).accessibilityLabel(voice.phase == .connecting ? "正在启动麦克风" : voice.finishing ? "正在收尾" : "正在听，\(voice.elapsed)秒")
                    .accessibilityIdentifier("voice-status")
                    .gesture(DragGesture(minimumDistance: 24).onEnded { value in
                        if value.translation.height < -60 && abs(value.translation.height) > abs(value.translation.width) { voice.finish(send: true) }
                        else if value.translation.width < -70 && abs(value.translation.width) > abs(value.translation.height) { voice.cancel() }
                    })
                Button { voice.finish(send: true) } label: {
                    Group {
                        if voice.finishing { ProgressView().tint(.white) }
                        else { Image(systemName: "checkmark").font(.system(size: 21, weight: .medium)) }
                    }.foregroundStyle(.white).frame(width: 33, height: 33)
                        .background(voice.canSubmit || voice.finishing ? Color.black : Color.secondary.opacity(0.4), in: Circle()).frame(width: 44, height: 44)
                }.disabled(!voice.canSubmit).accessibilityLabel("结束录音并发送").accessibilityIdentifier("send-voice")
            }
        }
    }
}
