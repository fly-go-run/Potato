import { describe, expect, it } from "vitest";
import type { StreamMessage } from "../../lib/stream";
import { startsTrackWork } from "./MessageList";

function reasoning(id: string, text: string): StreamMessage {
  return {
    id,
    type: "reasoning",
    role: "assistant",
    content: text ? [{ type: "text", text }] : [],
  } as StreamMessage;
}

function toolCall(id: string, name: string): StreamMessage {
  return {
    id,
    type: "function_call",
    role: "assistant",
    content: [{ type: "data", data: { name, call_id: id } }],
  } as unknown as StreamMessage;
}

describe("startsTrackWork", () => {
  const noOutputs = new Map<string, StreamMessage>();

  it("folds preceding text when the model really keeps thinking", () => {
    expect(startsTrackWork(reasoning("r1", "让我查一下"), noOutputs)).toBe(
      true,
    );
  });

  it("ignores the empty reasoning the Responses API appends at turn end", () => {
    // agentscope 在 response.completed 时补的占位:没有思考文本,只带
    // reasoning_item_id。它排在最终正文之后,不能把答案折进轨道。
    expect(startsTrackWork(reasoning("r2", ""), noOutputs)).toBe(false);
    expect(startsTrackWork(reasoning("r3", "   "), noOutputs)).toBe(false);
  });

  it("still folds on ordinary tool calls", () => {
    expect(
      startsTrackWork(toolCall("c1", "execute_shell_command"), noOutputs),
    ).toBe(true);
  });

  it("does not fold on explicit file delivery", () => {
    expect(
      startsTrackWork(toolCall("c2", "send_file_to_user"), noOutputs),
    ).toBe(false);
  });
});
