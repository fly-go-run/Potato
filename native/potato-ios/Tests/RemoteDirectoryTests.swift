import XCTest
@testable import PotatoMobile

@MainActor
final class RemoteDirectoryTests: XCTestCase {
    private let profile = RemoteAccountProfile(owner: "account-a", email: "a@example.test", relay: URL(string: "https://remote.example.test")!)
    private var device: RemoteDevice { RemoteDevice(id: "mac", name: "我的电脑", relay: profile.relay, owner: profile.owner) }
    private var overview: RemoteOverview {
        RemoteOverview(chats: [RemoteChat(id: "chat", session_id: "chat", name: "已保存的会话", status: "completed", pinned: false, project_path: "/work")], projects: [RemoteProject(path: "/work", name: "work")])
    }
    private func storage() throws -> UserDefaults {
        let suite = "RemoteDirectoryTests-" + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        addTeardownBlock { defaults.removePersistentDomain(forName: suite) }
        defaults.set(try JSONEncoder().encode(profile), forKey: "remote-profile")
        return defaults
    }
    private func client(connected: Bool = true) -> RemoteDirectoryClient {
        let device = device, overview = overview
        return RemoteDirectoryClient(devices: { _ in [device] }, status: { _ in connected }, overview: { _ in overview })
    }
    func testRelaunchRestoresAccountDevicesAndListsBeforeConnecting() async throws {
        let defaults = try storage(), store = RemoteStore(defaults: defaults, directoryClient: client())
        XCTAssertTrue(store.loadingDirectory); XCTAssertFalse(store.canShowSetup)
        await store.refresh()
        XCTAssertEqual(store.online[device.id], true)
        let reopened = RemoteStore(defaults: defaults, directoryClient: client())
        XCTAssertEqual(reopened.devices, [device])
        XCTAssertEqual(reopened.overviews[device.id]?.chats.first?.name, "已保存的会话")
        XCTAssertEqual(reopened.overviews[device.id]?.projects.first?.path, "/work")
        XCTAssertNil(reopened.online[device.id]); XCTAssertEqual(reopened.connectionLabel(device), "未连接")
        XCTAssertFalse(reopened.canShowSetup)
        await reopened.refresh(); XCTAssertEqual(reopened.connectionLabel(device), "在线")
    }
    func testAccountAndServiceChangesNeverRestoreAnotherDirectory() async throws {
        let defaults = try storage(), store = RemoteStore(defaults: defaults, directoryClient: client())
        await store.refresh()
        for replacement in [RemoteAccountProfile(owner: "other", email: "other@example.test", relay: profile.relay), RemoteAccountProfile(owner: profile.owner, email: profile.email, relay: URL(string: "https://other.example.test")!)] {
            defaults.set(try JSONEncoder().encode(replacement), forKey: "remote-profile")
            let reopened = RemoteStore(defaults: defaults, directoryClient: client())
            XCTAssertTrue(reopened.devices.isEmpty); XCTAssertTrue(reopened.overviews.isEmpty)
        }
        defaults.removeObject(forKey: "remote-profile")
        let loggedOut = RemoteStore(defaults: defaults, directoryClient: client())
        XCTAssertTrue(loggedOut.devices.isEmpty); XCTAssertTrue(loggedOut.overviews.isEmpty)
    }
    func testConfirmedOfflineKeepsCachedListAndNeverFetchesOverview() async throws {
        let defaults = try storage(), store = RemoteStore(defaults: defaults, directoryClient: client())
        await store.refresh()
        var offline = client(connected: false)
        offline.overview = { _ in XCTFail("An offline computer must not be queried"); throw URLError(.notConnectedToInternet) }
        let reopened = RemoteStore(defaults: defaults, directoryClient: offline)
        await reopened.refresh()
        XCTAssertEqual(reopened.connectionLabel(device), "离线")
        XCTAssertEqual(reopened.overviews[device.id]?.chats.first?.name, "已保存的会话")
        XCTAssertFalse(reopened.canShowSetup)
    }
    func testNetworkFailureIsUnconfirmedNotOfflineAndRecovers() async throws {
        let defaults = try storage(), original = RemoteStore(defaults: defaults, directoryClient: client())
        await original.refresh()
        var fail = true
        var network = client()
        network.status = { _ in if fail { throw URLError(.timedOut) }; return true }
        let store = RemoteStore(defaults: defaults, directoryClient: network)
        await store.refresh()
        XCTAssertNil(store.online[device.id]); XCTAssertEqual(store.connectionLabel(device), "连接未确认")
        XCTAssertEqual(store.overviews[device.id]?.chats.count, 1)
        fail = false; await store.refresh()
        XCTAssertEqual(store.connectionLabel(device), "在线"); XCTAssertTrue(store.deviceIssues.isEmpty)
    }
    func testFailedDiscoveryPreservesCacheButConfirmedRemovalPrunesIt() async throws {
        let defaults = try storage(), original = RemoteStore(defaults: defaults, directoryClient: client())
        await original.refresh()
        var fail = true, network = client()
        network.devices = { _ in if fail { throw URLError(.notConnectedToInternet) }; return [] }
        let store = RemoteStore(defaults: defaults, directoryClient: network)
        await store.refresh()
        XCTAssertEqual(store.devices, [device]); XCTAssertNotNil(store.overviews[device.id])
        XCTAssertFalse(store.canShowSetup)
        fail = false; await store.refresh()
        XCTAssertTrue(store.devices.isEmpty); XCTAssertTrue(store.overviews.isEmpty); XCTAssertTrue(store.canShowSetup)
        XCTAssertTrue(RemoteStore(defaults: defaults, directoryClient: client()).devices.isEmpty)
    }
    func testNoCacheDoesNotShowSetupUntilEmptyDiscoveryCompletes() async throws {
        let defaults = try storage()
        var network = client()
        network.devices = { _ in [] }
        let store = RemoteStore(defaults: defaults, directoryClient: network)
        XCTAssertTrue(store.loadingDirectory); XCTAssertFalse(store.canShowSetup)
        await store.refresh()
        XCTAssertFalse(store.loadingDirectory); XCTAssertTrue(store.canShowSetup)
    }
    func testCancelledOverviewCannotReplaceCacheOrFakeOffline() async throws {
        let defaults = try storage(), original = RemoteStore(defaults: defaults, directoryClient: client())
        await original.refresh()
        var entered = false
        var network = client()
        network.overview = { _ in entered = true; try await Task.sleep(for: .seconds(60)); return self.overview }
        let store = RemoteStore(defaults: defaults, directoryClient: network)
        let task = Task { await store.refresh() }
        for _ in 0..<100 { if entered { break }; await Task.yield() }
        XCTAssertTrue(entered)
        task.cancel(); store.suspendConnections(); await task.value
        XCTAssertNil(store.online[device.id]); XCTAssertTrue(store.deviceIssues.isEmpty)
        XCTAssertEqual(store.overviews[device.id]?.chats.first?.name, "已保存的会话")
    }
    func testRemovedPairedDeviceCannotReturnFromSuspendedRefreshOrCache() async throws {
        let defaults = try storage()
        defaults.removeObject(forKey: "remote-profile")
        let paired = RemoteDevice(id: "paired", name: "Paired Mac", relay: profile.relay)
        defaults.set(try JSONEncoder().encode([paired]), forKey: "remote-devices")
        var continuation: CheckedContinuation<Bool, Never>?
        var network = client()
        network.status = { _ in await withCheckedContinuation { continuation = $0 } }
        let store = RemoteStore(defaults: defaults, directoryClient: network)
        let task = Task { await store.refresh() }
        for _ in 0..<100 { if continuation != nil { break }; await Task.yield() }
        XCTAssertNotNil(continuation)
        store.forget(paired)
        continuation?.resume(returning: true); await task.value
        XCTAssertTrue(store.devices.isEmpty); XCTAssertTrue(store.online.isEmpty); XCTAssertTrue(store.overviews.isEmpty)
        XCTAssertTrue(RemoteStore(defaults: defaults, directoryClient: client()).devices.isEmpty)
    }
}
