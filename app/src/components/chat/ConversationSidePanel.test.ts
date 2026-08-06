import { describe, expect, it } from "vitest";
import type { RunStatus } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import {
  collectConversationArtifacts,
  presentRunStatus,
  resolveConversationFileLink,
  shouldPresentArtifactPair,
} from "../../lib/conversationArtifacts";
import {
  buildToolPair,
  isFailedToolState,
  isRunningToolState,
  isSuccessfulToolState,
  toolPairStatus,
} from "./ToolCard";

function toolMessage(
  id: string,
  type: "function_call" | "function_call_output",
  data: Record<string, unknown>,
  status: RunStatus = "completed",
): StreamMessage {
  return {
    id,
    type,
    role: type === "function_call" ? "assistant" : "tool",
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

function assistantTextMessage(id: string, text: string): StreamMessage {
  return {
    id,
    type: "message",
    role: "assistant",
    status: "completed",
    metadata: null,
    content: [
      {
        object: "content",
        type: "text",
        delta: false,
        index: 0,
        status: "completed",
        msg_id: id,
        text,
      },
    ],
  };
}

describe("collectConversationArtifacts", () => {
  it("only collects files explicitly delivered across turns", () => {
    const messages = [
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/report.pdf" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 24 bytes",
        state: " SUCCESS ",
      }),
      toolMessage("call-2", "function_call", {
        call_id: "2",
        name: "send_file_to_user",
        arguments: JSON.stringify({ file_path: "/tmp/data.xlsx" }),
      }),
      toolMessage("out-2", "function_call_output", {
        call_id: "2",
        name: "send_file_to_user",
        output: "File sent successfully.",
        state: "completed",
      }),
    ];

    expect(collectConversationArtifacts(messages)).toEqual([
      expect.objectContaining({
        path: "/tmp/data.xlsx",
        name: "data.xlsx",
        sourceMessageId: "call-2",
      }),
    ]);
  });

  it("does not surface temporary helper files written during execution", () => {
    const messages = [
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/classify_pdfs.py" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 1700 bytes",
        state: "completed",
      }),
      assistantTextMessage("assistant-1", "检查完成，共找到 47 篇论文。"),
    ];

    expect(collectConversationArtifacts(messages)).toEqual([]);
    expect(
      shouldPresentArtifactPair(
        buildToolPair(messages[0]!, messages[1]!),
        collectConversationArtifacts(messages),
      ),
    ).toBe(false);
  });

  it("promotes a written file only when the assistant explicitly links it", () => {
    const written = [
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/paper_filter.py" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 1700 bytes",
        state: "completed",
      }),
    ];
    const messages = [
      ...written,
      assistantTextMessage(
        "assistant-1",
        "已生成 [论文过滤脚本](sandbox:/tmp/paper_filter.py)。",
      ),
    ];
    const artifacts = collectConversationArtifacts(messages);

    expect(artifacts).toEqual([
      expect.objectContaining({ path: "/tmp/paper_filter.py" }),
    ]);
    expect(
      shouldPresentArtifactPair(
        buildToolPair(written[0]!, written[1]!),
        artifacts,
      ),
    ).toBe(true);
  });

  it("keeps one task-level entry when the same path is delivered twice", () => {
    const messages = [
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/report.pdf" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 24 bytes",
        state: "completed",
      }),
      toolMessage("call-2", "function_call", {
        call_id: "2",
        name: "send_file_to_user",
        arguments: JSON.stringify({ file_path: "/tmp/report.pdf" }),
      }),
      toolMessage("out-2", "function_call_output", {
        call_id: "2",
        name: "send_file_to_user",
        output: "File sent successfully.",
        state: "completed",
      }),
    ];

    expect(collectConversationArtifacts(messages)).toHaveLength(1);
    expect(collectConversationArtifacts(messages)[0]?.sourceMessageId).toBe(
      "call-2",
    );
  });

  it("ignores read and edit operations", () => {
    const messages = [
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "read_file",
        arguments: JSON.stringify({ file_path: "/tmp/input.md" }),
      }),
      toolMessage("call-2", "function_call", {
        call_id: "2",
        name: "edit_file",
        arguments: JSON.stringify({ file_path: "/tmp/input.md" }),
      }),
    ];

    expect(collectConversationArtifacts(messages)).toEqual([]);
  });

  it("ignores artifact calls that are still running or have no output", () => {
    const messages = [
      toolMessage(
        "call-1",
        "function_call",
        {
          call_id: "1",
          name: "write_file",
          arguments: JSON.stringify({ file_path: "/tmp/running.md" }),
        },
        "in_progress",
      ),
      toolMessage("call-2", "function_call", {
        call_id: "2",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/no-output.md" }),
      }),
    ];

    expect(collectConversationArtifacts(messages)).toEqual([]);
  });

  it("ignores every non-success terminal or tool state", () => {
    const cases: Array<[string, RunStatus, string]> = [
      ["failed", "failed", "failed"],
      ["cancelled", "cancelled", "cancelled"],
      ["error-state", "completed", "error"],
      ["still-running", "completed", "in_progress"],
      ["denied", "completed", "denied"],
      ["interrupted", "completed", "interrupted"],
    ];
    const messages = cases.flatMap(([id, status, state], index) => [
      toolMessage(`call-${id}`, "function_call", {
        call_id: String(index),
        name: "write_file",
        arguments: JSON.stringify({ file_path: `/tmp/${id}.md` }),
      }),
      toolMessage(
        `out-${id}`,
        "function_call_output",
        {
          call_id: String(index),
          name: "write_file",
          output: "tool failed",
          state,
        },
        status,
      ),
    ]);

    expect(collectConversationArtifacts(messages)).toEqual([]);
  });

  it("collects files explicitly delivered in assistant sandbox links", () => {
    const artifacts = collectConversationArtifacts([
      assistantTextMessage(
        "assistant-1",
        "已生成 [下载目录文档清单.txt](<sandbox:/Users/me/下载目录文档清单.txt>)。",
      ),
    ]);

    expect(artifacts).toEqual([
      {
        id: "assistant-1:/Users/me/下载目录文档清单.txt",
        path: "/Users/me/下载目录文档清单.txt",
        name: "下载目录文档清单.txt",
        sourceMessageId: "assistant-1",
      },
    ]);
  });

  it("keeps the tool artifact when an assistant links the same file", () => {
    const artifacts = collectConversationArtifacts([
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "/tmp/report.txt" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 24 bytes",
        state: "completed",
      }),
      assistantTextMessage("assistant-1", "[下载](sandbox:/tmp/report.txt)"),
    ]);

    expect(artifacts).toHaveLength(1);
    expect(artifacts[0]?.id).toBe("call-1:/tmp/report.txt");
  });

  it("dedupes a linked file against its backslash tool path", () => {
    const artifacts = collectConversationArtifacts([
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "C:\\tmp\\报告.txt" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 24 bytes",
        state: "completed",
      }),
      assistantTextMessage("assistant-1", "[报告](file:///C:/tmp/报告.txt)"),
    ]);

    expect(artifacts).toHaveLength(1);
    expect(artifacts[0]?.id).toBe("call-1:C:\\tmp\\报告.txt");
  });

  it("dedupes across drive-letter case differences", () => {
    const artifacts = collectConversationArtifacts([
      toolMessage("call-1", "function_call", {
        call_id: "1",
        name: "write_file",
        arguments: JSON.stringify({ file_path: "C:\\tmp\\报告.txt" }),
      }),
      toolMessage("out-1", "function_call_output", {
        call_id: "1",
        name: "write_file",
        output: "Wrote 24 bytes",
        state: "completed",
      }),
      assistantTextMessage("assistant-1", "[报告](file:///c:/tmp/报告.txt)"),
    ]);

    expect(artifacts).toHaveLength(1);
    expect(artifacts[0]?.id).toBe("call-1:C:\\tmp\\报告.txt");
  });

  it("extracts links with balanced parens and escaped label brackets", () => {
    const artifacts = collectConversationArtifacts([
      assistantTextMessage(
        "assistant-1",
        "[报告 \\[最终\\]](sandbox:/tmp/report(final).pdf)",
      ),
    ]);

    expect(artifacts).toEqual([
      expect.objectContaining({
        path: "/tmp/report(final).pdf",
        name: "report(final).pdf",
      }),
    ]);
  });

  it("only collects assistant sandbox and file protocol links", () => {
    const userMessage = assistantTextMessage(
      "user-1",
      "[用户文件](sandbox:/tmp/user.txt)",
    );
    userMessage.role = "user";
    const artifacts = collectConversationArtifacts([
      assistantTextMessage(
        "assistant-1",
        "[网页](https://example.com/a.txt) [绝对路径](/Users/a/b.txt) [相对](b.txt) ![图片](sandbox:/a/b.png)",
      ),
      userMessage,
    ]);

    expect(artifacts).toEqual([]);
  });

  it("collects Windows paths from file protocol links", () => {
    expect(
      collectConversationArtifacts([
        assistantTextMessage(
          "assistant-1",
          "[报告](file:///C:/Users/me/报告.pdf)",
        ),
      ]),
    ).toEqual([
      expect.objectContaining({
        path: "C:/Users/me/报告.pdf",
        name: "报告.pdf",
        sourceMessageId: "assistant-1",
      }),
    ]);
  });
});

