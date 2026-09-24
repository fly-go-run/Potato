import XCTest
import SwiftUI
@testable import PotatoMobile

final class StreamingTests: XCTestCase {
    @MainActor
    func testRemountedStreamingReplyStartsAtItsFullReceivedHeight() throws {
        try XCTSkipIf(UIAccessibility.isReduceMotionEnabled, "This regression exercises animated streaming.")
        let text = (1...20).map { "第\($0)段，已经收到的回复内容。" }.joined(separator: "\n\n")
        let bounds = CGSize(width: 350, height: 10_000)
        let reference = UIHostingController(rootView: MarkdownContent(text: text, streaming: true))
        let expected = reference.sizeThatFits(in: bounds).height
        XCTAssertGreaterThan(expected, 500)
        // Remounts happen when returning to a conversation during generation.
        for _ in 0..<3 {
            let host = UIHostingController(rootView: StreamingMarkdown(text: text, streaming: true)
                .environment(\.scenePhase, .active))
            XCTAssertEqual(host.sizeThatFits(in: bounds).height, expected, accuracy: 1)
        }
    }
    func testRemountedBufferOnlyAnimatesNewText() {
        let received = String(repeating: "已经显示的中文🌱\n", count: 100)
        var buffer = StreamingTextBuffer(visible: received)
        buffer.update(received, animated: true, now: 0)
        XCTAssertEqual(buffer.visible, received); XCTAssertFalse(buffer.pending)
        buffer.update(received + "新的回复", animated: true, now: 1)
        XCTAssertEqual(buffer.visible, received)
        buffer.advance(now: 1.3)
        XCTAssertEqual(buffer.visible, received + "新的回复")
    }
    func testSplitBlockMarkersNeverRewriteThePrecedingParagraph() {
        for suffix in ["`", "``", "```", "```s", "```swift", "#", "## ", "-", "- ", "- [", "- [x]", "1", "1.", ">"] {
            XCTAssertEqual(MarkdownBlock.parse("已有正文\n" + suffix, streaming: true), [.paragraph("已有正文")], suffix)
        }
        XCTAssertEqual(MarkdownBlock.parse("已有正文\n```swift\nlet x = 1", streaming: true), [.paragraph("已有正文"), .code("swift", "let x = 1")])
        XCTAssertEqual(MarkdownBlock.parse("已有正文\n# 标题", streaming: true), [.paragraph("已有正文"), .heading(1, "标题")])
        XCTAssertEqual(MarkdownBlock.parse("已有正文\n123 个", streaming: true), [.paragraph("已有正文\n123 个")])
    }
    func testSplitClosingFencesNeverAddTemporaryCodeLines() {
        for fence in ["```", "~~~~"] {
            let text = fence + "swift\nlet x = 1\n"
            for count in 0...fence.count {
                XCTAssertEqual(MarkdownBlock.parse(text + String(fence.prefix(count)), streaming: true), [.code("swift", "let x = 1")])
            }
            let literal = text + String(fence.prefix(1)) + "literal"
            XCTAssertEqual(MarkdownBlock.parse(literal, streaming: true), [.code("swift", "let x = 1\n" + String(fence.prefix(1)) + "literal")])
        }
    }
    func testCompletionReleasesUnresolvedMarkdownAndInvalidatesCache() {
        let cache = MarkdownRenderCache()
        for text in ["已有正文\n`", "已有正文\n123", "```swift\nlet x = 1\n``"] {
            XCTAssertNotEqual(cache.blocks(for: text, streaming: true), MarkdownBlock.parse(text))
            XCTAssertEqual(cache.blocks(for: text, streaming: false), MarkdownBlock.parse(text))
        }
    }
    func testBurstsRevealImmediatelyAndCatchUpWithinQuarterSecondWithoutSplittingGraphemes() {
        let text = "中文👨‍👩‍👧‍👦e\u{301}🇨🇳" + String(repeating: "平滑输出🌱", count: 1000)
        var buffer = StreamingTextBuffer()
        buffer.update(text, animated: true, now: 0)
        XCTAssertEqual(buffer.visible, "中")
        for tick in 1...8 {
            buffer.advance(now: Double(tick) * 0.032)
            XCTAssertTrue(text.hasPrefix(buffer.visible))
        }
        XCTAssertEqual(buffer.visible, text); XCTAssertFalse(buffer.pending)
    }
    func testUnevenChunksNeverLoseTextAndCompletionOrReplacementFlushes() {
        var buffer = StreamingTextBuffer(), text = ""
        for tick in 0..<80 {
            if tick % 3 == 0 { text += "中🌱"; buffer.update(text, animated: true, now: Double(tick) * 0.032) }
            buffer.advance(now: Double(tick) * 0.032)
            XCTAssertTrue(text.hasPrefix(buffer.visible))
        }
        buffer.update(text, animated: false, now: 3)
        XCTAssertEqual(buffer.visible, text)
        buffer.update("换版本", animated: true, now: 4)
        XCTAssertEqual(buffer.visible, "换版本")
        buffer.update("无动画", animated: false, now: 5)
        XCTAssertEqual(buffer.visible, "无动画"); XCTAssertFalse(buffer.pending)
    }
    func testSplitExtendedGraphemeCanReplaceLastCharacterSafely() {
        var buffer = StreamingTextBuffer()
        buffer.update("👨", animated: true, now: 0)
        buffer.update("👨‍👩‍👧‍👦", animated: true, now: 0.04)
        buffer.advance(now: 1)
        XCTAssertEqual(buffer.visible, "👨‍👩‍👧‍👦")
    }
    func testCloudSSEByteSplitsHeartbeatsAndTerminalPagination() throws {
        var decoder = CloudReplyEventDecoder(cursor: 0)
        var pages: [CloudReplyPage] = []
        let event = CloudReplyFixtureProtocol.delta("中文🌱")
        let first: [String: Any] = ["state": "complete", "last": 2, "events": [["seq": 1, "data": event]]]
        let last: [String: Any] = ["state": "complete", "last": 2, "events": [["seq": 2, "data": event]]]
        for body in [first, last] {
            let json = String(decoding: try JSONSerialization.data(withJSONObject: body), as: UTF8.self)
            for byte in ": heartbeat\r\n\r\ndata: \(json)\r\n\r\n".utf8 {
                if let page = try decoder.consume(byte) { pages.append(page) }
            }
        }
        XCTAssertEqual(decoder.cursor, 2); XCTAssertTrue(decoder.isEmpty)
        XCTAssertEqual(pages.map { $0.events.first!.seq }, [1, 2])
        XCTAssertEqual(pages[0].events[0].data, event)
    }
    func testCloudSSERejectsGapAndOutOfRangeCursor() throws {
        for sequence in [0, 2] {
            var decoder = CloudReplyEventDecoder(cursor: 0)
            let json: [String: Any] = ["state": "running", "last": 2, "events": [["seq": sequence, "data": "{}"]]]
            let frame = "data: " + String(decoding: try JSONSerialization.data(withJSONObject: json), as: UTF8.self) + "\n\n"
            XCTAssertThrowsError(try frame.utf8.forEach { _ = try decoder.consume($0) })
        }
    }
}
