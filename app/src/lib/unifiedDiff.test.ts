import { describe, expect, it } from "vitest";
import {
  matchRepoRelativePath,
  parseUnifiedDiff,
} from "./unifiedDiff";

// Literal output from `git diff --no-color --no-ext-diff` in a temporary Git
// repository. The trailing newline from each command is intentional.
const MULTI_HUNK_DIFF = `diff --git a/multi.txt b/multi.txt
index 71c3357..61c8a3e 100644
--- a/multi.txt
+++ b/multi.txt
@@ -1,5 +1,6 @@
 zero
-one
+one-a
+one-b
 two
 three
 four
@@ -11,7 +12,8 @@ nine
 ten
 eleven
 twelve
-thirteen
+thirteen-a
+thirteen-b
 fourteen
 fifteen
 sixteen
`;

const NEW_FILE_DIFF = `diff --git a/new-file.txt b/new-file.txt
new file mode 100644
index 0000000..66a52ee
--- /dev/null
+++ b/new-file.txt
@@ -0,0 +1,2 @@
+first
+second
`;

const DELETED_FILE_DIFF = `diff --git a/deleted.txt b/deleted.txt
deleted file mode 100644
index b4f8863..0000000
--- a/deleted.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-remove-me
-keep-no
`;

const RENAME_DIFF = `diff --git a/old-name.txt b/new-name.txt
similarity index 100%
rename from old-name.txt
rename to new-name.txt
`;

const BINARY_DIFF = `diff --git a/binary.bin b/binary.bin
index 8352675..1592e5c 100644
Binary files a/binary.bin and b/binary.bin differ
`;

const CRLF_DIFF =
  "diff --git a/crlf.txt b/crlf.txt\n" +
  "index d464033..a3e2a01 100644\n" +
  "--- a/crlf.txt\n" +
  "+++ b/crlf.txt\n" +
  "@@ -1,3 +1,3 @@\n" +
  " first\r\n" +
  "-keep\r\n" +
  "+changed\r\n" +
  " last\r\n";

const NO_NEWLINE_DIFF = `diff --git a/no-newline.txt b/no-newline.txt
index d2607b7..e25628c 100644
--- a/no-newline.txt
+++ b/no-newline.txt
@@ -1,2 +1,2 @@
 old line
-last
\\ No newline at end of file
+new last
\\ No newline at end of file
`;

const FUNCTION_CONTEXT_DIFF = `diff --git a/example.c b/example.c
index ab6a54d..2b3b8aa 100644
--- a/example.c
+++ b/example.c
@@ -2,7 +2,7 @@ function foo()
 one
 two
 three
-old
+new
 six
 seven
 eight
`;

// This is the literal output of `git -c core.quotepath=false diff` for the
// path `目录/含中文 文件.txt`. Git emits a tab after paths containing spaces.
const UNICODE_AND_SPACE_PATH_DIFF = `diff --git a/目录/含中文 文件.txt b/目录/含中文 文件.txt
index 148600a..0694fda 100644
--- a/目录/含中文 文件.txt\t
+++ b/目录/含中文 文件.txt\t
@@ -1 +1,2 @@
-旧内容
+新内容
+另一行
`;

const MULTI_FILE_DIFF = `diff --git a/multi.txt b/multi.txt
index 71c3357..61c8a3e 100644
--- a/multi.txt
+++ b/multi.txt
@@ -1,5 +1,6 @@
 zero
-one
+one-a
+one-b
 two
 three
 four
@@ -11,7 +12,8 @@ nine
 ten
 eleven
 twelve
-thirteen
+thirteen-a
+thirteen-b
 fourteen
 fifteen
 sixteen
diff --git a/other.txt b/other.txt
index 85c3040..3cc64f7 100644
--- a/other.txt
+++ b/other.txt
@@ -1,3 +1,4 @@
 alpha
-beta
+BETA
 gamma
+delta
`;

