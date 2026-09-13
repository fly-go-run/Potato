import { describe, expect, it } from "vitest";
import {
  buildCronSpec,
  cronExpression,
  isCronJobEditable,
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
    expect(spec.request?.custom).toBe("kept");
    expect(spec.dispatch.mode).toBe("final");
    expect(promptFromSpec(spec)).toBe("Run checks");
  });
});

describe("legacy cron variants", () => {
  it("keeps once/text jobs out of the compact editor without dereferencing null", () => {
    const legacy = {
      id: "once-text",
      name: "One-off notice",
      enabled: true,
      schedule: { type: "once" as const, run_at: "2026-07-28T08:00:00Z" },
      task_type: "text",
      request: null,
      dispatch: {
        type: "channel" as const,
        channel: "console",
        target: { user_id: "default", session_id: "session-1" },
      },
    };

    expect(promptFromSpec(legacy)).toBe("");
    expect(cronExpression(legacy)).toBeNull();
    expect(isCronJobEditable(legacy)).toBe(false);
  });
});

describe("multipart task editing", () => {
  const form = { name: "Report", cron: "0 9 * * *", prompt: "Report", targetKey: targetKey(target) };
  it("shows every text block and preserves original input when only the name changes", () => {
    const spec = buildCronSpec(form, target, "UTC");
    spec.request!.input = [{ role: "user", type: "message", content: [
      { type: "text", text: "Report" }, { type: "text", text: "Keep secrets private" },
    ] }];
    expect(isCronJobEditable(spec)).toBe(true);
    expect(promptFromSpec(spec)).toBe("Report\nKeep secrets private");
    const edited = buildCronSpec({ ...form, name: "Renamed", prompt: promptFromSpec(spec) }, target, "UTC", spec);
    expect(edited.request!.input).toEqual(spec.request!.input);
    const changed = buildCronSpec({ ...form, prompt: "New report\nKeep secrets private" }, target, "UTC", spec);
    expect(promptFromSpec(changed)).toBe("New report\nKeep secrets private");
  });
  it("does not offer a text-only editor for images or multiple messages", () => {
    const spec = buildCronSpec(form, target, "UTC");
    spec.request!.input = [{ role: "user", content: [{ type: "text", text: "Report" }, { type: "image", image_url: "image" }] }];
    expect(isCronJobEditable(spec)).toBe(false);
    spec.request!.input = [{ role: "user", content: "one" }, { role: "user", content: "two" }];
    expect(isCronJobEditable(spec)).toBe(false);
  });
});
