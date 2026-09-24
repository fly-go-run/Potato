import XCTest
import UIKit
@testable import PotatoMobile

@MainActor final class ComposerAttachmentTests: XCTestCase {
    private func store() -> WorkspaceStore {
        WorkspaceStore(storage: LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)))
    }
    private func file(_ name: String, in store: WorkspaceStore) throws -> Attachment {
        try store.storage.importData(Data(name.utf8), name: name + ".txt", type: .plainText)
    }

    func testRapidTapsReserveCapacityDeduplicateAndKeepOrder() async throws {
        let store = store(), imports = ComposerAttachments(), id = store.selectedID
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        var loads: [Int] = []
        for index in 0..<20 {
            XCTAssertTrue(imports.add(key: "photo-\(index)", to: id, store: store) {
                if index == 0 { try await Task.sleep(for: .milliseconds(20)) }
                loads.append(index); return try self.file(String(index), in: store)
            })
        }
        XCTAssertEqual(imports.remaining(in: store.selected), 0)
        XCTAssertFalse(imports.add(key: "photo-0", to: id, store: store) { XCTFail("Duplicate must not load"); return try self.file("duplicate", in: store) })
        XCTAssertFalse(imports.add(key: "twenty-first", to: id, store: store) { XCTFail("Twenty-first must not load"); return try self.file("twenty-first", in: store) })
        await imports.waitUntilFinished()
        XCTAssertEqual(loads, Array(0..<20))
        XCTAssertEqual(store.selected.pendingAttachments.map(\.name), (0..<20).map { "\($0).txt" })
        XCTAssertEqual(imports.count(in: id), 0)
        XCTAssertEqual(try store.storage.load()?.conversations.first { $0.id == id }?.pendingAttachments.count, 20)
        store.removePendingAttachment(store.selected.pendingAttachments[0].id)
        XCTAssertNil(imports.attachment(for: "photo-0", in: store.selected))
        XCTAssertTrue(imports.add(key: "photo-0", to: id, store: store) { try self.file("readded", in: store) })
        await imports.waitUntilFinished()
        XCTAssertEqual(store.selected.pendingAttachments.last?.name, "readded.txt")
    }

    func testFailureReleasesSlotAndRetriesWithoutDroppingOtherFiles() async throws {
        let store = store(), imports = ComposerAttachments(), id = store.selectedID
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        imports.add(key: "bad", to: id, store: store) { throw LocalFailure.message("offline") }
        imports.add(key: "good", to: id, store: store) { try self.file("good", in: store) }
        await imports.waitUntilFinished()
        XCTAssertEqual(imports.failure(in: id), "offline")
        XCTAssertEqual(imports.remaining(in: store.selected), 19)
        XCTAssertEqual(store.selected.pendingAttachments.map(\.name), ["good.txt"])
        imports.add(key: "bad", to: id, store: store) { try self.file("retry", in: store) }
        await imports.waitUntilFinished()
        XCTAssertNil(imports.failure(in: id))
        XCTAssertEqual(store.selected.pendingAttachments.map(\.name), ["good.txt", "retry.txt"])
    }

    func testSwitchingChatsDoesNotRedirectAnImport() async throws {
        let store = store(), imports = ComposerAttachments(), original = store.selectedID
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        imports.add(key: "photo", to: original, store: store) {
            try await Task.sleep(for: .milliseconds(20)); return try self.file("original", in: store)
        }
        store.newChat()
        XCTAssertNotEqual(store.selectedID, original)
        await imports.waitUntilFinished()
        XCTAssertTrue(store.selected.pendingAttachments.isEmpty)
        XCTAssertEqual(store.conversations.first { $0.id == original }?.pendingAttachments.first?.name, "original.txt")
    }

    func testDeletedChatReleasesQueuedResourcesWithoutImporting() async {
        let store = store(), imports = ComposerAttachments(), id = store.selectedID
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        var released = false
        imports.add(to: id, store: store, onFinish: { released = true }) {
            XCTFail("Deleted conversation must not import"); return try self.file("unused", in: store)
        }
        store.trash(id)
        await imports.waitUntilFinished()
        XCTAssertTrue(released)
        XCTAssertEqual(imports.count(in: id), 0)
    }

    func testLibraryFillingDraftDuringImportCannotExceedLimit() async throws {
        let store = store(), imports = ComposerAttachments(), id = store.selectedID
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        let imported = try file("pending", in: store)
        imports.add(to: id, store: store) { imported }
        for index in 0..<20 { store.addAttachment(try file("library-\(index)", in: store)) }
        await imports.waitUntilFinished()
        XCTAssertEqual(store.selected.pendingAttachments.count, 20)
        XCTAssertFalse(FileManager.default.fileExists(atPath: store.storage.url(for: imported).path))
        XCTAssertNotNil(imports.failure(in: id))
    }

    func testTwentyImagesAreAllSerializedWithinRequestBudget() throws {
        let store = store()
        defer { try? FileManager.default.removeItem(at: store.storage.root) }
        // High-entropy pixels exercise compression, rather than a tiny solid-color image.
        let side = 512
        var pixels = [UInt8](repeating: 255, count: side * side * 4)
        var random: UInt32 = 0x12345678
        for index in pixels.indices where index % 4 != 3 {
            random ^= random << 13; random ^= random >> 17; random ^= random << 5
            pixels[index] = UInt8(truncatingIfNeeded: random)
        }
        let provider = try XCTUnwrap(CGDataProvider(data: Data(pixels) as CFData))
        let image = try XCTUnwrap(CGImage(width: side, height: side, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: side * 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
        let data = try XCTUnwrap(UIImage(cgImage: image).pngData())
        let photos = try (0..<20).map { try store.storage.importData(data, name: "photo-\($0).png", type: .png) }
        XCTAssertGreaterThan(photos[0].size, 120_000)
        var settings = ConnectionSettings(); settings.endpoint = "https://fixture.invalid/v1/chat/completions"; settings.model = "fixture"
        let request = try ChatService.request(settings: settings, token: "", messages: [ChatMessage(role: "user", text: "Compare all photos", attachments: photos)], storage: store.storage)
        let body = try XCTUnwrap(request.httpBody)
        XCTAssertLessThanOrEqual(body.count, 4 * 1_024 * 1_024)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
        let parts = try XCTUnwrap(messages.last?["content"] as? [[String: Any]])
        XCTAssertEqual(parts.filter { $0["type"] as? String == "image_url" }.count, 20)
        for part in parts.dropFirst() {
            let url = try XCTUnwrap((part["image_url"] as? [String: String])?["url"])
            let encoded = try XCTUnwrap(url.split(separator: ",").last)
            XCTAssertLessThanOrEqual(try XCTUnwrap(Data(base64Encoded: String(encoded))).count, 120_000)
        }
        XCTAssertEqual(try Data(contentsOf: store.storage.url(for: photos[0])).count, photos[0].size)
    }
}
