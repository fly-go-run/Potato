import XCTest
@testable import PotatoMobile

final class RemoteModelTests: XCTestCase {
    private func catalog() throws -> RemoteModelCatalog {
        try JSONDecoder().decode(RemoteModelCatalog.self, from: Data(#"{"version":1,"active":{"provider_id":"p","model":"one","reasoning_effort":"high"},"models":[{"provider_id":"p","provider_name":"Fixture","id":"one","name":"One","effort_options":["low","high"],"default_effort":"high"},{"provider_id":"p","provider_name":"Fixture","id":"unknown","name":"Unknown","effort_options":[],"default_effort":null}]}"#.utf8))
    }
    func testFollowDesktopResolvesOnceAndDefaultExplicitlyOmitsEffort() throws {
        let catalog = try catalog(), chosen = try catalog.resolve(nil)
        XCTAssertEqual(chosen.model, "one"); XCTAssertEqual(chosen.reasoning_effort, "high")
        let value = RemoteModelChoice(provider_id: "p", model: "one", reasoning_effort: nil)
        XCTAssertNil(try catalog.resolve(value).reasoning_effort)
        XCTAssertTrue(value.arguments["reasoning_effort"] is NSNull)
        let copy = try JSONDecoder().decode(RemoteModelChoice.self, from: JSONEncoder().encode(value))
        XCTAssertEqual(copy, value); XCTAssertTrue(copy.arguments["reasoning_effort"] is NSNull)
    }
    func testUnknownAndStaleCapabilitiesDoNotInventOrSilentlyReplaceEffort() throws {
        let catalog = try catalog()
        XCTAssertTrue(catalog.models[1].effort_options.isEmpty)
        XCTAssertThrowsError(try catalog.resolve(RemoteModelChoice(provider_id: "p", model: "unknown", reasoning_effort: "high")))
        XCTAssertThrowsError(try catalog.resolve(RemoteModelChoice(provider_id: "p", model: "gone", reasoning_effort: nil)))
        XCTAssertThrowsError(try catalog.resolve(RemoteModelChoice(provider_id: "p", model: "one", reasoning_effort: "max")))
        XCTAssertEqual(try catalog.resolve(catalog.models[1].choice).model, "unknown")
        XCTAssertNil(try JSONDecoder().decode(RemoteModelOverview.self, from: Data("{}".utf8)).model_catalog)
    }
    func testLegacyPendingDoesNotAcquireANewModelChoiceDuringDecoding() throws {
        let data = Data(#"{"id":"old","text":"old text","chatID":null,"projectPath":null}"#.utf8)
        let request = try JSONDecoder().decode(RemotePendingSend.self, from: data)
        XCTAssertNil(request.modelChoice); XCTAssertNil(request.expectedRunID)
        let device = RemoteDevice(id: "one", name: "Mac", relay: URL(string: "https://fixture.invalid")!)
        XCTAssertNil(try request.bound(to: device).arguments(for: device)["model_choice"])
    }
    func testSteeringPayloadRetainsExactRunIdentity() throws {
        let device = RemoteDevice(id: "one", name: "Mac", relay: URL(string: "https://fixture.invalid")!)
        let request = RemotePendingSend(expectedRunID: "running-request", id: "attempt", text: "more", chatID: "chat", projectPath: nil, target: RemoteTargetIdentity(device))
        let restored = try JSONDecoder().decode(RemotePendingSend.self, from: JSONEncoder().encode(request))
        XCTAssertEqual(try restored.arguments(for: device)["expected_run_id"] as? String, "running-request")
        XCTAssertNil(try restored.arguments(for: device)["model_choice"])
    }
}

@MainActor final class RemoteModelDraftTests: XCTestCase {
    func testChoiceIsTargetScopedAndPendingRetrySurvivesPreferenceChangeAndRelaunch() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let repo = RemoteDraftRepository(file: root.appendingPathComponent("drafts.json"))
        let first = RemoteDevice(id: "one", name: "Mac", relay: URL(string: "https://fixture.invalid")!, owner: "same")
        let second = RemoteDevice(id: "two", name: "Mac", relay: first.relay, owner: "same")
        let choice = RemoteModelChoice(provider_id: "p", model: "one", reasoning_effort: "high")
        let draft = RemoteDraftSession(device: first, repository: repo); draft.load()
        draft.text = "instruction"; try draft.chooseModel(choice)
        let request = try draft.prepareSend(modelChoice: choice)
        try draft.chooseModel(nil)
        let restored = RemoteDraftSession(device: first, repository: repo); restored.load()
        XCTAssertNil(restored.modelChoice); XCTAssertEqual(restored.pending, request)
        XCTAssertEqual(restored.pending?.modelChoice, choice)
        let other = RemoteDraftSession(device: second, repository: repo); other.load()
        XCTAssertNil(other.modelChoice); XCTAssertNil(other.pending)
        XCTAssertThrowsError(try request.arguments(for: second))
        try restored.chooseModel(choice)
        try restored.acknowledge(request, chatID: "created")
        let conversation = RemoteDraftSession(device: first, chatID: "created", repository: repo); conversation.load()
        XCTAssertEqual(conversation.modelChoice, choice); XCTAssertNil(conversation.pending)
        let fresh = RemoteDraftSession(device: first, repository: repo); fresh.load()
        XCTAssertNil(fresh.modelChoice)
    }
}