describe("resolveConversationFileLink", () => {
  const artifacts = [
    {
      id: "report",
      path: "/Users/example/Downloads/下载目录文档清单.txt",
      name: "下载目录文档清单.txt",
      sourceMessageId: "call-1",
    },
  ];

  it("maps friendly and absolute file links without capturing web links", () => {
    expect(resolveConversationFileLink("下载目录文档清单.txt", artifacts)).toBe(
      "/Users/example/Downloads/下载目录文档清单.txt",
    );
    expect(
      resolveConversationFileLink(
        "file:///Users/example/Downloads/下载目录文档清单.txt",
        artifacts,
      ),
    ).toBe("/Users/example/Downloads/下载目录文档清单.txt");
    expect(
      resolveConversationFileLink("https://example.com/report.txt", artifacts),
    ).toBeNull();
  });

  it("treats Windows drive-letter paths as local paths, not URL schemes", () => {
    expect(resolveConversationFileLink("C:\\Users\\me\\报告.pdf", [])).toBe(
      "C:/Users/me/报告.pdf",
    );
    expect(resolveConversationFileLink("c:/Users/me/report.pdf", [])).toBe(
      "c:/Users/me/report.pdf",
    );
    expect(
      resolveConversationFileLink("file:///C:/Users/me/report.pdf", []),
    ).toBe("C:/Users/me/report.pdf");
    expect(
      resolveConversationFileLink("sandbox:/C:/Users/me/report.pdf", []),
    ).toBe("C:/Users/me/report.pdf");
    expect(
      resolveConversationFileLink("mailto:someone@example.com", []),
    ).toBeNull();
  });

  it("resolves sandbox: links even without matching artifacts", () => {
    expect(
      resolveConversationFileLink(
        "sandbox:/Users/example/workspace/清单.txt",
        [],
      ),
    ).toBe("/Users/example/workspace/清单.txt");
    expect(
      resolveConversationFileLink(
        "sandbox:///Users/example/workspace/清单.txt",
        [],
      ),
    ).toBe("/Users/example/workspace/清单.txt");
  });
});

