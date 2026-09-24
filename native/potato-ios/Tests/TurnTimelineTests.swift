import XCTest
@testable import PotatoMobile

final class TurnTimelineTests: XCTestCase {
    private var savedLanguage = AppLanguage.system
    override func setUp() { savedLanguage = AppLocalization.shared.selection; AppLocalization.shared.selection = .simplifiedChinese }
    override func tearDown() { AppLocalization.shared.selection = savedLanguage }

    private func frame(_ id: String, _ role: String, _ kind: String, text: String = "", name: String? = nil, arguments: String? = nil, callID: String? = nil) -> RemoteMessage {
        RemoteMessage(id: id, role: role, kind: kind, text: text, status: "completed", callID: callID, name: name, arguments: arguments, output: nil, state: nil)
    }

    func testRemoteReplyKeepsCommentaryBetweenToolGroups() {
        let frames = [frame("u", "user", "message", text: "看下桌面"), frame("r", "assistant", "reasoning", text: "plan"),
                      frame("a", "assistant", "message", text: "我先看看。"), frame("c", "assistant", "function_call", name: "list_directory", callID: "1"),
                      frame("o", "tool", "function_call_output", text: "[]", callID: "1"), frame("a2", "assistant", "message", text: "整理好了。")]
        let row = RemoteConversationRow.make(frames)[1]
        XCTAssertEqual(row.segments.map(\.id), ["process:r", "a", "process:c", "a2"])
        guard case .process(let group) = row.segments[2] else { return XCTFail() }
        XCTAssertEqual(group.map(\.id), ["c", "o"])
        // Output frames pair with their call rather than becoming a second step.
        XCTAssertEqual(ActivityStep.remote(group, running: false, confirmed: true, activeID: nil).count, 1)
    }

    func testLegacyOutputJoinsThePrecedingCall() {
        let frames = [frame("c", "assistant", "function_call", text: "list_directory\n{\"path\":\"~/Desktop\"}"), frame("o", "tool", "function_call_output", text: "entries")]
        let steps = ActivityStep.remote(frames, running: false, confirmed: true, activeID: nil)
        XCTAssertEqual(steps.count, 1); XCTAssertEqual(steps[0].output, "entries")
        XCTAssertEqual(steps.actionSummary, "查看 1 个目录")
    }
    func testSummaryNamesActionsAndCountsDistinctFiles() {
        let frames = [frame("1", "assistant", "function_call", name: "read_file", arguments: #"{"path":"a.md"}"#),
                      frame("2", "assistant", "function_call", name: "read_file", arguments: #"{"path":"a.md"}"#),
                      frame("3", "assistant", "function_call", name: "read_file", arguments: #"{"path":"b.md"}"#),
                      frame("4", "assistant", "function_call", name: "execute_shell_command", arguments: #"{"command":"npm test\nextra"}"#),
                      frame("5", "assistant", "function_call", name: "execute_shell_command", arguments: #"{"command":"ls"}"#)]
        let steps = ActivityStep.remote(frames, running: false, confirmed: true, activeID: nil)
        XCTAssertEqual(steps.actionSummary, "读取 2 个文件、执行 2 条命令")
        XCTAssertEqual(steps[3].title, "npm test")
        XCTAssertEqual(frames[3].processTitle, "执行命令")
        AppLocalization.shared.selection = .english
        XCTAssertEqual(steps.actionSummary, "Read 2 files, ran 2 commands")
        XCTAssertEqual(Array(steps.prefix(1)).actionSummary, "Read a file")
    }

    func testLocalReplySplitsTextAtRecordedActivities() {
        var message = ChatMessage(role: "assistant", text: "", state: .streaming)
        message.receive(ReplyDelta(reasoning: "Plan"))
        message.receive(ReplyDelta(text: "我先搜一下。"))
        message.searches = [WebSearchRun(id: "s", query: "topic", state: "complete", results: [])]
        message.recordActivity("search:s")
        message.receive(ReplyDelta(text: "\n\n找到了答案。"))
        let segments = message.turnSegments
        XCTAssertEqual(segments.count, 4)
        guard case .activity(_, let thought) = segments[0], case .text(_, let first) = segments[1],
              case .activity(_, let search) = segments[2], case .text(_, let rest) = segments[3] else { return XCTFail() }
        XCTAssertEqual(thought.map(\.id), ["reasoning"])
        XCTAssertEqual(first, "我先搜一下。")
        XCTAssertEqual(search.map(\.id), ["search:s"])
        XCTAssertEqual(rest, "找到了答案。")
        let restored = try! JSONDecoder().decode(ChatMessage.self, from: JSONEncoder().encode(message))
        XCTAssertEqual(restored.turnSegments.map(\.id), segments.map(\.id))
    }

    func testLegacyReplyAndOpenCodeFenceKeepStepsTogether() {
        var legacy = ChatMessage(role: "assistant", text: "Answer")
        legacy.searches = [WebSearchRun(id: "s", query: "q", state: "complete", results: [])]
        legacy.activityOrder = ["search:s"]
        XCTAssertEqual(legacy.turnSegments.map(\.id), ["activity:search:s", "text:0"])
        var fenced = ChatMessage(role: "assistant", text: "", state: .streaming)
        fenced.receive(ReplyDelta(text: "```python\nprint(1)\n"))
        fenced.searches = [WebSearchRun(id: "s", query: "q", state: "complete", results: [])]
        fenced.recordActivity("search:s")
        fenced.receive(ReplyDelta(text: "```\nDone"))
        // No split inside the fence: the step stays ahead of the whole text.
        XCTAssertEqual(fenced.turnSegments.count, 2)
    }
}
