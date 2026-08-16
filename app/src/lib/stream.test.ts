import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { MessageFrame, SseFrame } from "./protocol/types";
import {
  initialConversationStreamState,
  initialSseParserState,
  isUnfinishedResponse,
  isUnexpectedStreamEof,
  parseSseBytes,
  parseSseChunk,
  reduceStreamFrame,
  reduceStreamFrames,
  type ConversationStreamState,
} from "./stream";

function fixture(name: string) {
  return readFileSync(
    fileURLToPath(new URL(`../../fixtures/sse/${name}`, import.meta.url)),
  );
}

function parseFixture(name: string) {
  const parsed = parseSseBytes(fixture(name));
  expect(parsed.state.buffer).toBe("");
  expect(parsed.state.trailingBytes).toEqual([]);
  expect(parsed.state.errors).toEqual([]);
  return parsed.frames;
}

describe("SSE parser", () => {
  it("detects a clean EOF before a terminal response frame", () => {
    expect(isUnexpectedStreamEof("created", false)).toBe(true);
    expect(isUnexpectedStreamEof("in_progress", false)).toBe(true);
    expect(isUnexpectedStreamEof("completed", false)).toBe(false);
    expect(isUnexpectedStreamEof("failed", false)).toBe(false);
    expect(isUnexpectedStreamEof("cancelled", false)).toBe(false);
    expect(isUnexpectedStreamEof("in_progress", true)).toBe(false);
  });

  it("keeps an interrupted created or in-progress response guarded", () => {
    expect(isUnfinishedResponse("created")).toBe(true);
    expect(isUnfinishedResponse("in_progress")).toBe(true);
    expect(isUnfinishedResponse("idle")).toBe(false);
    expect(isUnfinishedResponse("completed")).toBe(false);
    expect(isUnfinishedResponse("failed")).toBe(false);
    expect(isUnfinishedResponse("cancelled")).toBe(false);
  });

  it("parses a frame split in the middle of JSON", () => {
    const first = parseSseChunk('data: {"type":"turn_');
    expect(first.frames).toEqual([]);
    const second = parseSseChunk(
      'usage","session_id":"s","usage":null,"context_usage":null}\n\n',
      first.state,
    );
    expect(second.frames).toHaveLength(1);
    expect(second.frames[0]).toMatchObject({ type: "turn_usage" });
  });

  it("survives arbitrary byte boundaries including multibyte text", () => {
    const input = fixture("tool-call.sse.txt");
    let parser = initialSseParserState;
    const frames: SseFrame[] = [];
    let cursor = 0;
    let seed = 0x51eeda;

    while (cursor < input.length) {
      seed = (seed * 1664525 + 1013904223) >>> 0;
      const length = (seed % 17) + 1;
      const parsed = parseSseBytes(
        input.subarray(cursor, cursor + length),
        parser,
      );
      frames.push(...parsed.frames);
      parser = parsed.state;
      cursor += length;
    }

    expect(parser.errors).toEqual([]);
    expect(frames).toEqual(parseFixture("tool-call.sse.txt"));
  });
});

