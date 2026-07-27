import { describe, expect, it } from "vitest";
import {
  buildCronSpec,
  promptFromSpec,
  targetKey,
  type CronDispatchTarget,
  type CronJobSpec,
} from "./crons";

const target: CronDispatchTarget = {
  channel: "console",
  user_id: "default",
  session_id: "session-1",
};

describe("cron form spec assembly", () => {
  it("assembles the compact form into a valid agent CronJobSpec", () => {
    const spec = buildCronSpec(
      {
        name: " Daily report ",
        cron: " 0 9 * * * ",
        prompt: " Summarize today ",
        targetKey: targetKey(target),
      },
      target,
      "Asia/Shanghai",
    );

    expect(spec).toEqual({
      name: "Daily report",
      enabled: true,
      schedule: {
        type: "cron",
        cron: "0 9 * * *",
        timezone: "Asia/Shanghai",
      },
      task_type: "agent",
      request: {
        input: [
          {
            role: "user",
            type: "message",
            content: [{ type: "text", text: "Summarize today" }],
          },
        ],
      },
      dispatch: {
        type: "channel",
        channel: "console",
        target: { user_id: "default", session_id: "session-1" },
      },
    });
    expect(promptFromSpec(spec)).toBe("Summarize today");
  });

  it("preserves hidden backend fields while editing exposed fields", () => {
    const existing: CronJobSpec = {
      id: "job-1",
      name: "Old",
      enabled: false,
      schedule: { type: "cron", cron: "0 * * * *", timezone: "UTC" },
      task_type: "agent",
      request: { input: [], custom: "kept" },
      dispatch: {
        type: "channel",
        channel: "console",
        target: { user_id: "old", session_id: "old" },
        mode: "final",
      },
      runtime: { timeout_seconds: 600 },
      meta: { owner: "qa" },
      save_result_to_inbox: true,
    };

    const spec = buildCronSpec(
      {
        name: "New",
        cron: "0 9 * * mon",
        prompt: "Run checks",
        targetKey: targetKey(target),
      },
      target,
      "Asia/Shanghai",
      existing,
    );

    expect(spec.id).toBe("job-1");
    expect(spec.enabled).toBe(false);
    expect(spec.runtime).toEqual({ timeout_seconds: 600 });
    expect(spec.meta).toEqual({ owner: "qa" });
    expect(spec.request.custom).toBe("kept");
    expect(spec.dispatch.mode).toBe("final");
    expect(promptFromSpec(spec)).toBe("Run checks");
  });
});
