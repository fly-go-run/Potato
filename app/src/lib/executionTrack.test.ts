import { describe, expect, it } from "vitest";
import {
  COLLAPSED_WINDOW_CAPACITY,
  MIN_ROW_DWELL_MS,
  selectCollapsedWindow,
  summarizeTrack,
  type CollapsedRow,
  type TrackEntrySnapshot,
} from "./executionTrack";

const entry = (
  key: string,
  kind: TrackEntrySnapshot["kind"],
  overrides: Partial<TrackEntrySnapshot> = {},
): TrackEntrySnapshot => ({ key, kind, active: false, ...overrides });

/** 单帧选行:不关心驻留时,从空窗口起算。 */
const select = (
  entries: TrackEntrySnapshot[],
  opts: { streaming: boolean; now?: number; prev?: CollapsedRow[] },
) =>
  selectCollapsedWindow(entries, {
    streaming: opts.streaming,
    now: opts.now ?? 0,
    prev: opts.prev,
  });

/** 断言只看 key/role,``since`` 是簿记细节。 */
const shape = (rows: CollapsedRow[]) =>
  rows.map(({ key, role }) => ({ key, role }));

describe("selectCollapsedWindow", () => {
  it("uses a three-entry content window while streaming", () => {
    expect(COLLAPSED_WINDOW_CAPACITY).toBe(3);
    const entries = [
      entry("done-1", "tool", { completed: true }),
      entry("done-2", "tool", { completed: true }),
      entry("narration", "message"),
      entry("current", "tool", { active: true, toolName: "shell" }),
    ];

    expect(shape(select(entries, { streaming: true }))).toEqual([
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

    expect(shape(select(entries, { streaming: true }))).toEqual([
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

    expect(shape(select(entries, { streaming: true }))).toEqual([
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

    expect(shape(select(entries, { streaming: true }))).toEqual([
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

    expect(shape(select(entries, { streaming: true }))).toEqual([
      { key: "done", role: "done" },
    ]);
  });

  it("clears the content window when the turn ends", () => {
    const entries = [
      entry("done", "tool", { completed: true }),
      entry("narration", "message"),
    ];

    expect(shape(select(entries, { streaming: false }))).toEqual([]);
  });

  it("never shrinks the window as a turn progresses", () => {
    // 行数一旦回缩,折叠窗口的高度就会回摆,而底部吸附会把这个回摆
    // 投射成上方已定稿内容的上下抖动。窗口只允许增长到容量上限。
    const entries: TrackEntrySnapshot[] = [];
    const counts: number[] = [];
    let prev: CollapsedRow[] = [];
    let clock = 0;
    const snapshot = () => {
      clock += 5_000; // 每步都远超驻留期,专门考察行数
      prev = select(
        entries.map((item) => ({ ...item })),
        {
          streaming: true,
          now: clock,
          prev,
        },
      );
      counts.push(prev.length);
    };

    entries.push(entry("m1", "message"));
    snapshot();
    for (const step of ["t1", "t2", "t3", "t4"]) {
      entries.push(
        entry(step, "tool", {
          active: true,
          toolName: "execute_shell_command",
        }),
      );
      snapshot();
      Object.assign(entries[entries.length - 1]!, {
        active: false,
        completed: true,
      });
      snapshot();
      entries.push(entry(`${step}-note`, "message"));
      snapshot();
    }

    expect(Math.max(...counts)).toBe(COLLAPSED_WINDOW_CAPACITY);
    counts.forEach((count, index) => {
      if (index === 0) return;
      expect(count).toBeGreaterThanOrEqual(counts[index - 1]!);
    });
  });

  it("holds a row for its dwell instead of letting the next one shove it out", () => {
    // 之前叙述只有一个槽位,新叙述一到旧的当帧消失——读不完。
    const first = [entry("说明A", "message")];
    const opened = select(first, { streaming: true, now: 1_000 });
    expect(shape(opened)).toEqual([{ key: "说明A", role: "narration" }]);

    const both = [entry("说明A", "message"), entry("说明B", "message")];
    // 说明B 紧接着到:说明A 还没站够,窗口没满就并存,不是替换。
    const together = select(both, {
      streaming: true,
      now: 1_100,
      prev: opened,
    });
    expect(together.map((row) => row.key)).toEqual(["说明A", "说明B"]);
  });

  it("defers a newcomer while every row is still inside its dwell", () => {
    const entries = [
      entry("done-1", "tool", { completed: true }),
      entry("说明", "message"),
      entry("current", "tool", { active: true, toolName: "shell" }),
    ];
    const full = select(entries, { streaming: true, now: 1_000 });
    expect(full).toHaveLength(COLLAPSED_WINDOW_CAPACITY);

    // 第二个工具紧跟着开跑:窗口已满,而三行都还没读完 → 本帧不进。
    const busy = [
      ...entries.map((item) => ({ ...item, active: false, completed: true })),
      entry("current-2", "tool", { active: true, toolName: "shell" }),
    ];
    const deferred = select(busy, { streaming: true, now: 1_200, prev: full });
    expect(deferred.map((row) => row.key)).not.toContain("current-2");
    expect(deferred).toHaveLength(COLLAPSED_WINDOW_CAPACITY);

    // 驻留期一过,最不受欢迎的那行让位,新行才进来。
    const settled = select(busy, {
      streaming: true,
      now: 1_000 + MIN_ROW_DWELL_MS + 1,
      prev: deferred,
    });
    expect(settled.map((row) => row.key)).toContain("current-2");
    expect(settled).toHaveLength(COLLAPSED_WINDOW_CAPACITY);
  });

  it("drops a held row once it stops being a candidate", () => {
    // 入窗的 active 工具失败后既不是 current 也不是 done。若靠留任
    // 沿用旧角色,它会一直顶着虚线图标显示成「运行中」。
    const running = [entry("t1", "tool", { active: true, toolName: "shell" })];
    const held = select(running, { streaming: true, now: 1_000 });
    expect(shape(held)).toEqual([{ key: "t1", role: "current" }]);

    const failed = [entry("t1", "tool", { failed: true })];
    expect(select(failed, { streaming: true, now: 1_100, prev: held })).toEqual(
      [],
    );
  });

  it("drops a held row that no longer has anything to show", () => {
    const shown = [entry("说明", "message")];
    const held = select(shown, { streaming: true, now: 1_000 });
    expect(held).toHaveLength(1);

    const blank = [entry("说明", "message", { displayable: false })];
    // 留任却渲染成空 = 又变回「槽位在、内容没了」的空行。
    expect(select(blank, { streaming: true, now: 1_100, prev: held })).toEqual(
      [],
    );
  });

  it("keeps entries with nothing to show out of the window", () => {
    // 文本还没流过来的叙述若占了槽位,渲染出来是空的——视觉上就是
    // 「上一行凭空消失、下一行慢半拍才出现」。
    const streaming = [
      entry("done", "tool", { completed: true }),
      entry("空叙述", "message", { displayable: false }),
    ];
    expect(shape(select(streaming, { streaming: true }))).toEqual([
      { key: "done", role: "done" },
    ]);

    const arrived = [
      entry("done", "tool", { completed: true }),
      entry("空叙述", "message"),
    ];
    expect(shape(select(arrived, { streaming: true }))).toEqual([
      { key: "done", role: "done" },
      { key: "空叙述", role: "narration" },
    ]);
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
      summarizeTrack([entry("progress", "progress", { active: true })], {
        streaming: true,
        waiting: false,
      }),
    ).toEqual({ kind: "progress" });
    expect(
      summarizeTrack([entry("reasoning", "reasoning", { active: true })], {
        streaming: true,
        waiting: false,
      }),
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
    expect(summarizeTrack([], { streaming: false, waiting: false })).toEqual({
      kind: "done",
      steps: 1,
      failed: 0,
    });
  });
});
