import { describe, expect, it } from "vitest";
import type { RunStatus } from "./protocol/types";
import type { StreamMessage } from "./stream";
import { buildToolPair } from "../components/chat/ToolCard";
import {
  INLINE_DIFF_MAX_LINES,
  collectFileChanges,
  editDiffLines,
  pairFileEdit,
  totalChangeStats,
  visibleDiffLines,
} from "./fileChanges";

type ToolMessageType =
  | "function_call"
  | "function_call_output"
  | "plugin_call"
  | "plugin_call_output"
  | "mcp_tool_call"
  | "mcp_tool_call_output";

function toolMessage(
  id: string,
  type: ToolMessageType,
  data: Record<string, unknown>,
  status: RunStatus = "completed",
): StreamMessage {
  const isCall =
    type === "function_call" ||
    type === "plugin_call" ||
    type === "mcp_tool_call";
  return {
    id,
    type,
    role: isCall ? "assistant" : "tool",
    status,
    metadata: null,
    content: [
      {
        object: "content",
        type: "data",
        delta: false,
        index: 0,
        status: "completed",
        msg_id: id,
        data,
      },
    ],
  };
}

function toolCall(
  id: string,
  name: string,
  argumentsValue: string,
  status: RunStatus = "completed",
  type: "function_call" | "plugin_call" | "mcp_tool_call" = "function_call",
): StreamMessage {
  return toolMessage(
    id,
    type,
    { call_id: id, name, arguments: argumentsValue },
    status,
  );
}

function toolOutput(
  id: string,
  name: string,
  options: {
    state?: string;
    status?: RunStatus;
    output?: string;
    meta?: Record<string, unknown>;
  } = {},
  type:
    | "function_call_output"
    | "plugin_call_output"
    | "mcp_tool_call_output" = "function_call_output",
): StreamMessage {
  return toolMessage(
    `${id}-output`,
    type,
    {
      call_id: id,
      name,
      output: options.output ?? "ok",
      state: options.state ?? "completed",
      ...(options.meta ? { meta: options.meta } : {}),
    },
    options.status ?? "completed",
  );
}

