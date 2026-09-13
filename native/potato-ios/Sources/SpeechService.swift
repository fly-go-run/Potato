import Foundation
import AVFoundation

struct SpeechEvent: Decodable { let type: String; let text: String?; let message: String? }
private final class SpeechNoRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}
enum SpeechService {
    static func request(settings: ConnectionSettings, token: String) throws -> URLRequest {
        guard !settings.demo, let endpoint = settings.validatedURL, endpoint.path.hasSuffix("/v1/chat/completions"), !token.isEmpty else { throw LocalFailure.message("请先连接支持豆包语音的 Potato 服务。") }
        var components = URLComponents(url: endpoint, resolvingAgainstBaseURL: false)!
        components.scheme = "wss"; components.path = String(endpoint.path.dropLast("/v1/chat/completions".count)) + "/v1/audio/transcriptions"
        var request = URLRequest(url: components.url!); request.timeoutInterval = 20
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        return request
    }
}
protocol SpeechTransport: Sendable {
    func connect() async throws
    func send(_ data: Data) async throws
    func finish() async throws
}

// Keep the beginning of the recording while the two WebSocket handshakes run.
// The byte limit is 20 seconds of 16 kHz mono PCM, independent of hardware buffers.
final class SpeechAudioBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private let stream: AsyncThrowingStream<Data, Error>
    private let continuation: AsyncThrowingStream<Data, Error>.Continuation
    private let maximumBytes: Int
    private var pendingBytes = 0
    private var ended = false
    init(maximumBytes: Int = 640_000) {
        self.maximumBytes = maximumBytes
        let pair = AsyncThrowingStream<Data, Error>.makeStream()
        stream = pair.stream; continuation = pair.continuation
    }
    func append(_ data: Data) throws {
        lock.lock(); defer { lock.unlock() }
        guard !ended, !data.isEmpty else { return }
        guard data.count <= maximumBytes - pendingBytes else {
            throw LocalFailure.message("语音连接较慢，录音缓存已满，请重新录音。")
        }
        pendingBytes += data.count; continuation.yield(data)
    }
    func finish() {
        lock.lock(); defer { lock.unlock() }
        ended = true; continuation.finish()
    }
    private func sent(_ count: Int) {
        lock.lock(); defer { lock.unlock() }; pendingBytes -= count
    }
    @MainActor
    func upload(to connection: any SpeechTransport, connected: () -> Void) async throws {
        try Task.checkCancellation()
        try await connection.connect()
        try Task.checkCancellation(); connected()
        for try await data in stream {
            try Task.checkCancellation()
            try await connection.send(data); sent(data.count)
        }
        try Task.checkCancellation()
        try await connection.finish()
    }
}

final class SpeechConnection: SpeechTransport, @unchecked Sendable {
    private let session: URLSession
    private let socket: URLSessionWebSocketTask
    init(request: URLRequest) {
        let configuration = URLSessionConfiguration.ephemeral; configuration.timeoutIntervalForResource = 90
        session = URLSession(configuration: configuration, delegate: SpeechNoRedirect(), delegateQueue: nil)
        socket = session.webSocketTask(with: request); socket.maximumMessageSize = 1_000_000
    }
    func connect() async throws {
        // URLSession's resource timeout is longer than our startup audio buffer.
        let deadline = Task { [socket] in
            do { try await Task.sleep(for: .seconds(20)); socket.cancel(with: .goingAway, reason: nil) }
            catch { }
        }
        defer { deadline.cancel() }
        socket.resume(); let event = try await receive()
        guard event.type == "ready" else { throw LocalFailure.message("豆包语音连接未就绪，请稍后重试。") }
    }
    func send(_ data: Data) async throws { try await socket.send(.data(data)) }
    func finish() async throws { try await socket.send(.string("{\"type\":\"stop\"}")) }
    func receive() async throws -> SpeechEvent {
        let message = try await socket.receive()
        let data: Data
        switch message { case .string(let value): data = Data(value.utf8); case .data(let value): data = value; @unknown default: throw LocalFailure.message("语音服务响应无效。") }
        let event = try JSONDecoder().decode(SpeechEvent.self, from: data)
        if event.type == "error" { throw LocalFailure.message(event.message ?? "豆包语音识别失败，请稍后重试。") }
        guard ["ready", "partial", "final"].contains(event.type), (event.text?.count ?? 0) <= 30000 else { throw LocalFailure.message("语音服务响应无效。") }
        return event
    }
    func cancel() { socket.cancel(with: .goingAway, reason: nil); session.invalidateAndCancel() }
}
final class VoicePCMConverter {
    private let converter: AVAudioConverter
    private let output: AVAudioFormat
    init(input: AVAudioFormat) throws {
        guard input.sampleRate > 0, input.channelCount > 0,
              let output = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true),
              let converter = AVAudioConverter(from: input, to: output) else { throw LocalFailure.message("未检测到可用麦克风。") }
        converter.primeMethod = .none
        self.output = output; self.converter = converter
    }
    func convert(_ input: AVAudioPCMBuffer) throws -> Data {
        let capacity = AVAudioFrameCount(ceil(Double(input.frameLength) * 16000 / input.format.sampleRate)) + 16
        guard let buffer = AVAudioPCMBuffer(pcmFormat: output, frameCapacity: capacity) else { throw LocalFailure.message("录音转换失败。") }
        var supplied = false, error: NSError?
        let status = converter.convert(to: buffer, error: &error) { _, state in
            if supplied { state.pointee = .noDataNow; return nil }; supplied = true; state.pointee = .haveData; return input
        }
        guard status != .error, error == nil, let samples = buffer.int16ChannelData?[0] else { throw LocalFailure.message("录音转换失败。") }
        return Data(bytes: samples, count: Int(buffer.frameLength) * 2)
    }
    func finish() throws -> Data {
        var result = Data()
        for _ in 0..<8 {
            guard let buffer = AVAudioPCMBuffer(pcmFormat: output, frameCapacity: 1024) else { throw LocalFailure.message("录音转换失败。") }
            var error: NSError?
            let status = converter.convert(to: buffer, error: &error) { _, state in state.pointee = .endOfStream; return nil }
            guard status != .error, error == nil else { throw LocalFailure.message("录音转换失败。") }
            if buffer.frameLength > 0, let samples = buffer.int16ChannelData?[0] { result.append(Data(bytes: samples, count: Int(buffer.frameLength) * 2)) }
            if status == .endOfStream || buffer.frameLength == 0 { break }
        }
        return result
    }
}
