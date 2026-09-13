import XCTest
@testable import PotatoMobile

@MainActor
private final class CaptureStub: VoiceCapture {
    var events: [(VoiceCaptureEvent) -> Void] = []
    var finishes = 0
    func start(settings: ConnectionSettings, event: @escaping (VoiceCaptureEvent) -> Void) async { events.append(event); event(.ready) }
    func finish() { finishes += 1 }
    func cancel() {}
    func emit(_ value: VoiceCaptureEvent) { events.last?(value) }
}
@MainActor
final class VoiceComposerTests: XCTestCase {
    private func setup(_ text: String = "") -> (WorkspaceStore, CaptureStub, VoiceComposer) {
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)))
        store.newChat(); store.update { $0.input = text }
        let capture = CaptureStub(); return (store, capture, VoiceComposer(capture: capture))
    }
    private func started(_ voice: VoiceComposer, _ store: WorkspaceStore, _ range: NSRange? = nil) async {
        voice.start(store: store, selection: range)
        for _ in 0..<10 { if voice.phase == .recording { return }; await Task.yield() }
        XCTAssertEqual(voice.phase, .recording)
    }
    func testInsertionUsesUTF16RangeAndPartialReplacesOnlyDictation() async {
        let (store, capture, voice) = setup("前🙂后")
        await started(voice, store, NSRange(location: 1, length: 2))
        capture.emit(.text("说", final: false)); XCTAssertEqual(store.selected.input, "前说后")
        capture.emit(.text("说完整", final: false)); XCTAssertEqual(store.selected.input, "前说完整后")
        voice.cancel(); XCTAssertEqual(store.selected.input, "前🙂后")
        XCTAssertEqual(DictationInsertion(text: "🙂", selection: NSRange(location: 99, length: 2)).replacing(with: "好"), "🙂好")
    }
    func testSendWaitsForFinalIncludesTailAndSendsExactlyOnceWithAttachments() async {
        let (store, capture, voice) = setup("原稿：")
        let file = Attachment(name: "input.txt", filename: "fixture", type: "text/plain", size: 1)
        store.addAttachment(file)
        await started(voice, store)
        capture.emit(.text("今天", final: false)); voice.finish(send: true); voice.finish(send: true)
        XCTAssertEqual(capture.finishes, 1); XCTAssertTrue(store.selected.messages.isEmpty)
        capture.emit(.text("今天完成计划。", final: true)); capture.emit(.text("不该再次发送", final: true))
        XCTAssertEqual(store.selected.messages.filter { $0.role == "user" }.count, 1)
        XCTAssertEqual(store.selected.messages.first?.text, "原稿：今天完成计划。")
        XCTAssertEqual(store.selected.messages.first?.attachments, [file]); XCTAssertEqual(voice.sentRevision, 1); store.stop()
    }
    func testEditPreservesTextAndLateResultsCannotOverwriteManualEdit() async {
        let (store, capture, voice) = setup()
        await started(voice, store); capture.emit(.text("计划", final: false)); voice.finish(send: false)
        capture.emit(.text("项目计划", final: true)); XCTAssertEqual(voice.editRevision, 1)
        store.update { $0.input = "手工修改" }; capture.emit(.text("旧结果", final: true))
        XCTAssertEqual(store.selected.input, "手工修改"); XCTAssertTrue(store.selected.messages.isEmpty)
    }
    func testCancelKeepsOriginalDraftAndAttachmentsAndIgnoresOldSession() async {
        let (store, capture, voice) = setup("保留")
        let file = Attachment(name: "a.txt", filename: "fixture", type: "text/plain", size: 1); store.addAttachment(file)
        await started(voice, store); capture.emit(.text("新说的", final: false)); let old = capture.events.last!
        voice.finish(send: true); voice.cancel(); XCTAssertEqual(store.selected.input, "保留"); XCTAssertEqual(store.selected.pendingAttachments, [file])
        await started(voice, store); old(.text("旧会话结束", final: true))
        XCTAssertEqual(voice.phase, .recording); XCTAssertEqual(store.selected.input, "保留"); voice.cancel()
    }
    func testFailureEmptyFinalAndInterruptionKeepPartialWithoutSending() async {
        for event: VoiceCaptureEvent in [.failed("连接断开"), .text("", final: true), .interrupted] {
            let (store, capture, voice) = setup()
            await started(voice, store); capture.emit(.text("已识别", final: false)); voice.finish(send: true); capture.emit(event)
            XCTAssertEqual(store.selected.input, "已识别"); XCTAssertTrue(store.selected.messages.isEmpty); XCTAssertFalse(voice.active); XCTAssertNotNil(voice.notice)
        }
    }
    func testSwitchConversationAndInactiveNeverSendPendingVoice() async {
        let (store, capture, voice) = setup()
        await started(voice, store); let original = store.selectedID
        capture.emit(.text("旧对话草稿", final: false)); voice.finish(send: true)
        store.newChat(); capture.emit(.text("不应写入新对话", final: true))
        XCTAssertTrue(store.selected.input.isEmpty); XCTAssertEqual(store.conversations.first { $0.id == original }?.input, "旧对话草稿")
        await started(voice, store); capture.emit(.text("保留", final: false)); voice.finish(send: true); voice.foreground = false
        capture.emit(.text("最终保留", final: true)); XCTAssertTrue(store.selected.messages.isEmpty); XCTAssertEqual(store.selected.input, "最终保留")
    }
    func testLimitFinishesAsDraftAndEmptySpeechCannotSendOriginalText() async {
        let (store, capture, voice) = setup("原稿")
        await started(voice, store); voice.finish(send: true); XCTAssertEqual(capture.finishes, 0)
        capture.emit(.text("录音", final: false)); capture.emit(.limit); XCTAssertEqual(voice.phase, .finishingEdit)
        capture.emit(.text("录音结束", final: true)); XCTAssertEqual(store.selected.input, "原稿录音结束"); XCTAssertTrue(store.selected.messages.isEmpty)
    }
    func testMeterReflectsPCMAmplitudeAndSilence() {
        func pcm(_ sample: Int16) -> Data {
            var data = Data()
            for _ in 0..<160 { var sample = sample.littleEndian; withUnsafeBytes(of: &sample) { data.append(contentsOf: $0) } }
            return data
        }
        XCTAssertEqual(VoiceRecorder.level(Data()), 0)
        XCTAssertEqual(VoiceRecorder.level(pcm(0)), 0)
        let quiet = VoiceRecorder.level(pcm(400)), loud = VoiceRecorder.level(pcm(3200))
        XCTAssertGreaterThan(quiet, 0); XCTAssertGreaterThan(loud, quiet * 4)
        XCTAssertEqual(VoiceRecorder.level(pcm(-3200)), loud, accuracy: 0.001)
        XCTAssertLessThanOrEqual(VoiceRecorder.level(pcm(Int16.min)), 1)
    }
    func testWaveformFallsToSilenceAndIgnoresLateCaptureEvents() async {
        let (store, capture, voice) = setup(); await started(voice, store)
        capture.emit(.level(0.8)); XCTAssertEqual(voice.levels.last, 0.8)
        for _ in 0..<21 { capture.emit(.level(0)) }
        XCTAssertTrue(voice.levels.allSatisfy { $0 == 0 })
        voice.cancel(); capture.emit(.level(1))
        XCTAssertTrue(voice.levels.allSatisfy { $0 == 0 })
    }

}
