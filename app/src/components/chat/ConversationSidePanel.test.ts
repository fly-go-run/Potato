import { describe, expect, it } from "vitest";
import type { RunStatus } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import {
  collectConversationArtifacts,
  presentRunStatus,
} from "./ConversationSidePanel";
import {
  isFailedToolState,
  isRunningToolState,
  isSuccessfulToolState,
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

describe("collectConversationArtifacts", () => {
  it("aggregates files written and delivered across turns", () => {
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
      expect.objectContaining({
        path: "/tmp/report.pdf",
        name: "report.pdf",
        sourceMessageId: "call-1",
      }),
    ]);
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