describe("collectFileChanges", () => {
  it("counts edit_file changes with lineDiff semantics and preserves text", () => {
    const before = "first\nunchanged\nold";
    const after = "first\nunchanged\nnew";
    const changes = collectFileChanges([
      toolCall(
        "edit-call",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/src/example.ts",
          old_text: before,
          new_text: after,
        }),
      ),
      toolOutput("edit-call", "edit_file"),
    ]);

    expect(changes).toEqual([
      {
        path: "/workspace/src/example.ts",
        name: "example.ts",
        dir: "/workspace/src",
        kind: "edit_file",
        additions: 1,
        deletions: 1,
        edits: [
          {
            messageId: "edit-call",
            tool: "edit_file",
            before,
            after,
            additions: 1,
            deletions: 1,
          },
        ],
        lastMessageId: "edit-call",
      },
    ]);
  });

  it("counts write_file content lines and treats empty content as zero additions", () => {
    const content = "first\nsecond\nthird";
    const changes = collectFileChanges([
      toolCall(
        "write-call",
        "write_file",
        JSON.stringify({ file_path: "/workspace/notes.txt", content }),
      ),
      toolOutput("write-call", "write_file"),
      toolCall(
        "empty-write-call",
        "write_file",
        JSON.stringify({ file_path: "/workspace/empty.txt", content: "" }),
      ),
      toolOutput("empty-write-call", "write_file"),
    ]);

    expect(changes).toEqual([
      expect.objectContaining({
        path: "/workspace/notes.txt",
        name: "notes.txt",
        dir: "/workspace",
        kind: "write_file",
        additions: 3,
        deletions: 0,
        edits: [
          expect.objectContaining({
            messageId: "write-call",
            tool: "write_file",
            before: "",
            after: content,
            additions: 3,
            deletions: 0,
          }),
        ],
        lastMessageId: "write-call",
      }),
      expect.objectContaining({
        path: "/workspace/empty.txt",
        additions: 0,
        deletions: 0,
        edits: [
          expect.objectContaining({
            messageId: "empty-write-call",
            before: "",
            after: "",
            additions: 0,
            deletions: 0,
          }),
        ],
        lastMessageId: "empty-write-call",
      }),
    ]);
  });

  it("counts appended lines as additions with no deletions", () => {
    const content = "new line one\nnew line two";
    const changes = collectFileChanges([
      toolCall(
        "append-call",
        "append_file",
        JSON.stringify({ file_path: "/workspace/log.txt", content }),
      ),
      toolOutput("append-call", "append_file"),
    ]);

    expect(changes).toEqual([
      expect.objectContaining({
        path: "/workspace/log.txt",
        name: "log.txt",
        dir: "/workspace",
        kind: "append_file",
        additions: 2,
        deletions: 0,
        edits: [
          expect.objectContaining({
            messageId: "append-call",
            tool: "append_file",
            before: "",
            after: content,
            additions: 2,
            deletions: 0,
          }),
        ],
        lastMessageId: "append-call",
      }),
    ]);
  });

  it("merges repeated operations by path in call order and promotes edit to write", () => {
    const changes = collectFileChanges([
      toolCall(
        "edit-one",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/mixed.ts",
          old_text: "old",
          new_text: "new",
        }),
      ),
      toolOutput("edit-one", "edit_file"),
      toolCall(
        "append-two",
        "append_file",
        JSON.stringify({
          file_path: "/workspace/mixed.ts",
          content: "line one\nline two",
        }),
      ),
      toolOutput("append-two", "append_file"),
      toolCall(
        "write-three",
        "write_file",
        JSON.stringify({
          file_path: "/workspace/mixed.ts",
          content: "replacement",
        }),
      ),
      toolOutput("write-three", "write_file"),
    ]);

    expect(changes).toHaveLength(1);
    expect(changes[0]).toMatchObject({
      path: "/workspace/mixed.ts",
      kind: "write_file",
      additions: 4,
      deletions: 1,
      lastMessageId: "write-three",
    });
    expect(changes[0]?.edits).toEqual([
      expect.objectContaining({
        messageId: "edit-one",
        tool: "edit_file",
        before: "old",
        after: "new",
        additions: 1,
        deletions: 1,
      }),
      expect.objectContaining({
        messageId: "append-two",
        tool: "append_file",
        before: "",
        after: "line one\nline two",
        additions: 2,
        deletions: 0,
      }),
      expect.objectContaining({
        messageId: "write-three",
        tool: "write_file",
        before: "",
        after: "replacement",
        additions: 1,
        deletions: 0,
      }),
    ]);
  });

  it("promotes append_file to edit_file when a later edit touches the same path", () => {
    const changes = collectFileChanges([
      toolCall(
        "append-first",
        "append_file",
        JSON.stringify({
          file_path: "/workspace/append-edit.txt",
          content: "one",
        }),
      ),
      toolOutput("append-first", "append_file"),
      toolCall(
        "edit-second",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/append-edit.txt",
          old_text: "one",
          new_text: "two",
        }),
      ),
      toolOutput("edit-second", "edit_file"),
    ]);

    expect(changes[0]).toMatchObject({
      path: "/workspace/append-edit.txt",
      kind: "edit_file",
      additions: 2,
      deletions: 1,
      lastMessageId: "edit-second",
    });
    expect(changes[0]?.edits.map((edit) => edit.messageId)).toEqual([
      "append-first",
      "edit-second",
    ]);
  });

  it("returns files in order of their first successful touch", () => {
    const changes = collectFileChanges([
      toolCall(
        "second-file-call",
        "write_file",
        JSON.stringify({ file_path: "/workspace/second.txt", content: "2" }),
      ),
      toolOutput("second-file-call", "write_file"),
      toolCall(
        "first-file-call",
        "write_file",
        JSON.stringify({ file_path: "/workspace/first.txt", content: "1" }),
      ),
      toolOutput("first-file-call", "write_file"),
      toolCall(
        "second-file-again",
        "append_file",
        JSON.stringify({
          file_path: "/workspace/second.txt",
          content: "again",
        }),
      ),
      toolOutput("second-file-again", "append_file"),
    ]);

    expect(changes.map((change) => change.path)).toEqual([
      "/workspace/second.txt",
      "/workspace/first.txt",
    ]);
  });

  it.each([
    ["plugin_call", "plugin_call_output"],
    ["mcp_tool_call", "mcp_tool_call_output"],
  ] as const)(
    "accepts %s/%s tool message envelopes",
    (callType, outputType) => {
      const changes = collectFileChanges([
        toolCall(
          `${callType}-call`,
          "write_file",
          JSON.stringify({
            file_path: `/workspace/${callType}.txt`,
            content: "x",
          }),
          "completed",
          callType,
        ),
        toolOutput(`${callType}-call`, "write_file", {}, outputType),
      ]);

      expect(changes).toHaveLength(1);
      expect(changes[0]).toMatchObject({
        path: `/workspace/${callType}.txt`,
        additions: 1,
      });
    },
  );

  it("excludes calls whose output state is error", () => {
    const changes = collectFileChanges([
      toolCall(
        "error-call",
        "write_file",
        JSON.stringify({ file_path: "/workspace/error.txt", content: "x" }),
      ),
      toolOutput("error-call", "write_file", { state: "error" }),
    ]);

    expect(changes).toEqual([]);
  });

  it.each([
    "created",
    "in_progress",
    "completed",
    "failed",
    "cancelled",
  ] as RunStatus[])(
    "excludes a call with no output regardless of call.status=%s",
    (status) => {
      const changes = collectFileChanges([
        toolCall(
          `no-output-${status}`,
          "write_file",
          JSON.stringify({
            file_path: `/workspace/no-output-${status}.txt`,
            content: "x",
          }),
          status,
        ),
      ]);

      expect(changes).toEqual([]);
    },
  );

  it("excludes output whose message status is cancelled", () => {
    const changes = collectFileChanges([
      toolCall(
        "cancelled-call",
        "append_file",
        JSON.stringify({ file_path: "/workspace/cancelled.txt", content: "x" }),
      ),
      toolOutput("cancelled-call", "append_file", {
        status: "cancelled",
      }),
    ]);

    expect(changes).toEqual([]);
  });

  it("excludes non-file-change tools", () => {
    const messages = [
      toolCall(
        "read-call",
        "read_file",
        JSON.stringify({ file_path: "/workspace/read.txt" }),
      ),
      toolOutput("read-call", "read_file"),
      toolCall(
        "send-call",
        "send_file_to_user",
        JSON.stringify({ file_path: "/workspace/send.txt" }),
      ),
      toolOutput("send-call", "send_file_to_user"),
    ];

    expect(collectFileChanges(messages)).toEqual([]);
  });

  it("excludes invalid arguments, missing paths, and empty edits", () => {
    const messages = [
      toolCall("invalid-json", "write_file", "not valid JSON"),
      toolOutput("invalid-json", "write_file"),
      toolCall(
        "missing-path",
        "write_file",
        JSON.stringify({ content: "has content" }),
      ),
      toolOutput("missing-path", "write_file"),
      toolCall(
        "empty-edit",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/empty-edit.txt",
          old_text: "",
          new_text: "",
        }),
      ),
      toolOutput("empty-edit", "edit_file"),
    ];

    expect(collectFileChanges(messages)).toEqual([]);
  });

  it("prefers backend qp meta counts over local estimates", () => {
    // 后端 meta 报的 ±(全局替换的真值)与本地 LCS 对单份 old/new 的
    // 估计(1/1)不同——断言 meta 赢,证明没有静默回落。
    const editChanges = collectFileChanges([
      toolCall(
        "meta-edit",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/multi.ts",
          old_text: "old",
          new_text: "new",
        }),
      ),
      toolOutput("meta-edit", "edit_file", {
        meta: {
          v: 1,
          kind: "file_edit",
          ok: true,
          data: { path: "/workspace/multi.ts", additions: 3, deletions: 3 },
        },
      }),
    ]);
    expect(editChanges[0].additions).toBe(3);
    expect(editChanges[0].deletions).toBe(3);

    // 覆盖写:本地只能按"新增全部行"高估,meta 报真实差异。
    const writeChanges = collectFileChanges([
      toolCall(
        "meta-write",
        "write_file",
        JSON.stringify({
          file_path: "/workspace/rewrite.txt",
          content: "a\nb\nc",
        }),
      ),
      toolOutput("meta-write", "write_file", {
        meta: {
          v: 1,
          kind: "file_write",
          ok: true,
          data: { path: "/workspace/rewrite.txt", additions: 1, deletions: 2 },
        },
      }),
    ]);
    expect(writeChanges[0].additions).toBe(1);
    expect(writeChanges[0].deletions).toBe(2);
  });

  it("falls back to local estimates when qp meta is malformed", () => {
    const changes = collectFileChanges([
      toolCall(
        "bad-meta",
        "write_file",
        JSON.stringify({ file_path: "/workspace/x.txt", content: "a\nb" }),
      ),
      toolOutput("bad-meta", "write_file", {
        meta: { v: 99, kind: "file_write" },
      }),
    ]);
    expect(changes[0].additions).toBe(2);
    expect(changes[0].deletions).toBe(0);
  });
});

