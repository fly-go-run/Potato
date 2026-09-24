import XCTest
@testable import PotatoMobile

final class CloudModelsFixture: URLProtocol {
    nonisolated(unsafe) static var captured: [URLRequest] = []
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "cloud-models.invalid" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var request = request
        if request.httpBody == nil, let stream = request.httpBodyStream {
            stream.open(); defer { stream.close() }
            var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(buffer, count: n) }
            request.httpBody = data
        }
        Self.captured.append(request)
        let body = Data(#"{"object":"list","catalog_source":"configured","default_model":"sub2api/new","revision":4,"can_edit":true,"data":[{"id":"deepseek/flash","name":"Flash"},{"id":"sub2api/new","name":"New"}]}"#.utf8)
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body); client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

@MainActor final class CloudModelsTests: XCTestCase {
    func testSaveSendsRevisionAndInstallsReturnedCatalog() async throws {
        let config = URLSessionConfiguration.ephemeral; config.protocolClasses = [CloudModelsFixture.self]
        let store = WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)), streamConfiguration: config, tokenProvider: { "synthetic" })
        store.settings.cloudAccount = RemoteAccountProfile(owner: "cloud-models-test", email: "owner@example.test", relay: URL(string: "https://cloud-models.invalid")!, scope: "cloud")
        store.settings.endpoint = "https://cloud-models.invalid/v1/chat/completions"; store.settings.model = "deepseek/flash"
        store.settings.modelCatalog = LocalModelCatalog(endpoint: store.settings.serviceIdentity!, models: [LocalModelEntry(id: "deepseek/flash", name: "Flash")], defaultModel: "deepseek/flash", revision: 3, canEdit: true)
        CloudModelsFixture.captured = []
        try await store.saveCloudModels(["deepseek/flash", "sub2api/new"], defaultModel: "sub2api/new")
        let request = try XCTUnwrap(CloudModelsFixture.captured.first)
        XCTAssertEqual(request.httpMethod, "PUT"); XCTAssertEqual(request.url?.path, "/v1/models/enabled")
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: request.httpBody ?? Data()) as? [String: Any])
        XCTAssertEqual(body["revision"] as? Int, 3); XCTAssertEqual(body["models"] as? [String], ["deepseek/flash", "sub2api/new"])
        XCTAssertEqual(store.settings.currentCatalog?.revision, 4); XCTAssertEqual(store.settings.currentCatalog?.canEdit, true)
        XCTAssertEqual(store.settings.currentCatalog?.models.map(\.id), ["deepseek/flash", "sub2api/new"])
        XCTAssertEqual(store.settings.model, "deepseek/flash")
    }
}