describe("stream reducer", () => {
  it("reduces the simple text fixture to its final state", () => {
    const state = reduceStreamFrames(parseFixture("simple-text.sse.txt"));
    expect(state.responseStatus).toBe("completed");
    expect(state.error).toBeNull();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]?.content[0]).toMatchObject({
      type: "text",
      text: "你好，我是 Potato。",
      delta: false,
    });
    expect(state.turnUsage?.context_usage?.context_usage_ratio).toBeCloseTo(
      0.009558823529411765,
    );
  });

  it("joins tool arguments, replaces output, and keeps the summary", () => {
    const state = reduceStreamFrames(parseFixture("tool-call.sse.txt"));
    expect(state.responseStatus).toBe("completed");
    expect(state.messages.map((message) => message.type)).toEqual([
      "plugin_call",
      "plugin_call_output",
      "message",
    ]);
    expect(state.messages[0]?.content[0]).toMatchObject({
      type: "data",
      data: {
        call_id: "call_Skh97fTOjOxJBALqRqQAgoM2",
        name: "execute_shell_command",
        arguments: '{"command":"echo potato-fixture"}',
      },
    });
    expect(state.messages[1]?.content[0]).toMatchObject({
      type: "data",
      data: { output: "potato-fixture", state: "success" },
    });
    expect(state.messages[2]?.content[0]).toMatchObject({
      type: "text",
      text: "输出是 `potato-fixture`。",
    });
  });

  it("handles a failed response and top-level errors", () => {
    const failed = reduceStreamFrame(initialConversationStreamState, {
      object: "response",
      id: "response_failed",
      status: "failed",
      output: [],
      created_at: null,
      completed_at: null,
      metadata: null,
      error: { code: "model_error", message: "模型不可用" },
      sequence_number: 2,
    });
    expect(failed.responseStatus).toBe("failed");
    expect(failed.error).toBe("模型不可用");

    const topLevel = reduceStreamFrame(failed, { error: "连接中断" });
    expect(topLevel.error).toBe("连接中断");
  });

  it("clears history when message metadata requests it", () => {
    const withMessage: ConversationStreamState = {
      ...initialConversationStreamState,
      messages: [
        {
          id: "old",
          type: "message",
          role: "assistant",
          status: "completed",
          content: [],
          metadata: null,
        },
      ],
    };
    const clearFrame: MessageFrame = {
      object: "message",
      id: "clear",
      type: "message",
      role: "assistant",
      content: [],
      status: "completed",
      metadata: { clear_history: true },
      sequence_number: 8,
    };
    const state = reduceStreamFrame(withMessage, clearFrame);
    expect(state.messages).toEqual([]);
    expect(state.clearHistoryVersion).toBe(1);
  });

  it("records rate limits and marks the response failed", () => {
    const state = reduceStreamFrame(initialConversationStreamState, {
      type: "rate_limited",
      error: "请求过于频繁",
      alternatives: [
        {
          provider_id: "free",
          provider_name: "Free",
          model_id: "free-model",
          model_name: "Free Model",
        },
      ],
    });
    expect(state.responseStatus).toBe("failed");
    expect(state.error).toBe("请求过于频繁");
    expect(state.rateLimited?.alternatives).toHaveLength(1);
  });

  it("streams reasoning text through the same message lifecycle", () => {
    const state = reduceStreamFrames([
      {
        object: "message",
        id: "reasoning_1",
        type: "reasoning",
        role: "assistant",
        content: [],
        status: "in_progress",
        metadata: null,
        sequence_number: 1,
      },
      {
        object: "content",
        type: "text",
        text: "先分析",
        delta: true,
        index: 0,
        status: null,
        msg_id: "reasoning_1",
        sequence_number: 2,
      },
      {
        object: "content",
        type: "text",
        text: "先分析，再回答。",
        delta: false,
        index: 0,
        status: null,
        msg_id: "reasoning_1",
        sequence_number: 3,
      },
    ]);
    expect(state.messages[0]).toMatchObject({
      type: "reasoning",
      status: "in_progress",
      content: [{ type: "text", text: "先分析，再回答。" }],
    });
  });

  it("keeps context compaction as one quiet progress message", () => {
    const state = reduceStreamFrames([
      {
        object: "message",
        id: "compact_1",
        type: "progress",
        role: "assistant",
        content: [],
        status: "in_progress",
        metadata: { kind: "context_compaction", phase: "in_progress" },
        sequence_number: 1,
      },
      {
        object: "content",
        type: "data",
        data: { status: "in_progress" },
        delta: false,
        index: 0,
        status: "in_progress",
        msg_id: "compact_1",
        sequence_number: 2,
      },
      {
        object: "message",
        id: "compact_1",
        type: "progress",
        role: "assistant",
        content: [
          {
            object: "content",
            type: "data",
            data: { status: "completed" },
            delta: false,
            index: 0,
            status: "completed",
            msg_id: null,
          },
        ],
        status: "completed",
        metadata: { kind: "context_compaction", phase: "completed" },
        sequence_number: 3,
      },
    ]);

    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toMatchObject({
      id: "compact_1",
      type: "progress",
      status: "completed",
      metadata: { kind: "context_compaction", phase: "completed" },
      content: [{ type: "data", data: { status: "completed" } }],
    });
  });

  it("does not apply a clear-history output twice during final correction", () => {
    const clearFrame: MessageFrame = {
      object: "message",
      id: "clear_once",
      type: "message",
      role: "assistant",
      content: [],
      status: "completed",
      metadata: { clear_history: true },
      sequence_number: 3,
    };
    const cleared = reduceStreamFrame(
      {
        ...initialConversationStreamState,
        messages: [
          {
            id: "old",
            type: "message",
            role: "user",
            status: "completed",
            content: [],
            metadata: null,
          },
        ],
      },
      clearFrame,
    );
    const corrected = reduceStreamFrame(cleared, {
      object: "response",
      id: "response_clear",
      status: "completed",
      output: [clearFrame],
      created_at: null,
      completed_at: null,
      metadata: null,
      sequence_number: 4,
    });
    expect(corrected.messages).toEqual([]);
    expect(corrected.clearHistoryVersion).toBe(1);
  });

  it("preserves optional answer phase on message metadata", () => {
    const state = reduceStreamFrame(initialConversationStreamState, {
      object: "message",
      id: "msg_phase",
      type: "message",
      role: "assistant",
      content: [],
      status: "in_progress",
      metadata: { phase: "commentary" },
      sequence_number: 1,
    });
    expect(state.messages[0]?.metadata).toEqual({ phase: "commentary" });
  });

  it("ignores duplicate or out-of-order sequence numbers", () => {
    const frames = parseFixture("simple-text.sse.txt");
    const current = reduceStreamFrames(frames.slice(0, 5));
    const stale = reduceStreamFrame(current, frames[2]!);
    expect(stale).toBe(current);
    expect(stale.lastSequenceNumber).toBe(5);
  });
});