describe("editDiffLines", () => {
  it("aligns edit_file with lineDiff and treats write/append as all adds", () => {
    expect(
      editDiffLines({
        tool: "edit_file",
        before: "keep\nold",
        after: "keep\nnew",
      }),
    ).toEqual([
      { kind: "same", text: "keep" },
      { kind: "remove", text: "old" },
      { kind: "add", text: "new" },
    ]);
    expect(
      editDiffLines({
        tool: "write_file",
        before: "",
        after: "one\ntwo",
      }),
    ).toEqual([
      { kind: "add", text: "one" },
      { kind: "add", text: "two" },
    ]);
    expect(
      editDiffLines({
        tool: "append_file",
        before: "",
        after: "tail",
      }),
    ).toEqual([{ kind: "add", text: "tail" }]);
  });

  it("skips line alignment for oversized edits", () => {
    expect(
      editDiffLines({
        tool: "edit_file",
        before: "a\nb",
        after: "c",
        oversized: true,
      }),
    ).toEqual([
      { kind: "remove", text: "a" },
      { kind: "remove", text: "b" },
      { kind: "add", text: "c" },
    ]);
  });
});

describe("visibleDiffLines", () => {
  it("keeps short diffs intact and reports the omitted tail", () => {
    const short = ["a", "b", "c"];
    expect(visibleDiffLines(short, 3)).toEqual({
      visible: short,
      truncated: 0,
    });
    const long = Array.from({ length: INLINE_DIFF_MAX_LINES + 7 }, (_, i) => i);
    expect(visibleDiffLines(long)).toEqual({
      visible: long.slice(0, INLINE_DIFF_MAX_LINES),
      truncated: 7,
    });
  });
});

