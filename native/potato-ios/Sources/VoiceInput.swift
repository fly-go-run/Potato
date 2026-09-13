import SwiftUI
import AVFoundation

@MainActor
protocol VoiceCapture: AnyObject {
    func start(settings: ConnectionSettings, event: @escaping (VoiceCaptureEvent) -> Void) async
    func finish()
    func cancel()
}
enum VoiceCaptureEvent {
    case ready, text(String, final: Bool), level(Float), limit, interrupted, failed(String)
}

@MainActor
final class VoiceRecorder: VoiceCapture {
    private let engine = AVAudioEngine()
    private var connection: SpeechConnection?
    private var converter: VoicePCMConverter?
    private var tapped = false
    private var capturing = false
    private var identity: UUID?
    private var handler: ((VoiceCaptureEvent) -> Void)?
    private var audio: SpeechAudioBuffer?
    private var sender: Task<Void, Never>?
    private var receiver: Task<Void, Never>?
    private var timer: Task<Void, Never>?
    private var finalTimer: Task<Void, Never>?
    private var interruption: NSObjectProtocol?
    #if DEBUG
    // RMS envelope of the synthetic Chinese verification fixture, then silence; never a looping animation.
    private static let previewLevels: [Float] = [0.993, 0.734, 0.423, 1, 0.31, 0.438, 0.554, 0.856, 0.942, 0.355, 0.001, 0.001, 0.315, 1, 0.945, 1, 0.62, 0.828, 1, 0.884, 0.432, 0.827, 0.936, 0.394, 0.915, 0.985, 0.406, 0.931, 0.875, 0.792, 1, 0.236, 0.906, 0.384, 0.949, 0.678, 0.008]
    private var previewText: String {
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("--grok-preview") { return "这是一个测试，你可以看一下，这是我语音转文字" }
        let initial = "先把项目方案整理好，下午再和团队讨论一下。"
        if arguments.contains("--voice-long-preview") {
            return initial + String(repeating: "新增内容正在继续转写，回看前文时应保持位置，点击回到最新后继续跟随。", count: 5)
        }
        return initial
    }
    #endif

