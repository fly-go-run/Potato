import XCTest
import AVFoundation
@testable import PotatoMobile

private actor DelayedSpeechTransport: SpeechTransport {
    let gate: AsyncStream<Void>
    let started: XCTestExpectation
    let rejected: Bool
    private(set) var packets: [Data] = []
    private(set) var stops = 0
    init(gate: AsyncStream<Void>, started: XCTestExpectation, rejected: Bool = false) {
        self.gate = gate; self.started = started; self.rejected = rejected
    }
    func connect() async throws {
        started.fulfill()
        for await _ in gate { break }
        if rejected { throw LocalFailure.message("连接失败") }
    }
    func send(_ data: Data) async throws { packets.append(data) }
    func finish() async throws { stops += 1 }
}

@MainActor
final class SearchSpeechTests: XCTestCase {
    func testRecordingBeforeConnectionAndEarlyFinishPreserveEveryPacketInOrder() async throws {
        let gate = AsyncStream<Void>.makeStream(), started = expectation(description: "connecting")
        let connection = DelayedSpeechTransport(gate: gate.stream, started: started)
        let audio = SpeechAudioBuffer()
        let upload = Task { try await audio.upload(to: connection) {} }
        await fulfillment(of: [started], timeout: 2)
        // More than the old 32-buffer queue: several seconds of speech during handshake.
        let packets = (0..<80).map { Data(repeating: UInt8($0), count: 3200) }
        for packet in packets { try audio.append(packet) }
        audio.finish()
        let before = await connection.packets, stopsBefore = await connection.stops
        XCTAssertTrue(before.isEmpty); XCTAssertEqual(stopsBefore, 0)
        gate.continuation.yield(()); gate.continuation.finish()
        try await upload.value
        let after = await connection.packets, stopsAfter = await connection.stops
        XCTAssertEqual(after, packets); XCTAssertEqual(stopsAfter, 1)
    }
    func testCancelDuringHandshakeNeverUploadsBufferedSpeechOrStop() async throws {
        let gate = AsyncStream<Void>.makeStream(), started = expectation(description: "connecting")
        let connection = DelayedSpeechTransport(gate: gate.stream, started: started)
        let audio = SpeechAudioBuffer(); try audio.append(Data([1, 2])); audio.finish()
        let upload = Task { try await audio.upload(to: connection) { XCTFail("Cancelled connection became ready") } }
        await fulfillment(of: [started], timeout: 2)
        upload.cancel(); gate.continuation.yield(()); gate.continuation.finish()
        do { try await upload.value; XCTFail("Cancellation was ignored") } catch is CancellationError {} catch { XCTFail("\(error)") }
        let packets = await connection.packets, stops = await connection.stops
        XCTAssertTrue(packets.isEmpty); XCTAssertEqual(stops, 0)
    }
    func testRejectedHandshakeNeverUploadsAudio() async throws {
        let gate = AsyncStream<Void>.makeStream(), started = expectation(description: "connecting")
        let connection = DelayedSpeechTransport(gate: gate.stream, started: started, rejected: true)
        let audio = SpeechAudioBuffer(); try audio.append(Data([1, 2])); audio.finish()
        let upload = Task { try await audio.upload(to: connection) { XCTFail("Rejected connection became ready") } }
        await fulfillment(of: [started], timeout: 2)
        gate.continuation.yield(()); gate.continuation.finish()
        do { try await upload.value; XCTFail("Connection error was ignored") } catch {}
        let packets = await connection.packets, stops = await connection.stops
        XCTAssertTrue(packets.isEmpty); XCTAssertEqual(stops, 0)
    }
    func testStartupBufferBoundsBytesWithoutSilentlyDroppingOpeningAudio() async throws {
        let audio = SpeechAudioBuffer(maximumBytes: 8)
        try audio.append(Data([1, 2])); try audio.append(Data([3, 4, 5, 6, 7, 8]))
        XCTAssertThrowsError(try audio.append(Data([9, 10])))
        audio.finish(); try audio.append(Data([11, 12]))
        let gate = AsyncStream<Void>.makeStream(), started = expectation(description: "connecting")
        gate.continuation.yield(()); gate.continuation.finish()
        let connection = DelayedSpeechTransport(gate: gate.stream, started: started)
        try await audio.upload(to: connection) {}
        await fulfillment(of: [started], timeout: 2)
        let packets = await connection.packets
        XCTAssertEqual(packets, [Data([1, 2]), Data([3, 4, 5, 6, 7, 8])])
    }

    func testSearchSourcesStreamPersistAndFollowReplyVersions() throws {
        let source = WebSource(title: "Documentation", url: "https://example.com/docs", content: "Reference")
        let run = WebSearchRun(id: "call1", query: "docs", state: "complete", results: [source])
        let data = try JSONSerialization.data(withJSONObject: ["potato_search": JSONSerialization.jsonObject(with: JSONEncoder().encode(run))])
        guard case .search(let decoded) = try SSEDecoder.decode(String(decoding: data, as: UTF8.self)) else { return XCTFail("Missing search event") }
        XCTAssertEqual(decoded, run)
        let storage = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let store = WorkspaceStore(storage: storage), id = store.selected.messages.last!.id
        store.recordSearch(run, messageID: id, conversationID: store.selectedID); store.persist()
        XCTAssertEqual(WorkspaceStore(storage: storage).selected.messages.last?.displaySearches, [run])
        store.retry(); store.stop(); store.chooseReplyVersion(store.selected.messages.last!.id, offset: -1); store.persist()
        XCTAssertEqual(WorkspaceStore(storage: storage).selected.messages.last?.displaySearches, [run])
        XCTAssertNil(WebSource(title: "Bad", url: "javascript:alert(1)", content: "").safeURL)
    }
    func testSpeechUsesWorkerDeviceAuthAndConvertsHardwarePCM() throws {
        var settings = ConnectionSettings(); settings.demo = false; settings.endpoint = "https://potato.example/v1/chat/completions"
        let request = try SpeechService.request(settings: settings, token: "device-token")
        XCTAssertEqual(request.url?.absoluteString, "wss://potato.example/v1/audio/transcriptions")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer device-token")
        XCTAssertNil(request.value(forHTTPHeaderField: "X-Api-Key"))
        settings.demo = true; XCTAssertThrowsError(try SpeechService.request(settings: settings, token: "device-token"))
        let input = try XCTUnwrap(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48000, channels: 1, interleaved: false))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: input, frameCapacity: 4800)); buffer.frameLength = 4800
        for index in 0..<4800 { buffer.floatChannelData![0][index] = 0.25 * sin(Float(index) * 2 * .pi * 440 / 48000) }
        let converter = try VoicePCMConverter(input: input)
        var data = try converter.convert(buffer)
        data.append(try converter.finish())
        XCTAssertGreaterThan(data.count, 3000); XCTAssertLessThanOrEqual(data.count, 3232); XCTAssertEqual(data.count % 2, 0)
        XCTAssertTrue(data.contains { $0 != 0 })
    }
}
