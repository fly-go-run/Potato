import XCTest
import UIKit
import ImageIO
import UniformTypeIdentifiers
@testable import PotatoMobile

@MainActor
final class SandboxTests: XCTestCase {
    private func storage() -> LocalStorage { LocalStorage(root: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)) }
    func testSamePNGWithDifferentMetadataKeepsNamedFileAndDistinctChart() throws {
        func png(_ color: UIColor, dpi: Int) throws -> Data {
            let format = UIGraphicsImageRendererFormat(); format.scale = 1
            let image = UIGraphicsImageRenderer(size: CGSize(width: 16, height: 16), format: format).image { context in color.setFill(); context.fill(CGRect(x: 0, y: 0, width: 16, height: 16)) }
            let result = NSMutableData()
            let destination = try XCTUnwrap(CGImageDestinationCreateWithData(result, UTType.png.identifier as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try XCTUnwrap(image.cgImage), [kCGImagePropertyDPIWidth: dpi, kCGImagePropertyDPIHeight: dpi] as CFDictionary)
            XCTAssertTrue(CGImageDestinationFinalize(destination)); return result as Data
        }
        let preview = try png(.red, dpi: 72), named = try png(.red, dpi: 144), other = try png(.blue, dpi: 72)
        XCTAssertNotEqual(preview, named)
        XCTAssertEqual(ImageImport.pngPixelDigest(preview), ImageImport.pngPixelDigest(named))
        XCTAssertNotEqual(ImageImport.pngPixelDigest(preview), ImageImport.pngPixelDigest(other))
        let disk = storage(), store = WorkspaceStore(storage: disk), reply = store.selected.messages.last!
        let artifacts = [SandboxArtifact(name: "chart-1.png", mime: "image/png", base64: preview.base64EncodedString()), SandboxArtifact(name: "chart.png", mime: "image/png", base64: named.base64EncodedString()), SandboxArtifact(name: "other.png", mime: "image/png", base64: other.base64EncodedString())]
        try store.saveExecution(SandboxExecution(status: "complete", stdout: "", stderr: "", text: "", artifacts: artifacts), messageID: reply.id, conversationID: store.selectedID)
        XCTAssertEqual(store.selected.messages.last?.attachments.map(\.name), ["chart.png", "other.png"])
        XCTAssertEqual(try Data(contentsOf: disk.url(for: store.selected.messages.last!.attachments[0])), named)
    }

    func testRequestUsesSameOriginAndOnlySelectedFiles() throws {
        let disk = storage()
        let input = try disk.importData(Data("value\n1\n2".utf8), name: "数据.csv", type: .commaSeparatedText)
        var settings = ConnectionSettings(); settings.demo = false; settings.endpoint = "https://potato.example/v1/chat/completions"
        let request = try SandboxService.request(code: "print(3)", files: [input], settings: settings, token: "device-fixture", storage: disk)
        XCTAssertEqual(request.url?.absoluteString, "https://potato.example/v1/sandbox/run")
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: request.httpBody!) as? [String: Any])
        let files = try XCTUnwrap(body["files"] as? [[String: String]])
        XCTAssertEqual(files.count, 1); XCTAssertEqual(files[0]["name"], "input-1.csv")
        XCTAssertEqual(Data(base64Encoded: files[0]["base64"]!), Data("value\n1\n2".utf8))
        XCTAssertNil(body["messages"]); XCTAssertNil(body["model"])
        settings.endpoint = "https://api.deepseek.com/chat/completions"
        XCTAssertThrowsError(try SandboxService.request(code: "1", files: [], settings: settings, token: "", storage: disk))
    }
    func testArtifactsPersistAndStayWithReplyVersion() throws {
        let disk = storage(), store = WorkspaceStore(storage: disk)
        let reply = store.selected.messages.last!
        let result = SandboxExecution(status: "complete", stdout: "3", stderr: "", text: "", artifacts: [SandboxArtifact(name: "report.md", mime: "text/markdown", base64: Data("# Report".utf8).base64EncodedString())])
        try store.saveExecution(result, messageID: reply.id, conversationID: store.selectedID)
        let file = try XCTUnwrap(store.selected.messages.last?.attachments.first)
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: file).path))
        XCTAssertEqual(store.selected.messages.last?.execution?.artifacts.count, 0)
        store.retry(); store.stop(); store.chooseReplyVersion(store.selected.messages.last!.id, offset: -1); store.persist()
        let restored = WorkspaceStore(storage: disk)
        XCTAssertEqual(restored.selected.messages.last?.displayAttachments.first?.id, file.id)
        XCTAssertEqual(restored.selected.messages.last?.displayExecution?.stdout, "3")
        XCTAssertTrue(FileManager.default.fileExists(atPath: disk.url(for: file).path))
        XCTAssertThrowsError(try disk.importArtifact(SandboxArtifact(name: "../x.html", mime: "text/html", base64: "YQ==")))
    }
}