    func start(settings: ConnectionSettings, event: @escaping (VoiceCaptureEvent) -> Void) async {
        cancel()
        let id = UUID(); identity = id; handler = event
        do {
            #if DEBUG
            if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--voice-preview") {
                capturing = true; emit(.ready, id)
                sender = Task { [weak self] in
                    let text = self?.previewText ?? ""
                    guard !text.isEmpty else { return }
                    for length in 1...text.count {
                        try? await Task.sleep(for: .milliseconds(70))
                        guard let self, self.identity == id, !Task.isCancelled else { return }
                        self.emit(.text(String(text.prefix(length)), final: false), id)
                        self.emit(.level(length <= Self.previewLevels.count ? Self.previewLevels[length - 1] : 0), id)
                    }
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .milliseconds(70))
                        guard let self, self.identity == id, !Task.isCancelled else { return }
                        self.emit(.level(0), id)
                    }
                }
                return
            }
            #endif
            let request = try SpeechService.request(settings: settings, token: settings.connectionToken)
            #if DEBUG
            if ProcessInfo.processInfo.arguments.contains("--live-voice-fixture") {
                let url = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("PotatoVoiceVerification.pcm")
                let data = try Data(contentsOf: url)
                guard !data.isEmpty, data.count <= 1_920_000, data.count % 2 == 0 else { throw LocalFailure.message("测试音频无效。") }
                try await connect(request, id: id)
                guard identity == id else { return }
                capturing = true; emit(.ready, id); startLimit(id)
                sender = Task { [weak self] in
                    guard let self, let connection = self.connection else { return }
                    do {
                        var offset = 0
                        while self.identity == id && self.capturing {
                            let chunk = offset < data.count ? data.subdata(in: offset..<min(offset + 3200, data.count)) : Data(repeating: 0, count: 3200)
                            offset += 3200; self.emit(.level(Self.level(chunk)), id)
                            try await connection.send(chunk); try await Task.sleep(for: .milliseconds(100))
                        }
                        guard self.identity == id, !Task.isCancelled else { return }
                        try await connection.finish()
                        self.startFinalTimeout(id)
                    } catch { self.fail(error, id: id) }
                }
                return
            }
            #endif
            let allowed = await withCheckedContinuation { continuation in AVAudioApplication.requestRecordPermission { continuation.resume(returning: $0) } }
            guard identity == id, !Task.isCancelled else { return }
            guard allowed else { throw LocalFailure.message("麦克风权限未开启，请在系统设置中允许访问。") }
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.record, mode: .measurement, options: .duckOthers); try session.setActive(true)
            let node = engine.inputNode, format = node.outputFormat(forBus: 0)
            let converter = try VoicePCMConverter(input: format); self.converter = converter
            let audio = SpeechAudioBuffer(); self.audio = audio
            node.installTap(onBus: 0, bufferSize: 2048, format: format) { [weak self] buffer, _ in
                do {
                    let data = try converter.convert(buffer)
                    // Local metering must not wait behind WebSocket uploads.
                    let level = Self.level(data)
                    Task { @MainActor [weak self] in self?.emit(.level(level), id) }
                    try audio.append(data)
                } catch { Task { @MainActor [weak self] in self?.fail(error, id: id) } }
            }
            tapped = true; engine.prepare(); try engine.start(); capturing = true
            interruption = NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: session, queue: .main) { [weak self] note in
                guard (note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) == AVAudioSession.InterruptionType.began.rawValue else { return }
                Task { @MainActor in self?.emit(.interrupted, id) }
            }
            emit(.ready, id); startLimit(id)
            // Ready means the microphone is capturing. Upload waits for the service,
            // while local metering and the bounded audio buffer are already active.
            let connection = SpeechConnection(request: request); self.connection = connection
            sender = Task { [weak self] in
                guard let self, self.identity == id, !Task.isCancelled else { return }
                do {
                    try await audio.upload(to: connection) { self.listen(connection, id: id) }
                    guard self.identity == id, !Task.isCancelled else { return }
                    self.startFinalTimeout(id)
                } catch { self.fail(error, id: id) }
            }
        } catch { fail(error, id: id) }
    }
    private func connect(_ request: URLRequest, id: UUID) async throws {
        let connection = SpeechConnection(request: request); self.connection = connection
        try await connection.connect()
        guard identity == id, !Task.isCancelled else { connection.cancel(); return }
        listen(connection, id: id)
    }
    private func listen(_ connection: SpeechConnection, id: UUID) {
        receiver = Task { [weak self] in
            do {
                while !Task.isCancelled {
                    let event = try await connection.receive()
                    guard let self, self.identity == id, !Task.isCancelled else { return }
                    if let text = event.text { self.emit(.text(text, final: event.type == "final"), id) }
                    if event.type == "final" { if self.identity == id { self.cancel() }; return }
                }
            } catch { self?.fail(error, id: id) }
        }
    }
    private func startLimit(_ id: UUID) {
        timer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            if !Task.isCancelled { self?.emit(.limit, id) }
        }
    }
    private func emit(_ event: VoiceCaptureEvent, _ id: UUID) { guard identity == id else { return }; handler?(event) }
    private func fail(_ error: Error, id: UUID) { guard identity == id, !Task.isCancelled else { return }; emit(.failed(ChatService.failureDescription(error)), id); if identity == id { cancel() } }
    private func stopCapture() {
        timer?.cancel(); timer = nil; capturing = false; engine.stop()
        if tapped { engine.inputNode.removeTap(onBus: 0); tapped = false }
        if let interruption { NotificationCenter.default.removeObserver(interruption); self.interruption = nil }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
    func finish() {
        guard capturing, let id = identity else { return }
        stopCapture()
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--voice-preview") {
            sender?.cancel()
            sender = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(850))
                guard !Task.isCancelled else { return }
                guard let self else { return }
                self.emit(.text(self.previewText, final: true), id)
            }
            return
        }
        #endif
        do {
            if let tail = try converter?.finish() { try audio?.append(tail) }
        } catch { fail(error, id: id); return }
        converter = nil; audio?.finish(); audio = nil
        // A user can finish before the handshake. Bound that drain as well as
        // the final recognition wait, without spending its 10 seconds connecting.
        startFinalTimeout(id, uploadPending: true)
    }
    private func startFinalTimeout(_ id: UUID, uploadPending: Bool = false) {
        finalTimer?.cancel()
        finalTimer = Task { [weak self] in
            try? await Task.sleep(for: .seconds(uploadPending ? 30 : 10))
            if !Task.isCancelled { self?.fail(LocalFailure.message(uploadPending ? "语音上传超时，请重试。" : "等待最终转写超时。"), id: id) }
        }
    }
    func cancel() {
        identity = nil; handler = nil; stopCapture(); audio?.finish(); audio = nil
        sender?.cancel(); sender = nil; receiver?.cancel(); receiver = nil; finalTimer?.cancel(); finalTimer = nil
        connection?.cancel(); connection = nil; converter = nil
    }
    nonisolated static func level(_ data: Data) -> Float {
        guard data.count > 1 else { return 0 }
        var sum: Float = 0
        data.withUnsafeBytes { raw in
            for offset in stride(from: 0, to: data.count - 1, by: 2) {
                let value = Float(Int16(littleEndian: raw.loadUnaligned(fromByteOffset: offset, as: Int16.self))) / 32768
                sum += value * value
            }
        }
        return min(1, sqrt(sum / Float(data.count / 2)) * 6)
    }
}