describe("parseUnifiedDiff", () => {
  it("parses multiple hunks and advances both line-number streams", () => {
    const [file] = parseUnifiedDiff(MULTI_HUNK_DIFF);

    expect(file).toMatchObject({
      oldPath: "multi.txt",
      newPath: "multi.txt",
      isNew: false,
      isDeleted: false,
      isRename: false,
      isBinary: false,
      additions: 4,
      deletions: 2,
    });
    expect(file?.hunks).toHaveLength(2);
    expect(file?.hunks.map(({ oldStart, oldCount, newStart, newCount }) => ({
      oldStart,
      oldCount,
      newStart,
      newCount,
    }))).toEqual([
      { oldStart: 1, oldCount: 5, newStart: 1, newCount: 6 },
      { oldStart: 11, oldCount: 7, newStart: 12, newCount: 8 },
    ]);
    expect(file?.hunks[0]?.lines).toEqual([
      { kind: "context", text: "zero", oldLine: 1, newLine: 1 },
      { kind: "remove", text: "one", oldLine: 2, newLine: null },
      { kind: "add", text: "one-a", oldLine: null, newLine: 2 },
      { kind: "add", text: "one-b", oldLine: null, newLine: 3 },
      { kind: "context", text: "two", oldLine: 3, newLine: 4 },
      { kind: "context", text: "three", oldLine: 4, newLine: 5 },
      { kind: "context", text: "four", oldLine: 5, newLine: 6 },
    ]);
    expect(file?.hunks[1]?.lines).toEqual([
      { kind: "context", text: "ten", oldLine: 11, newLine: 12 },
      { kind: "context", text: "eleven", oldLine: 12, newLine: 13 },
      { kind: "context", text: "twelve", oldLine: 13, newLine: 14 },
      { kind: "remove", text: "thirteen", oldLine: 14, newLine: null },
      {
        kind: "add",
        text: "thirteen-a",
        oldLine: null,
        newLine: 15,
      },
      {
        kind: "add",
        text: "thirteen-b",
        oldLine: null,
        newLine: 16,
      },
      { kind: "context", text: "fourteen", oldLine: 15, newLine: 17 },
      { kind: "context", text: "fifteen", oldLine: 16, newLine: 18 },
      { kind: "context", text: "sixteen", oldLine: 17, newLine: 19 },
    ]);
  });

  it("parses a no-index new-file diff with one-based new lines", () => {
    const [file] = parseUnifiedDiff(NEW_FILE_DIFF);

    expect(file).toMatchObject({
      oldPath: "",
      newPath: "new-file.txt",
      isNew: true,
      isDeleted: false,
      isRename: false,
      isBinary: false,
      additions: 2,
      deletions: 0,
    });
    expect(file?.hunks).toEqual([
      {
        section: "",
        oldStart: 0,
        oldCount: 0,
        newStart: 1,
        newCount: 2,
        lines: [
          { kind: "add", text: "first", oldLine: null, newLine: 1 },
          { kind: "add", text: "second", oldLine: null, newLine: 2 },
        ],
      },
    ]);
  });

  it("parses a deleted-file diff with an empty new path", () => {
    const [file] = parseUnifiedDiff(DELETED_FILE_DIFF);

    expect(file).toMatchObject({
      oldPath: "deleted.txt",
      newPath: "",
      isNew: false,
      isDeleted: true,
      isRename: false,
      isBinary: false,
      additions: 0,
      deletions: 2,
    });
    expect(file?.hunks[0]).toMatchObject({
      oldStart: 1,
      oldCount: 2,
      newStart: 0,
      newCount: 0,
    });
    expect(file?.hunks[0]?.lines).toEqual([
      { kind: "remove", text: "remove-me", oldLine: 1, newLine: null },
      { kind: "remove", text: "keep-no", oldLine: 2, newLine: null },
    ]);
  });

  it("parses a staged pure rename from rename headers", () => {
    const [file] = parseUnifiedDiff(RENAME_DIFF);

    expect(file).toEqual({
      oldPath: "old-name.txt",
      newPath: "new-name.txt",
      isNew: false,
      isDeleted: false,
      isRename: true,
      isBinary: false,
      hunks: [],
      additions: 0,
      deletions: 0,
    });
  });

  it("recognizes binary files without inventing hunks", () => {
    const [file] = parseUnifiedDiff(BINARY_DIFF);

    expect(file).toMatchObject({
      isNew: false,
      isDeleted: false,
      isRename: false,
      isBinary: true,
      hunks: [],
      additions: 0,
      deletions: 0,
    });
  });

  it("strips CR from CRLF diff lines while preserving line numbers", () => {
    const [file] = parseUnifiedDiff(CRLF_DIFF);

    expect(file?.hunks[0]?.lines).toEqual([
      { kind: "context", text: "first", oldLine: 1, newLine: 1 },
      { kind: "remove", text: "keep", oldLine: 2, newLine: null },
      { kind: "add", text: "changed", oldLine: null, newLine: 2 },
      { kind: "context", text: "last", oldLine: 3, newLine: 3 },
    ]);
    expect(file).toMatchObject({ additions: 1, deletions: 1 });
  });

  it("ignores no-newline markers without shifting line numbers", () => {
    const [file] = parseUnifiedDiff(NO_NEWLINE_DIFF);

    expect(file?.hunks[0]?.lines).toEqual([
      { kind: "context", text: "old line", oldLine: 1, newLine: 1 },
      { kind: "remove", text: "last", oldLine: 2, newLine: null },
      { kind: "add", text: "new last", oldLine: null, newLine: 2 },
    ]);
    expect(file).toMatchObject({ additions: 1, deletions: 1 });
  });

  it("returns no files for empty input and keeps concatenated files independent", () => {
    expect(parseUnifiedDiff("")).toEqual([]);

    const files = parseUnifiedDiff(MULTI_FILE_DIFF);
    expect(files).toHaveLength(2);
    expect(files[0]).toMatchObject({
      oldPath: "multi.txt",
      newPath: "multi.txt",
      additions: 4,
      deletions: 2,
    });
    expect(files[0]?.hunks).toHaveLength(2);
    expect(files[1]).toMatchObject({
      oldPath: "other.txt",
      newPath: "other.txt",
      additions: 2,
      deletions: 1,
    });
    expect(files[1]?.hunks[0]?.lines).toEqual([
      { kind: "context", text: "alpha", oldLine: 1, newLine: 1 },
      { kind: "remove", text: "beta", oldLine: 2, newLine: null },
      { kind: "add", text: "BETA", oldLine: null, newLine: 2 },
      { kind: "context", text: "gamma", oldLine: 3, newLine: 3 },
      { kind: "add", text: "delta", oldLine: null, newLine: 4 },
    ]);
  });

  it("captures a Git function-context suffix as section", () => {
    const [file] = parseUnifiedDiff(FUNCTION_CONTEXT_DIFF);

    expect(file?.hunks[0]).toMatchObject({
      section: "function foo()",
      oldStart: 2,
      oldCount: 7,
      newStart: 2,
      newCount: 7,
    });
  });

  it("parses core.quotepath=false Chinese and space paths, stripping git's trailing tab", () => {
    const [file] = parseUnifiedDiff(UNICODE_AND_SPACE_PATH_DIFF);

    expect(file).toMatchObject({
      oldPath: "目录/含中文 文件.txt",
      newPath: "目录/含中文 文件.txt",
      additions: 2,
      deletions: 1,
    });
    expect(file?.hunks[0]?.lines).toEqual([
      { kind: "remove", text: "旧内容", oldLine: 1, newLine: null },
      { kind: "add", text: "新内容", oldLine: null, newLine: 1 },
      { kind: "add", text: "另一行", oldLine: null, newLine: 2 },
    ]);
  });
});

describe("matchRepoRelativePath", () => {
  it("chooses the most specific suffix match", () => {
    expect(
      matchRepoRelativePath("/workspace/a/b/x.ts", ["x.ts", "b/x.ts"]),
    ).toBe("b/x.ts");
  });

  it("normalizes backslashes and accepts a ./ candidate prefix", () => {
    expect(
      matchRepoRelativePath("C:\\repo\\src\\main.ts", [
        "src/main.ts",
      ]),
    ).toBe("src/main.ts");
    expect(
      matchRepoRelativePath("/repo/src/main.ts", ["./src/main.ts"]),
    ).toBe("./src/main.ts");
  });

  it("returns null when no candidate is a path-boundary suffix", () => {
    expect(
      matchRepoRelativePath("/repo/src/main.ts", [
        "main.ts.bak",
        "lib/main.ts",
      ]),
    ).toBeNull();
  });
});
