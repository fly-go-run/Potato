import { describe, expect, it } from "vitest";
import { summarizeTrack, type TrackEntrySnapshot } from "./executionTrack";

const entry = (
  key: string,
  kind: TrackEntrySnapshot["kind"],
  overrides: Partial<TrackEntrySnapshot> = {},
): TrackEntrySnapshot => ({ key, kind, active: false, ...overrides });

describe("summarizeTrack", () => {
  it("gives waiting precedence", () => {
    // 首帧未到时轨道里可能已经有占位条目,但用户只关心「还在等模型」;
    // waiting 压过一切,否则等待期会先闪一句「思考中」。
    expect(summarizeTrack([], { streaming: true, waiting: true })).toEqual({
      kind: "waiting",
    });
    expect(
      summarizeTrack(
        [entry("t1", "tool", { active: true, toolName: "shell" })],
        {
          streaming: true,
          waiting: true,
        },
      ),
    ).toEqual({ kind: "waiting" });
  });

  it("describes the latest active tool", () => {
    // 并发/嵌套调用时可能同时有多个 active tool。摘要行只有一句话,
    // 取最后一个才对得上用户视线里刚出现的那张卡。
    const entries = [
      entry("old-tool", "tool", { active: true, toolName: "read_file" }),
      entry("new-tool", "tool", { active: true, toolName: "shell" }),
    ];
    expect(
      summarizeTrack(entries, { streaming: true, waiting: false }),
    ).toEqual({ kind: "runningTool", toolName: "shell" });
  });

  it("falls back to an empty tool name instead of dropping the state", () => {
    // 工具名可能只随 output 到达。名字还没解析出来时仍要报 runningTool
    // (渲染层自会给个泛化文案),不能退回 thinking——那是另一种语气。
    expect(
      summarizeTrack([entry("t1", "tool", { active: true })], {
        streaming: true,
        waiting: false,
      }),
    ).toEqual({ kind: "runningTool", toolName: "" });
  });

  it("prefers a running tool over active progress", () => {
    // 上下文压缩等 progress 可能与工具并行。工具是用户能理解的具体动作,
    // 优先说它;progress 只在没有工具在跑时才出面。
    const entries = [
      entry("compaction", "progress", { active: true }),
      entry("t1", "tool", { active: true, toolName: "shell" }),
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
    // reasoning 没有专门的状态:摘要行本来就是「思考中」,不必重复。
    expect(
      summarizeTrack([entry("reasoning", "reasoning", { active: true })], {
        streaming: true,
        waiting: false,
      }),
    ).toEqual({ kind: "thinking" });
  });

  it("stays in thinking state during streaming gaps", () => {
    // 这条是状态机存在的理由:一个工具已收口、下一个还没发出的间隙里,
    // 若返回 done,摘要行会在「已完成 1 步」与「正在…」之间来回摆。
    const entries = [entry("done", "tool", { completed: true })];
    expect(
      summarizeTrack(entries, { streaming: true, waiting: false }),
    ).toEqual({ kind: "thinking" });
  });

  it("returns the final step and failure counts only after streaming ends", () => {
    // 步数口径:叙述(message)是内容不是步骤,不计;思考、工具、进度都算。
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
    // 收口时轨道可能一条不剩(纯文字回答里的空 reasoning 已被时间线跳过)。
    // 「已完成 0 个步骤」读起来像出错了,下限保底为 1。
    expect(summarizeTrack([], { streaming: false, waiting: false })).toEqual({
      kind: "done",
      steps: 1,
      failed: 0,
    });
    expect(
      summarizeTrack([entry("narration", "message")], {
        streaming: false,
        waiting: false,
      }),
    ).toEqual({ kind: "done", steps: 1, failed: 0 });
  });
});
