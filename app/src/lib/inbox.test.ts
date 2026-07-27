import { describe, expect, it } from "vitest";
import {
  countUnread,
  eventRunId,
  markEventsRead,
  traceEventSummary,
  type InboxEvent,
} from "./inbox";

const events: InboxEvent[] = [
  {
    id: "a",
    agent_id: "default",
    source_type: "cron",
    source_id: "job",
    event_type: "cron_result",
    status: "success",
    severity: "info",
    title: "A",
    body: "",
    payload: { run_id: "run-1" },
    read: false,
    created_at: 1,
  },
  {
    id: "b",
    agent_id: "default",
    source_type: "task",
    source_id: "task",
    event_type: "task_result",
    status: "error",
    severity: "error",
    title: "B",
    body: "",
    payload: {},
    read: true,
    created_at: 2,
  },
];

describe("inbox unread logic", () => {
  it("counts and marks one event without mutating the rest", () => {
    expect(countUnread(events)).toBe(1);
    const next = markEventsRead(events, ["a"]);
    expect(countUnread(next)).toBe(0);
    expect(next[0].read).toBe(true);
    expect(next[1]).toBe(events[1]);
  });

  it("marks every event read for the all-read action", () => {
    expect(markEventsRead(events).every((event) => event.read)).toBe(true);
  });

  it("reads run_id only when it is a non-empty string", () => {
    expect(eventRunId(events[0])).toBe("run-1");
    expect(eventRunId(events[1])).toBeNull();
  });

  it("summarizes trace messages from their visible text content", () => {
    expect(
      traceEventSummary({
        role: "assistant",
        content: [{ type: "text", text: "Completed" }],
      }),
    ).toBe("assistant: Completed");
  });
});
