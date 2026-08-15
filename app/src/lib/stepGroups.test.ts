import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { buildToolPair } from "../components/chat/ToolCard";
import type { RunStatus } from "./protocol/types";
import type { StreamMessage } from "./stream";
import {
  extractPairObject,
  focusFoldRowKey,
  materializeRun,
  toolFamily,
  windowFoldRows,
  type FoldRow,
  type ProcessEntry,
} from "./stepGroups";

const fixture = JSON.parse(
  readFileSync(
    join(
      dirname(fileURLToPath(import.meta.url)),
      "../../fixtures/http/chat-history-tool-call.json",
    ),
    "utf8",
  ),
) as {
  messages: StreamMessage[];
};

type ToolMessageType =
  | "function_call"
  | "function_call_output"
  | "plugin_call"
  | "plugin_call_output";

function toolMessage(
  id: string,
  type: ToolMessageType,
  data: Record<string, unknown>,
  status: RunStatus = "completed",
): StreamMessage {
  const isCall = type === "function_call" || type === "plugin_call";
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

function successPair(
  id: string,
  name: string,
  args: Record<string, unknown> | string = {},
): Extract<ProcessEntry, { kind: "pair" }> {
  const argumentsValue = typeof args === "string" ? args : JSON.stringify(args);
  const call = toolMessage(id, "function_call", {
    call_id: id,
    name,
    arguments: argumentsValue,
  });
  const output = toolMessage(`${id}-out`, "function_call_output", {
    call_id: id,
    name,
    output: "ok",
    state: "success",
  });
  return { kind: "pair", key: id, pair: buildToolPair(call, output) };
}

function runningPair(
  id: string,
  name: string,
  args: Record<string, unknown> = {},
): Extract<ProcessEntry, { kind: "pair" }> {
  const call = toolMessage(
    id,
    "function_call",
    { call_id: id, name, arguments: JSON.stringify(args) },
    "in_progress",
  );
  return { kind: "pair", key: id, pair: buildToolPair(call, null) };
}

function failedPair(
  id: string,
  name: string,
  args: Record<string, unknown> = {},
): Extract<ProcessEntry, { kind: "pair" }> {
  const call = toolMessage(id, "function_call", {
    call_id: id,
    name,
    arguments: JSON.stringify(args),
  });
  const output = toolMessage(
    `${id}-out`,
    "function_call_output",
    { call_id: id, name, output: "no", state: "error" },
    "failed",
  );
  return { kind: "pair", key: id, pair: buildToolPair(call, output) };
}

function unnamedPair(id: string): Extract<ProcessEntry, { kind: "pair" }> {
  const call = toolMessage(id, "function_call", {
    call_id: id,
    arguments: "{}",
  });
  const output = toolMessage(`${id}-out`, "function_call_output", {
    call_id: id,
    output: "ok",
    state: "success",
  });
  return { kind: "pair", key: id, pair: buildToolPair(call, output) };
}

function reasoning(
  id: string,
  text: string,
  status: RunStatus = "completed",
): ProcessEntry {
  return {
    kind: "reasoning",
    key: id,
    message: {
      id,
      type: "reasoning",
      role: "assistant",
      status,
      metadata: null,
      content: text
        ? [
            {
              object: "content",
              type: "text",
              delta: false,
              index: 0,
              status,
              msg_id: id,
              text,
            },
          ]
        : [],
    },
  };
}

function progress(id: string): ProcessEntry {
  return {
    kind: "progress",
    key: id,
    message: {
      id,
      type: "progress",
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
          text: "done",
        },
      ],
    },
  };
}

function foldRowsOf(entries: ProcessEntry[]): FoldRow[] {
  return materializeRun(entries)
    .filter(
      (item): item is Extract<ReturnType<typeof materializeRun>[number], { kind: "fold" }> =>
        item.kind === "fold",
    )
    .map((item) => item.row);
}

describe("toolFamily", () => {
  it("maps known tools and leaves the rest as other", () => {
    expect(toolFamily("web_search")).toBe("search");
    expect(toolFamily("web_fetch")).toBe("fetch");
    expect(toolFamily("grep_search")).toBe("grep");
    expect(toolFamily("glob_search")).toBe("glob");
    expect(toolFamily("read_file")).toBe("read");
    expect(toolFamily("write_file")).toBe("edit");
    expect(toolFamily("edit_file")).toBe("edit");
    expect(toolFamily("append_file")).toBe("edit");
    expect(toolFamily("execute_shell_command")).toBe("shell");
    expect(toolFamily("skill")).toBe("skill");
    expect(toolFamily("mcp__web_search")).toBe("search");
    expect(toolFamily("send_file_to_user")).toBe("other");
  });
});