describe("pairFileEdit", () => {
  it("returns an in-flight write so the raw row can preview before completion", () => {
    const pair = buildToolPair(
      toolCall(
        "live-write",
        "write_file",
        JSON.stringify({ file_path: "/workspace/live.txt", content: "alpha" }),
        "in_progress",
      ),
      null,
    );
    expect(pairFileEdit(pair)).toMatchObject({
      tool: "write_file",
      after: "alpha",
      additions: 1,
      deletions: 0,
    });
  });
});

describe("totalChangeStats", () => {
  it("summarizes files, additions, and deletions and returns zeros for empty input", () => {
    const changes = collectFileChanges([
      toolCall(
        "stats-write",
        "write_file",
        JSON.stringify({
          file_path: "/workspace/stats-one.txt",
          content: "one\ntwo",
        }),
      ),
      toolOutput("stats-write", "write_file"),
      toolCall(
        "stats-edit",
        "edit_file",
        JSON.stringify({
          file_path: "/workspace/stats-two.txt",
          old_text: "old",
          new_text: "new\nextra",
        }),
      ),
      toolOutput("stats-edit", "edit_file"),
    ]);

    expect(totalChangeStats(changes)).toEqual({
      files: 2,
      additions: 4,
      deletions: 1,
    });
    expect(totalChangeStats([])).toEqual({
      files: 0,
      additions: 0,
      deletions: 0,
    });
  });
});
