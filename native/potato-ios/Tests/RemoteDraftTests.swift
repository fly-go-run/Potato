import XCTest
@testable import PotatoMobile

@MainActor final class RemoteDraftTests: XCTestCase {
    private var directory: URL!
    private var repository: RemoteDraftRepository!
    private let first = RemoteDevice(id: "first", name: "First Mac", relay: URL(string: "https://fixture.invalid")!, owner: "account")
    private let second = RemoteDevice(id: "second", name: "Second Mac", relay: URL(string: "https://fixture.invalid")!, owner: "account")
    override func setUp() async throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        repository = RemoteDraftRepository(file: directory.appendingPathComponent("drafts.json"))
    }
    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
    }
    private func session(_ device: RemoteDevice? = nil, chatID: String? = nil) -> RemoteDraftSession {
        let value = RemoteDraftSession(device: device ?? first, chatID: chatID, repository: repository)
        value.load(); return value
    }
    func testQueuedIntentSurvivesRestartAndAcknowledgementPreservesNewDraft() throws {
        let value = session(chatID: "chat")
        value.text = "next task"
        let request = try value.prepareSend(deliveryMode: .queue)
        let restored = session(chatID: "chat")
        XCTAssertEqual(restored.pending?.deliveryMode, .queue)
        XCTAssertEqual(restored.pending?.id, request.id)
        XCTAssertEqual(try request.arguments(for: first)["delivery_mode"] as? String, "queue")
        XCTAssertNil(try request.arguments(for: first)["expected_run_id"])
        restored.text = "another draft"
        try restored.acknowledge(request, chatID: "chat")
        XCTAssertEqual(restored.text, "another draft")
        XCTAssertNil(restored.pending)
    }
    func testInterruptRetryKeepsExactRunAndIdlePrecondition() throws {
        let value = session(chatID: "chat"); value.text = "urgent"
        let request = try value.prepareSend(expectedRunID: "observed-run", deliveryMode: .interrupt)
        XCTAssertEqual(session(chatID: "chat").pending, request)
        XCTAssertEqual(try request.arguments(for: first)["expected_run_id"] as? String, "observed-run")
        XCTAssertEqual(try request.arguments(for: first)["delivery_mode"] as? String, "interrupt")
        XCTAssertThrowsError(try request.arguments(for: second))
        try value.reject(request)
        let idle = try value.prepareSend(deliveryMode: .interrupt)
        XCTAssertTrue(try idle.arguments(for: first)["expected_run_id"] is NSNull)
    }
    func testExplicitUnconfirmedArchivePreservesIdentityAndSurvivesNextConversation() throws {
        let model = session(); model.text = "Possibly executed"
        let request = try model.prepareSend()
        try model.archiveUnconfirmed(request)
        XCTAssertNil(model.pending); XCTAssertEqual(model.text, "")
        XCTAssertEqual(model.archived, [request])
        XCTAssertEqual(session().archived, [request]); XCTAssertTrue(session(second).archived.isEmpty)
        model.text = "A different instruction"
        let next = try model.prepareSend()
        XCTAssertNotEqual(next.id, request.id)
        try model.acknowledge(next, chatID: "new-chat")
        XCTAssertEqual(session(chatID: "new-chat").archived, [request])
        XCTAssertEqual(session().archived, [request])
    }
    func testUnconfirmedArchiveKeepsEditedDraftAndRejectsStaleOrWrongTarget() throws {
        let model = session(); model.text = "Original"
        let request = try model.prepareSend()
        model.text = "An independently edited follow-up"
        XCTAssertThrowsError(try repository.archiveUnconfirmed(request, at: RemoteDraftAddress(device: second)))
        XCTAssertEqual(session().pending, request)
        try model.archiveUnconfirmed(request)
        XCTAssertEqual(model.text, "An independently edited follow-up")
        XCTAssertEqual(model.archived[0].text, "Original")
        let next = try model.prepareSend()
        XCTAssertThrowsError(try model.archiveUnconfirmed(request))
        XCTAssertEqual(session().pending, next)
        XCTAssertEqual(session().archived, [request])
    }
    func testAcknowledgementPromotesDraftAndRelaunchRestoresFollowup() throws {
        let model = session(); model.text = "Start work"
        let request = try model.prepareSend()
        try model.acknowledge(request, chatID: "created-chat")
        XCTAssertTrue(model.text.isEmpty); XCTAssertNil(model.pending)
        model.text = "Continue in that same task"; model.flush()
        XCTAssertTrue(session().text.isEmpty)
        let reopened = session(chatID: "created-chat")
        XCTAssertEqual(reopened.text, "Continue in that same task")
        let followup = try reopened.prepareSend()
        XCTAssertEqual(followup.chatID, "created-chat")
        XCTAssertEqual(followup.target, RemoteTargetIdentity(first))
    }
    func testPendingSurvivesRelaunchAndEditsWithoutChangingPayload() throws {
        let model = session(); model.text = "Original instruction"
        let request = try model.prepareSend()
        model.text = "Unsent next thought"; model.flush()
        let reopened = session()
        XCTAssertEqual(reopened.pending, request)
        XCTAssertEqual(reopened.text, "Unsent next thought")
        XCTAssertEqual(try request.arguments(for: first)["text"] as? String, "Original instruction")
        XCTAssertThrowsError(try reopened.prepareSend())
        try reopened.acknowledge(request, chatID: "created-chat")
        XCTAssertEqual(session(chatID: "created-chat").text, "Unsent next thought")
    }
    func testLateReceiptPreservesAnIndependentlyEditedConversationDraft() throws {
        let model = session(); model.text = "First instruction"
        let request = try model.prepareSend()
        model.text = "Followup written before acknowledgment"; model.flush()
        let existing = session(chatID: "created-chat")
        existing.text = "Draft already written in history"; existing.flush()
        try model.acknowledge(request, chatID: "created-chat")
        XCTAssertEqual(model.text, "Draft already written in history")
        XCTAssertEqual(model.otherDrafts, ["Followup written before acknowledgment"])
        try model.chooseOtherDraft("Followup written before acknowledgment")
        XCTAssertEqual(model.otherDrafts, ["Draft already written in history"])
        XCTAssertEqual(session(chatID: "created-chat").text, "Followup written before acknowledgment")
    }
    func testLateReceiptDoesNotEraseANewerPendingCommand() throws {
        let model = session(); model.text = "First"
        let old = try model.prepareSend()
        try model.acknowledge(old, chatID: "first-chat")
        let next = session(); next.text = "Second"
        let newer = try next.prepareSend()
        let originalAddress = RemoteDraftAddress(device: first)
        _ = try repository.acknowledge(old, at: originalAddress, chatID: "first-chat")
        XCTAssertEqual(try repository.load(originalAddress).pending, newer)
    }
    func testDefinitiveRejectionPreservesTextAndPermitsANewAttempt() throws {
        let model = session(); model.text = "Invalid request"
        let request = try model.prepareSend()
        try model.reject(request)
        XCTAssertEqual(model.text, "Invalid request"); XCTAssertNil(model.pending)
        model.text = "Corrected request"
        XCTAssertNotEqual(try model.prepareSend().id, request.id)
    }
    func testEmptyDraftDoesNotReserveAnOperation() {
        let model = session(); model.text = " \n "
        XCTAssertThrowsError(try model.prepareSend())
        XCTAssertNil(model.pending)
    }
    func testCorruptRepositoryIsNotOverwrittenOrSent() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let bytes = Data("broken original".utf8)
        try bytes.write(to: repository.file)
        let model = session()
        XCTAssertNotNil(model.storageError)
        model.text = "Still held in memory"; model.flush()
        XCTAssertThrowsError(try model.prepareSend())
        XCTAssertEqual(try Data(contentsOf: repository.file), bytes)
    }
    func testFailedPersistenceNeverCreatesAnInMemoryPendingSend() throws {
        try Data("This path is a file".utf8).write(to: directory)
        let model = session(); model.text = "Do not send without a durable record"
        XCTAssertThrowsError(try model.prepareSend())
        XCTAssertNil(model.pending)
        XCTAssertEqual(model.text, "Do not send without a durable record")
    }
    func testDestinationIdentitySeparatesAccountsOriginsAndTypedScopes() {
        let a = RemoteDraftAddress(device: first)
        let same = RemoteDevice(id: first.id, name: "Renamed", relay: URL(string: "https://FIXTURE.invalid:443/")!, owner: first.owner)
        XCTAssertEqual(a, RemoteDraftAddress(device: same))
        let otherOwner = RemoteDevice(id: first.id, name: first.name, relay: first.relay, owner: "other")
        let otherOrigin = RemoteDevice(id: first.id, name: first.name, relay: URL(string: "http://fixture.invalid:443")!, owner: first.owner)
        XCTAssertNotEqual(a.key, RemoteDraftAddress(device: otherOwner).key)
        XCTAssertNotEqual(a.key, RemoteDraftAddress(device: otherOrigin).key)
        XCTAssertNotEqual(RemoteDraftAddress(device: first, chatID: "new").key, a.key)
        XCTAssertNotEqual(RemoteDraftAddress(device: first, chatID: "/project").key, RemoteDraftAddress(device: first, projectPath: "/project").key)
    }
    func testStoredMismatchedTargetCannotBeLoaded() throws {
        let a = RemoteDraftAddress(device: first)
        let request = RemotePendingSend(id: "id", text: "text", chatID: nil, projectPath: nil, target: RemoteTargetIdentity(second))
        XCTAssertThrowsError(try repository.reserve(request, at: a, text: "text"))
        XCTAssertNil(try repository.load(a).pending)
        let record = RemoteDraftRecord(text: "text", pending: request)
        let rawRecord = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record))
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try JSONSerialization.data(withJSONObject: ["version": 1, "drafts": [a.key: rawRecord]]).write(to: repository.file)
        XCTAssertThrowsError(try repository.load(a))
    }
    func testMismatchedAcknowledgementCannotMoveAnExistingConversation() throws {
        let model = session(chatID: "original-chat"); model.text = "Followup"
        let request = try model.prepareSend()
        XCTAssertThrowsError(try model.acknowledge(request, chatID: "wrong-chat"))
        XCTAssertEqual(session(chatID: "original-chat").pending, request)
        XCTAssertNil(session(chatID: "wrong-chat").pending)
    }
    func testQueuedBubbleAppearsImmediatelyAndDeduplicatesByOperation() throws {
        let model = session(chatID: "chat"); model.text = "Same text"
        let request = try model.prepareSend(deliveryMode: .queue)
        let immediate = RemoteOutbox.visibleItems(queue: nil, pending: request, sending: true, delivered: [])
        XCTAssertEqual(immediate.map(\.id), [request.id])
        XCTAssertEqual(immediate.first?.state, "sending")
        let queue = RemoteOutbox(items: [
            .init(id: request.id, text: request.text, state: "pending", attachments: 0),
            .init(id: "another", text: request.text, state: "pending", attachments: 0)
        ], paused: false, reason: "", interrupt: false)
        let acknowledged = RemoteOutbox.visibleItems(queue: queue, pending: request, sending: false, delivered: [])
        XCTAssertEqual(acknowledged.map(\.id), [request.id, "another"])
        let delivered = RemoteOutbox.visibleItems(queue: queue, pending: request, sending: false, delivered: [request.id])
        XCTAssertEqual(delivered.map(\.id), ["another"])
        let unconfirmed = RemoteOutbox.visibleItems(queue: nil, pending: request, sending: false, delivered: [])
        XCTAssertEqual(unconfirmed.first?.state, "unconfirmed")
    }

}