describe("materializeRun merge", () => {
  it("merges consecutive successful same-family pairs and keeps the first key", () => {
    const items = materializeRun([
      successPair("s1", "web_search", { search_term: "alpha" }),
      successPair("s2", "web_search", { search_term: "beta" }),
    ]);
    expect(items).toHaveLength(1);
    const row = items[0]!;
    expect(row).toMatchObject({
      kind: "fold",
      row: {
        type: "group",
        key: "s1",
        family: "search",
        direct: false,
        object: "alpha",
        objectVaried: true,
      },
    });
    if (row.kind === "fold" && row.row.type === "group") {
      expect(row.row.pairs).toHaveLength(2);
    }
  });

  it("does not merge unnamed pairs, matching isWorkSlot", () => {
    const items = materializeRun([
      unnamedPair("u1"),
      successPair("s1", "web_search", { query: "q" }),
      unnamedPair("u2"),
    ]);
    expect(items.map((item) => item.kind)).toEqual(["fold", "fold", "fold"]);
    expect(
      items.map((item) =>
        item.kind === "fold" && item.row.type === "group"
          ? item.row.direct
          : null,
      ),
    ).toEqual([true, true, true]);
  });

  it("lifts failed pairs to visible and does not merge them", () => {
    const items = materializeRun([
      successPair("s1", "web_search", { search_term: "ok" }),
      failedPair("f1", "web_search", { search_term: "no" }),
      successPair("s2", "web_search", { search_term: "ok" }),
    ]);
    expect(items.map((item) => item.kind)).toEqual([
      "fold",
      "visible-failed",
      "fold",
    ]);
    expect(items[1]).toMatchObject({ kind: "visible-failed", key: "f1" });
  });

  it("merges skills only when the skill name matches", () => {
    const same = materializeRun([
      successPair("a", "skill", { name: "docx" }),
      successPair("b", "skill", { skill: "docx" }),
    ]);
    expect(same).toHaveLength(1);
    if (same[0]?.kind === "fold" && same[0].row.type === "group") {
      expect(same[0].row.pairs).toHaveLength(2);
      expect(same[0].row.skillName).toBe("docx");
    }

    const different = materializeRun([
      successPair("a", "skill", { name: "docx" }),
      successPair("b", "skill", { name: "pdf" }),
    ]);
    expect(different).toHaveLength(2);
  });

  it("does not merge a completed pair with an in-flight one", () => {
    const items = materializeRun([
      successPair("s1", "web_search", { search_term: "done" }),
      runningPair("s2", "web_search", { search_term: "now" }),
    ]);
    expect(items).toHaveLength(2);
    expect(
      items.map((item) =>
        item.kind === "fold" && item.row.type === "group"
          ? item.row.direct
          : false,
      ),
    ).toEqual([true, true]);
  });

  it("emits n=1 other as a direct card and n>1 other as one group", () => {
    const single = materializeRun([
      successPair("o1", "send_file_to_user", { file_path: "a.txt" }),
    ]);
    expect(single[0]).toMatchObject({
      kind: "fold",
      row: { family: "other", direct: true, name: "send_file_to_user" },
    });

    const many = materializeRun([
      successPair("o1", "custom_tool", {}),
      successPair("o2", "custom_tool", {}),
    ]);
    expect(many).toHaveLength(1);
    expect(many[0]).toMatchObject({
      kind: "fold",
      row: { family: "other", direct: false, name: "custom_tool" },
    });
  });

  it("merges consecutive reasoning with a blank line and the first message id", () => {
    const items = materializeRun([
      reasoning("r1", "first thought"),
      reasoning("r2", "second thought"),
    ]);
    expect(items).toHaveLength(1);
    expect(items[0]).toEqual({
      kind: "fold",
      row: {
        type: "thinking",
        key: "r1",
        messages: [
          expect.objectContaining({ id: "r1" }),
          expect.objectContaining({ id: "r2" }),
        ],
        text: "first thought\n\nsecond thought",
      },
    });
  });

  it("keeps an in-flight empty reasoning as a fold-row", () => {
    const items = materializeRun([reasoning("r1", "", "in_progress")]);
    expect(items).toEqual([
      {
        kind: "fold",
        row: {
          type: "thinking",
          key: "r1",
          messages: [expect.objectContaining({ id: "r1" })],
          text: "",
        },
      },
    ]);
  });

  it("emits completed progress as a direct fold-row", () => {
    const items = materializeRun([progress("p1")]);
    expect(items[0]).toMatchObject({
      kind: "fold",
      row: { type: "progress", key: "p1" },
    });
  });
});