describe("presentRunStatus", () => {
  it.each([
    ["created", "chat.panel.running"],
    ["in_progress", "chat.panel.running"],
    ["completed", "chat.panel.completed"],
    ["idle", "chat.panel.completed"],
    ["failed", "chat.panel.failed"],
    ["cancelled", "chat.panel.cancelled"],
  ] as const)("maps %s without flattening terminal states", (status, label) => {
    expect(presentRunStatus(status).label).toBe(label);
  });
});

describe("tool result state presentation", () => {
  it("keeps a completed call visible while its output is still missing", () => {
    const call = toolMessage("call-pending", "function_call", {
      call_id: "pending",
      name: "execute_shell_command",
      arguments: JSON.stringify({ command: "printf pending" }),
    });

    expect(toolPairStatus(buildToolPair(call, null))).toMatchObject({
      running: true,
      completed: false,
      failed: false,
    });
  });

  it.each([null, "", "success", "completed", " SUCCESS "])(
    "accepts %s as successful",
    (state) => {
      expect(isSuccessfulToolState(state)).toBe(true);
      expect(isFailedToolState(state)).toBe(false);
    },
  );

  it.each(["created", "in_progress", " IN_PROGRESS "])(
    "keeps %s neutral and running",
    (state) => {
      expect(isRunningToolState(state)).toBe(true);
      expect(isFailedToolState(state)).toBe(false);
    },
  );

  it.each(["failed", "error", "cancelled", "denied", "interrupted"])(
    "renders %s as failed instead of a success check",
    (state) => {
      expect(isSuccessfulToolState(state)).toBe(false);
      expect(isRunningToolState(state)).toBe(false);
      expect(isFailedToolState(state)).toBe(true);
    },
  );
});
