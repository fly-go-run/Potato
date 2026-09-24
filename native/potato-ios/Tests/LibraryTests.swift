import XCTest
import UniformTypeIdentifiers
import UIKit
import PDFKit
@testable import PotatoMobile

@MainActor final class LibraryTests: XCTestCase {
    var disk: LocalStorage!
    override func setUp() { super.setUp(); disk = LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)) }
    override func tearDown() { try? FileManager.default.removeItem(at: disk.root); super.tearDown() }

    func testLegacyMigrationIncludesAllVersionsButNotUnsentDraftAndIsIdempotent() throws {
        let sent = try disk.importData(Data("已发送".utf8), name: "报告.txt", type: .plainText)
        let previous = try disk.importData(Data("旧版产物".utf8), name: "旧版.txt", type: .plainText)
        let draft = try disk.importData(Data("草稿".utf8), name: "草稿.txt", type: .plainText)
        var chat = Conversation()
        var message = ChatMessage(role: "assistant", text: "文件", attachments: [sent])
        message.versions = [ReplyVersion(attachments: [previous], text: "旧版", state: .complete, failure: nil)]
        chat.messages = [message]; chat.pendingAttachments = [draft]
        let old = SavedWorkspace(version: 1, conversations: [chat], selectedID: chat.id)
        try disk.save(old)
        let store = WorkspaceStore(storage: disk)
        XCTAssertEqual(Set(store.visibleLibrary.map { $0.attachment.name }), ["报告.txt", "旧版.txt"])
        XCTAssertEqual(store.library.first?.sources.first?.messageID, message.id)
        XCTAssertEqual(try disk.load()?.version, 2)
        let ids = Set(store.library.map(\.id))
        try store.trashLibrary(ids)
        let reload = WorkspaceStore(storage: disk)
        XCTAssertTrue(reload.visibleLibrary.isEmpty)
        XCTAssertEqual(Set(reload.library.map(\.id)), ids)
        try reload.permanentlyDeleteLibrary(ids)
        XCTAssertTrue(WorkspaceStore(storage: disk).library.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: sent).path))
    }
    func testLibraryAndConversationsHaveIndependentLifetimesAndSharedBytesSurviveCleanup() throws {
        let store = WorkspaceStore(storage: disk)
        let id = try store.saveLibraryText("独立资料", name: "资料.txt")
        let file = store.library[0].attachment
        try store.useLibrary([id])
        let chatID = store.selectedID
        store.send(); store.stop()
        store.trash(chatID)
        XCTAssertEqual(store.visibleLibrary.count, 1)
        try store.trashLibrary([id])
        XCTAssertEqual(store.conversations.first { $0.id == chatID }?.messages.last { $0.role == "user" }?.attachments.first?.libraryID, id)
        try store.permanentlyDeleteLibrary([id])
        _ = WorkspaceStore(storage: disk)
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: file).path))
    }
    func testStandaloneAndTrashFilesSurviveRestartButUnreferencedPermanentDeletionReclaimsBytes() throws {
        let store = WorkspaceStore(storage: disk)
        let id = try store.saveLibraryText("保留原件")
        let file = store.library[0].attachment
        XCTAssertEqual(WorkspaceStore(storage: disk).visibleLibrary.count, 1)
        try store.trashLibrary([id]); _ = WorkspaceStore(storage: disk)
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: file).path))
        try store.permanentlyDeleteLibrary([id]); _ = WorkspaceStore(storage: disk)
        XCTAssertFalse(FileManager.default.fileExists(atPath: disk.url(for: file).path))
    }
    func testContentDedupAndExplicitRestoreWithoutAutomaticResurrection() throws {
        let store = WorkspaceStore(storage: disk)
        let first = try store.saveLibraryText("相同内容", name: "第一个")
        let second = try store.saveLibraryText("相同内容", name: "第二个")
        XCTAssertEqual(first, second); XCTAssertEqual(store.library.count, 1)
        try store.trashLibrary([first])
        try store.collectLibrary([store.library[0].attachment], conversationID: store.selectedID, messageID: UUID())
        XCTAssertTrue(store.visibleLibrary.isEmpty)
        XCTAssertEqual(try store.saveLibraryText("相同内容"), first)
        XCTAssertEqual(store.visibleLibrary.count, 1)
    }
    func testUseDoesNotSendOrEraseDraftAndSelectionLimitIsAtomic() throws {
        let store = WorkspaceStore(storage: disk)
        store.update { $0.input = "别覆盖草稿" }
        let a = try store.saveLibraryText("甲"), b = try store.saveLibraryText("乙")
        let before = store.selected.messages
        try store.useLibrary([a, b])
        XCTAssertEqual(store.selected.messages, before); XCTAssertEqual(store.selected.input, "别覆盖草稿")
        XCTAssertEqual(store.selected.pendingAttachments.count, 2)
        try store.useLibrary([a]); XCTAssertEqual(store.selected.pendingAttachments.count, 2)
        var extra = Set<UUID>()
        for n in 1...19 { extra.insert(try store.saveLibraryText("更多\(n)")) }
        XCTAssertThrowsError(try store.useLibrary(extra))
        XCTAssertEqual(store.selected.pendingAttachments.count, 2)
        try store.useLibrary(Set(extra.prefix(18)))
        XCTAssertEqual(store.selected.pendingAttachments.count, 20)
        let old = store.selectedID
        try store.useLibrary([a], newConversation: true)
        XCTAssertNotEqual(store.selectedID, old)
        XCTAssertEqual(store.conversations.first { $0.id == old }?.input, "别覆盖草稿")
        XCTAssertTrue(store.selected.messages.isEmpty)
    }
    func testOriginalImageAndChatDerivativeAreIndependentAndDontDuplicateLibrary() throws {
        let image = UIGraphicsImageRenderer(size: CGSize(width: 40, height: 40)).image { context in UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 40, height: 40)) }
        let data = try XCTUnwrap(image.pngData())
        let store = WorkspaceStore(storage: disk)
        let original = try disk.importLibraryData(data, name: "原图.png", type: .png)
        let id = try store.saveLibraryFile(original)
        try store.useLibrary([id])
        let outgoing = try XCTUnwrap(store.selected.pendingAttachments.first)
        XCTAssertEqual(outgoing.type, "image/jpeg"); XCTAssertNotEqual(outgoing.filename, original.filename)
        XCTAssertEqual(try Data(contentsOf: disk.url(for: original)), data)
        try store.collectLibrary([outgoing], conversationID: store.selectedID, messageID: UUID())
        XCTAssertEqual(store.library.count, 1)
    }
    func testScannedPDFOfficeAndLongTextCanBeSavedButUnsupportedChatUseDoesNotMutateDraft() throws {
        let store = WorkspaceStore(storage: disk)
        let data = UIGraphicsPDFRenderer(bounds: CGRect(x: 0, y: 0, width: 100, height: 100)).pdfData { $0.beginPage() }
        let scan = try disk.importLibraryData(data, name: "扫描.pdf", type: .pdf)
        let long = try disk.importLibraryData(Data(String(repeating: "字", count: 60_010).utf8), name: "长文.txt", type: .plainText)
        let office = try disk.importLibraryData(Data("Office test fixture".utf8), name: "演示.pptx", type: UTType(filenameExtension: "pptx")!)
        for file in [scan, long, office] {
            let id = try store.saveLibraryFile(file)
            XCTAssertNotNil(disk.libraryChatLimitation(file))
            XCTAssertThrowsError(try store.useLibrary([id], newConversation: true))
        }
        XCTAssertEqual(store.library.count, 3); XCTAssertTrue(store.selected.pendingAttachments.isEmpty)
        XCTAssertEqual(try Data(contentsOf: disk.url(for: long)).count, 60_010 * 3)
    }
    func testRenameSearchAndExportRetainOriginalExtensionAndReadableFilename() throws {
        let store = WorkspaceStore(storage: disk)
        let id = try store.saveLibraryText("# 正文\n关键词：独立留存", name: "说明.txt")
        try store.renameLibrary(id, name: "我的资料")
        let item = try XCTUnwrap(store.library.first)
        XCTAssertEqual(item.attachment.name, "我的资料.txt")
        XCTAssertTrue(item.matches("独立留存")); XCTAssertNotNil(item.excerpt(for: "独立留存"))
        let url = try disk.libraryShareURL(item.attachment)
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        XCTAssertEqual(url.lastPathComponent, "我的资料.txt")
        XCTAssertTrue(try String(contentsOf: url, encoding: .utf8).contains("独立留存"))
    }
    func testFailedPersistenceCannotReportSuccessfulLibraryMutation() throws {
        let store = WorkspaceStore(storage: disk)
        let id = try store.saveLibraryText("必须保留")
        try FileManager.default.removeItem(at: disk.stateURL)
        try FileManager.default.createDirectory(at: disk.stateURL, withIntermediateDirectories: false)
        XCTAssertThrowsError(try store.trashLibrary([id]))
        XCTAssertEqual(store.visibleLibrary.count, 1)
    }
    func testFileImportPreservesBytesAndRejectsOversizeAndUnsupportedFiles() throws {
        let source = disk.root.appendingPathComponent("import.md")
        try disk.prepare()
        let bytes = Data("# 导入文件\n正文内容".utf8); try bytes.write(to: source)
        let imported = try disk.importLibraryFile(source)
        XCTAssertEqual(try Data(contentsOf: disk.url(for: imported)), bytes)
        XCTAssertEqual(imported.name, "import.md")
        XCTAssertThrowsError(try disk.importLibraryData(Data(repeating: 0, count: 10 * 1_024 * 1_024 + 1), name: "大文件.pdf", type: .pdf))
        XCTAssertThrowsError(try disk.importLibraryData(Data([1, 2, 3]), name: "程序.bin", type: .data))
    }
    func testMissingFileMigrationIsVisibleAndCannotBeUsed() throws {
        let file = Attachment(name: "失效.txt", filename: "missing.txt", type: "text/plain", size: 10)
        var chat = Conversation(); chat.messages = [ChatMessage(role: "user", text: "旧资料", attachments: [file])]
        try disk.save(SavedWorkspace(version: 1, conversations: [chat], selectedID: chat.id))
        let store = WorkspaceStore(storage: disk)
        XCTAssertEqual(store.library.count, 1)
        XCTAssertThrowsError(try store.useLibrary([store.library[0].id]))
    }
}
