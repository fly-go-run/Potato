import { describe, expect, it } from "vitest";
import {
  COLLAPSED_WINDOW_CAPACITY,
  selectCollapsedWindow,
  summarizeTrack,
  type TrackEntrySnapshot,
} from "./executionTrack";

const entry = (
  key: string,
  kind: TrackEntrySnapshot["kind"],
  overrides: Partial<TrackEntrySnapshot> = {},
): TrackEntrySnapshot => ({ key, kind, active: false, ...overrides });

describe("selectCollapsedWindow", () => {
  it("uses a three-entry content window while streaming", () => {
    expect(COLLAPSED_WINDOW_CAPACITY).toBe(3);
    const entries = [
      entry("done-1", "tool", { completed: true }),
      entry("done-2", "tool", { completed: true }),
      entry("narration", "message"),
      entry("current", "tool", { active: true, toolName: "shell" }),
    ];

    expect(selectCollapsedWindow(entries, { streaming: true })).toEqual([
      { key: "done-2", role: "done" },
      { key: "narration", role: "narration" },
      { key: "current", role: "current" },
    ]);
  });

  it("keeps two recent completed tools when there is no narration", () => {
    const entries = [
      entry("done-1", "tool", { completed: true }),
      entry("done-2", "tool", { completed: true }),
      entry("done-3", "tool", { completed: true }),
      entry("current", "progress", { active: true }),
    ];

    expect(selectCollapsedWindow(entries, { streaming: true })).toEqual([
      { key: "done-2", role: "done" },
      { key: "done-3", role: "done" },
      { key: "current", role: "current" },
    ]);
  });

  it("keeps narration and recent completed tools during a tool gap", () => {
    const entries = [
      entry("done-1", "tool", { completed: true }),
      entry("done-2", "tool", { completed: true }),
      entry("narration", "message"),
    ];

    expect(selectCollapsedWindow(entries, { streaming: true })).toEqual([
      { key: "done-1", role: "done" },
      { key: "done-2", role: "done" },
      { key: "narration", role: "narration" },
    ]);
  });

  it("uses only the latest narration and last active tool or progress", () => {
    const entries = [
      entry("old-current", "tool", {
        active: true,
        toolName: "read_file",
      }),
      entry("old-narration", "message"),
      entry("done", "tool", { completed: true }),
      entry("latest-narration", "message"),
      entry("latest-current", "progress", { active: true }),
    ];

    expect(selectCollapsedWindow(entries, { streaming: true })).toEqual([
      { key: "done", role: "done" },
      { key: "latest-narration", role: "narration" },
      { key: "latest-current", role: "current" },
    ]);
  });

  it("excludes reasoning, completed progress, and failed tools", () => {
    const entries = [
      entry("reasoning", "reasoning", { active: true }),
      entry("progress", "progress", { completed: true }),
      entry("failed", "tool", { failed: true }),
      entry("done", "tool", { completed: true }),
    ];

    expect(selectCollapsedWindow(entries, { streaming: true })).toEqual([
      { key: "done", role: "done" },
    ]);
  });

  it("clears the content window when the turn ends", () => {
    const entries = [
      entry("done", "tool", { completed: true }),
      entry("narration", "message"),
    ];

    expect(selectCollapsedWindow(entries, { streaming: false })).toEqual([]);
  });
});

describe("summarizeTrack", () => {
  it("gives waiting precedence", () => {
    expect(summarizeTrack([], { streaming: true, waiting: true })).toEqual({
      kind: "waiting",
    });
  });

  it("describes the latest active tool", () => {
    const entries = [
      entry("old-tool", "tool", {
        active: true,
        toolName: "read_file",
      }),
      entry("new-tool", "tool", { active: true, toolName: "shell" }),
    ];
    expect(
      summarizeTrack(entries, { streaming: true, waiting: false }),
    ).toEqual({ kind: "runningTool", toolName: "shell" });
  });

  it("describes active progress and reasoning", () => {
    expect(
      summarizeTrack(
        [entry("progress", "progress", { active: true })],
        { streaming: true, waiting: false },
      ),
    ).toEqual({ kind: "progress" });
    expect(
      summarizeTrack(
        [entry("reasoning", "reasoning", { active: true })],
        { streaming: true, waiting: false },
      ),
    ).toEqual({ kind: "thinking" });
  });

  it("stays in thinking state during streaming gaps", () => {
    const entries = [entry("done", "tool", { completed: true })];
    expect(
      summarizeTrack(entries, { streaming: true, waiting: false }),
    ).toEqual({ kind: "thinking" });
  });

  it("returns the final step and failure counts only after streaming ends", () => {
    const entries = [
      entry("reasoning", "reasoning"),
      entry("done", "tool", { completed: true }),
      entry("failed", "tool", { failed: true }),
      entry("progress", "progress", { completed: true }),
      entry("narration", "message"),
    ];
    expect(
      summarizeTrack(entries, { streaming: false, waiting: false }),
    ).toEqual({ kind: "done", steps: 4, failed: 1 });
  });

  it("keeps the existing minimum one-step final summary", () => {
    expect(
      summarizeTrack([], { streaming: false, waiting: false }),
    ).toEqual({ kind: "done", steps: 1, failed: 0 });
  });
});