describe("extractPairObject", () => {
  it("reads family-specific keys and ignores grep path", () => {
    expect(
      extractPairObject(
        "search",
        successPair("s", "web_search", { search_term: "term", query: "q" })
          .pair,
      ),
    ).toBe("term");
    expect(
      extractPairObject(
        "search",
        successPair("s", "web_search", { query: "only-query" }).pair,
      ),
    ).toBe("only-query");
    expect(
      extractPairObject(
        "fetch",
        successPair("f", "web_fetch", { url: "https://example.com/page" }).pair,
      ),
    ).toBe("https://example.com/page");
    expect(
      extractPairObject(
        "grep",
        successPair("g", "grep_search", {
          pattern: "TODO",
          path: "/workspace/src",
        }).pair,
      ),
    ).toBe("TODO");
    expect(
      extractPairObject(
        "read",
        successPair("r", "read_file", {
          file_path: "/workspace/src/lib/stepGroups.ts",
        }).pair,
      ),
    ).toBe("stepGroups.ts");
    expect(
      extractPairObject(
        "shell",
        successPair("sh", "execute_shell_command", {
          command: "echo history-fixture",
        }).pair,
      ),
    ).toBe("echo");
    expect(
      extractPairObject("other", successPair("o", "custom_tool", {}).pair),
    ).toBe("");
  });

  it("truncates search/fetch/grep objects to 32 characters", () => {
    const long = "abcdefghijklmnopqrstuvwxyz0123456789";
    expect(
      extractPairObject(
        "search",
        successPair("s", "web_search", { search_term: long }).pair,
      ),
    ).toBe(long.slice(0, 32));
  });

  it("returns an empty object when arguments are not JSON", () => {
    expect(
      extractPairObject(
        "search",
        successPair("s", "web_search", "not-json").pair,
      ),
    ).toBe("");
  });

  it("takes argv0 from the fixture shell command", () => {
    const call = fixture.messages.find(
      (message) => message.type === "plugin_call",
    );
    const output = fixture.messages.find(
      (message) => message.type === "plugin_call_output",
    );
    const pair = buildToolPair(call ?? null, output ?? null);
    expect(extractPairObject("shell", pair)).toBe("echo");
  });

  it("sums successful edit pairChangeStats onto the group", () => {
    const items = materializeRun([
      successPair("e1", "edit_file", {
        file_path: "/workspace/a.ts",
        old_text: "old\nline",
        new_text: "new\nline\nextra",
      }),
      successPair("e2", "edit_file", {
        file_path: "/workspace/b.ts",
        old_text: "x",
        new_text: "y\nz",
      }),
    ]);
    expect(items[0]).toMatchObject({
      kind: "fold",
      row: {
        type: "group",
        family: "edit",
        object: "a.ts",
        objectVaried: true,
      },
    });
    if (items[0]?.kind === "fold" && items[0].row.type === "group") {
      expect(items[0].row.additions).toBeGreaterThan(0);
      expect(items[0].row.deletions).toBeGreaterThan(0);
    }
  });
});

describe("focus and 8-row window", () => {
  const rows = Array.from({ length: 12 }, (_, index) => ({
    key: `row-${index + 1}`,
  }));

  it("settled over 8 shows the first 7 plus a non-zero overflow", () => {
    const window = windowFoldRows(rows, {
      settled: true,
      focusKey: null,
      overflowOpen: false,
    });
    expect(window.shownKeys).toEqual([
      "row-1",
      "row-2",
      "row-3",
      "row-4",
      "row-5",
      "row-6",
      "row-7",
    ]);
    expect(window.hiddenCount).toBe(5);
    expect(window.overflowAt).toBe("end");
  });

  it("live without focus keeps the latest 8 instead of cropping the tail", () => {
    const window = windowFoldRows(rows, {
      settled: false,
      focusKey: null,
      overflowOpen: false,
    });
    expect(window.shownKeys).toEqual([
      "row-5",
      "row-6",
      "row-7",
      "row-8",
      "row-9",
      "row-10",
      "row-11",
      "row-12",
    ]);
    expect(window.hiddenCount).toBe(4);
    expect(window.overflowAt).toBe("start");
  });

  it("live with focus pins that row as the last shown line", () => {
    const window = windowFoldRows(rows, {
      settled: false,
      focusKey: "row-9",
      overflowOpen: false,
    });
    expect(window.shownKeys.at(-1)).toBe("row-9");
    expect(window.shownKeys).toHaveLength(8);
    expect(window.shownKeys[0]).toBe("row-2");
    expect(window.hiddenCount).toBe(4);
    expect(window.overflowAt).toBe("start");
  });

  it("does not emit a zero-count overflow", () => {
    const window = windowFoldRows(rows.slice(0, 8), {
      settled: true,
      focusKey: null,
      overflowOpen: false,
    });
    expect(window.hiddenCount).toBe(0);
    expect(window.overflowAt).toBeNull();
  });

  it("opens the remaining rows when overflow is expanded", () => {
    const window = windowFoldRows(rows, {
      settled: true,
      focusKey: null,
      overflowOpen: true,
    });
    expect(window.shownKeys).toHaveLength(12);
    expect(window.hiddenCount).toBe(0);
    expect(window.overflowAt).toBeNull();
  });

  it("prefers the last active tool pair over in-flight thinking", () => {
    const rowsWithFocus = foldRowsOf([
      reasoning("r1", "thinking", "in_progress"),
      successPair("s1", "web_search", { search_term: "done" }),
      runningPair("s2", "web_search", { search_term: "now" }),
      reasoning("r2", "later", "in_progress"),
    ]);
    expect(focusFoldRowKey(rowsWithFocus, true)).toBe("s2");
    expect(focusFoldRowKey(rowsWithFocus, false)).toBeNull();
  });

  it("falls back to the last in-flight thinking row", () => {
    const rowsWithThinking = foldRowsOf([
      successPair("s1", "web_search", { search_term: "done" }),
      reasoning("r1", "one", "completed"),
      reasoning("r2", "", "in_progress"),
    ]);
    expect(focusFoldRowKey(rowsWithThinking, true)).toBe("r1");
  });
});
