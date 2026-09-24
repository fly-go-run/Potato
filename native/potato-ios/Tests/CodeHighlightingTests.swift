import XCTest
@testable import PotatoMobile

final class CodeHighlightingTests: XCTestCase {
    private func fragments(_ source: String, _ language: String, _ kind: CodeHighlighting.Kind) -> [String] {
        CodeHighlighting.tokens(source, language: language).filter { $0.kind == kind }.map { (source as NSString).substring(with: $0.range) }
    }
    func testPythonDoesNotColorCommentsInsideStringsOrKeywordsInsideComments() {
        let code = "def greet(name: str):\n    # return 12\n    return \"中文🌱 # if 42\"\n"
        XCTAssertEqual(fragments(code, "py", .keyword), ["def", "return"])
        XCTAssertEqual(fragments(code, "python3", .comment), ["# return 12"])
        XCTAssertEqual(fragments(code, "PYTHON", .string), ["\"中文🌱 # if 42\""])
        XCTAssertEqual(fragments(code, "python", .function), ["greet"])
        XCTAssertTrue(fragments(code, "python", .number).isEmpty)
    }
    func testMultilineAndIncompleteStringsAreSafeDuringStreaming() {
        let code = "text = \"\"\"first\n# not a comment\nreturn 42\"\"\"\nprint(17)"
        XCTAssertEqual(fragments(code, "python", .string).count, 1)
        XCTAssertTrue(fragments(code, "python", .comment).isEmpty)
        XCTAssertEqual(fragments(code, "python", .number), ["17"])
        for end in code.indices {
            let prefix = String(code[..<end])
            XCTAssertEqual(String(CodeHighlighting.attributed(prefix, language: "python", dark: false).characters), prefix)
        }
    }
    func testAllLanguagesPreserveSourceExactlyAndProduceHighlights() {
        let samples = [
            "python": "def greet():\n  return '你好 e\u{301} 👨‍👩‍👧‍👦'\n", "md": "# Title\n- **bold** and `code`\n[link](https://example.test)",
            "json": "{\"name\": \"中文\", \"count\": 12, \"ok\": true}", "js": "const text = `// string`; // comment\nconsole.log(1)",
            "ts": "interface User { name: string }", "sh": "# comment\nexport NAME=\"value\"\necho $NAME", "yml": "name: value\nok: true\n# comment",
            "html": "<!-- <fake> -->\n<div class=\"card\">你好</div>", "xml": "<item id=\"1\"/>", "css": ".card { color: #aabbcc; padding: 14px; }",
            "sql": "SELECT name FROM users WHERE id = 1 -- comment", "swift": "let value: String = \"hello\"", "rs": "fn main() { let n = 42; }",
            "go": "func main() { return }", "c": "int main() { return 0; }", "cpp": "class Demo { public: int value; };", "java": "public class Demo {}", "csharp": "public class Demo {}"
        ]
        for (language, source) in samples {
            XCTAssertFalse(CodeHighlighting.tokens(source, language: language).isEmpty, language)
            for dark in [false, true] {
                XCTAssertEqual(String(CodeHighlighting.attributed(source, language: language, dark: dark).characters), source, language)
            }
        }
        XCTAssertEqual(fragments(samples["json"]!, "json", .type), ["\"name\"", "\"count\"", "\"ok\""])
        XCTAssertEqual(fragments(samples["sql"]!, "sql", .keyword), ["SELECT", "FROM", "WHERE"])
    }
    func testUnknownPlainTextAndOversizedBlocksRemainUnchanged() {
        let source = "return 42 # plain text"
        for language in ["", "text", "plaintext", "unknown-language"] {
            XCTAssertTrue(CodeHighlighting.tokens(source, language: language).isEmpty)
            XCTAssertEqual(String(CodeHighlighting.attributed(source, language: language, dark: false).characters), source)
        }
        let large = String(repeating: "return 42\n", count: 8000)
        XCTAssertTrue(CodeHighlighting.tokens(large, language: "python").isEmpty)
        XCTAssertEqual(CodeHighlighting.language("  PYTHON linenums"), "python")
    }
    func testMarkdownSourceCanContainShorterNestedFencesAndTildeFences() {
        let source = "# Title\n```python\nprint(1)\n```\n"
        XCTAssertEqual(MarkdownBlock.parse("````markdown\n" + source + "````\nafter"), [.code("markdown", String(source.dropLast())), .paragraph("after")])
        XCTAssertEqual(MarkdownBlock.parse("~~~json\n{\"n\": 1}\n~~~"), [.code("json", "{\"n\": 1}")])
        XCTAssertEqual(MarkdownBlock.parse("```python\nprint(1)\n```not-closing\nprint(2)\n```"), [.code("python", "print(1)\n```not-closing\nprint(2)")])
    }
}
