import { describe, expect, it, vi } from "vitest";
import {
  mergeCatalogInstalled,
  pollHubInstall,
  runOptimisticSkillToggle,
  type HubInstallTask,
  type SkillInfo,
} from "./capabilities";

const skill: SkillInfo = {
  name: "docx",
  description: "Create documents",
  enabled: false,
};

describe("runOptimisticSkillToggle", () => {
  it("updates immediately and keeps the new state after success", async () => {
    const updates: SkillInfo[][] = [];
    let current = [skill];
    await runOptimisticSkillToggle({
      skills: [skill],
      name: "docx",
      enabled: true,
      onUpdate: (update) => {
        current = update(current);
        updates.push(current);
      },
      mutate: vi.fn().mockResolvedValue({ enabled: true }),
    });
    expect(updates.map((items) => items[0].enabled)).toEqual([true]);
  });

  it("rolls back after a failed request", async () => {
    const updates: SkillInfo[][] = [];
    let current = [skill];
    await expect(
      runOptimisticSkillToggle({
        skills: [skill],
        name: "docx",
        enabled: true,
        onUpdate: (update) => {
          current = update(current);
          updates.push(current);
        },
        mutate: vi.fn().mockRejectedValue(new Error("reload failed")),
      }),
    ).rejects.toThrow("reload failed");
    expect(updates.map((items) => items[0].enabled)).toEqual([true, false]);
  });

  it("rolls back only its target when another toggle is still optimistic", async () => {
    const pdf: SkillInfo = {
      name: "pdf",
      description: "Read PDFs",
      enabled: false,
    };
    let current = [skill, pdf];
    let rejectDocx!: (reason: Error) => void;
    const update = (updater: (skills: SkillInfo[]) => SkillInfo[]) => {
      current = updater(current);
    };

    const docxToggle = runOptimisticSkillToggle({
      skills: current,
      name: "docx",
      enabled: true,
      onUpdate: update,
      mutate: () =>
        new Promise((_, reject) => {
          rejectDocx = reject;
        }),
    });
    const pdfToggle = runOptimisticSkillToggle({
      skills: current,
      name: "pdf",
      enabled: true,
      onUpdate: update,
      mutate: vi.fn().mockResolvedValue({ enabled: true }),
    });

    await pdfToggle;
    rejectDocx(new Error("docx failed"));
    await expect(docxToggle).rejects.toThrow("docx failed");
    expect(current.map(({ name, enabled }) => ({ name, enabled }))).toEqual([
      { name: "docx", enabled: false },
      { name: "pdf", enabled: true },
    ]);
  });
});

describe("pollHubInstall", () => {
  it("polls pending and importing states until completed", async () => {
    const states: HubInstallTask["status"][] = [
      "pending",
      "importing",
      "completed",
    ];
    const getStatus = vi.fn(async () => ({
      task_id: "task-1",
      bundle_url: "https://example.test/skill.zip",
      status: states.shift() ?? "completed",
    }));
    const result = await pollHubInstall(getStatus, "task-1", async () => {});
    expect(result.status).toBe("completed");
    expect(getStatus).toHaveBeenCalledTimes(3);
  });

  it("returns a failed terminal state", async () => {
    const failed: HubInstallTask = {
      task_id: "task-2",
      bundle_url: "https://example.test/skill.zip",
      status: "failed",
      error: "scan failed",
    };
    const result = await pollHubInstall(
      vi.fn().mockResolvedValue(failed),
      "task-2",
      async () => {},
    );
    expect(result).toEqual(failed);
  });
});

describe("mergeCatalogInstalled", () => {
  it("merges installed state by stable plugin id", () => {
    const result = mergeCatalogInstalled(
      [
        {
          id: "weather-2.0.0",
          plugin_id: "weather",
          name: "Weather",
          version: "2.0.0",
          install_url: "https://example.test/weather.zip",
          installed: false,
        },
      ],
      [{ id: "weather", name: "Weather", version: "1.5.0" }],
    );
    expect(result[0]).toMatchObject({
      installed: true,
      installed_version: "1.5.0",
    });
  });
});
