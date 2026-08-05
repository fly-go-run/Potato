import { afterEach, describe, expect, it } from "vitest";
import {
  formatDuration,
  getMessageTiming,
  resetMessageTimings,
  trackMessageTimings,
} from "./messageTiming";
import {
  initialConversationStreamState,
  type ConversationStreamState,
  type StreamMessage,
} from "./stream";

function message(
  id: string,
  status: StreamMessage["status"],
): StreamMessage {
  return {
    id,
    type: "message",
    role: "assistant",
    status,
    content: [],
    metadata: null,
  };
}

function state(messages: StreamMessage[]): ConversationStreamState {
  return { ...initialConversationStreamState, messages };
}

afterEach(resetMessageTimings);

describe("message timings", () => {
  it.each(["created", "in_progress"] as const)(
    "starts timing when a message first appears as %s",
    (status) => {
      trackMessageTimings(state([]), state([message("message-1", status)]), 100);

      expect(getMessageTiming("message-1")).toEqual({
        startedAt: 100,
        endedAt: null,
      });
    },
  );

  it.each(["completed", "failed", "cancelled"] as const)(
    "ends timing when a message becomes %s",
    (status) => {
      const running = message("message-1", "in_progress");
      trackMessageTimings(state([]), state([running]), 100);
      trackMessageTimings(
        state([running]),
        state([{ ...running, status }]),
        350,
      );

      expect(getMessageTiming("message-1")).toEqual({
        startedAt: 100,
        endedAt: 350,
      });
    },
  );

  it.each(["completed", "failed", "cancelled"] as const)(
    "does not time a message first seen as %s",
    (status) => {
      trackMessageTimings(state([]), state([message("history", status)]), 100);

      expect(getMessageTiming("history")).toBeNull();
    },
  );

  it("skips a message whose reference has not changed", () => {
    const shared = message("message-1", "in_progress");
    trackMessageTimings(state([]), state([shared]), 100);
    shared.status = "completed";

    trackMessageTimings(state([shared]), state([shared]), 350);

    expect(getMessageTiming("message-1")).toEqual({
      startedAt: 100,
      endedAt: null,
    });
  });

  it("clears all recorded timings", () => {
    trackMessageTimings(
      state([]),
      state([message("one", "created"), message("two", "in_progress")]),
      100,
    );

    resetMessageTimings();

    expect(getMessageTiming("one")).toBeNull();
    expect(getMessageTiming("two")).toBeNull();
  });
});

describe("formatDuration", () => {
  it.each([
    [0, "0.1s"],
    [49, "0.1s"],
    [3_249, "3.2s"],
    [9_999, "10.0s"],
    [10_000, "10s"],
    [12_999, "12s"],
    [59_999, "59s"],
    [60_000, "1m 00s"],
    [65_999, "1m 05s"],
    [3_599_999, "59m 59s"],
    [3_600_000, "1h 00m"],
    [3_720_000, "1h 02m"],
  ])("formats %d milliseconds as %s", (ms, expected) => {
    expect(formatDuration(ms)).toBe(expected);
  });

  it.each([-1, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
    "returns an empty string for invalid duration %s",
    (ms) => {
      expect(formatDuration(ms)).toBe("");
    },
  );
});
