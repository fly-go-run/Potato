/**
 * 执行轨道摘要行的状态机(纯逻辑,UI 无关),供 MessageList 与测试共用。
 *
 * 摘要行是折叠段落之上唯一那句话:它要么描述「此刻正在做什么」,要么在
 * 轮次收口后给出「一共做了几步」。这条线只需要一个不回摆的口径:
 *   - streaming 期间绝不返回 done——工具与工具之间的间隙没有活动条目,
 *     若此时就报「已完成 N 步」,下一个工具一到又变回「正在…」,摘要行
 *     会在两种语气之间来回跳。
 *   - 流式空档:有 in-flight reasoning 才是 thinking;否则 waiting。
 *   - done 只在轮次真正结束时出现一次,那时全部条目已知,计数才算准。
 *
 * 折叠边界与槽位序列(哪些内容属于执行过程)不在这里,见 lib/turnTimeline.ts。
 */

export type TrackEntryKind = "reasoning" | "progress" | "message" | "tool";

export interface TrackEntrySnapshot {
  /** 消息 id / call id,跨帧稳定。 */
  key: string;
  kind: TrackEntryKind;
  /** 运行中(含 tool 已发出 call、尚未收到 output 的间隙)。 */
  active: boolean;
  /** tool/progress:已成功收口。 */
  completed?: boolean;
  /** tool:失败。 */
  failed?: boolean;
  /** tool:原始工具名。 */
  toolName?: string;
}

export type SummaryState =
  | { kind: "waiting" }
  | { kind: "runningTool"; toolName: string; running: number }
  | { kind: "progress" }
  | { kind: "thinking" }
  | { kind: "done"; steps: number; failed: number };

/**
 * 摘要状态机。streaming 期间绝不返回 done:没有活动条目的工具间隙
 * 归入 waiting(无 reasoning)或 thinking(有 in-flight reasoning),
 * 消除「正在… ↔ 已完成 N 步」的回摆。
 */
export function summarizeTrack(
  entries: TrackEntrySnapshot[],
  opts: { streaming: boolean; waiting: boolean },
): SummaryState {
  if (opts.waiting) return { kind: "waiting" };
  const runningTools = entries.filter(
    (entry) => entry.kind === "tool" && entry.active,
  );
  if (runningTools.length > 0) {
    const latest = runningTools[runningTools.length - 1]!;
    return {
      kind: "runningTool",
      toolName: latest.toolName ?? "",
      running: runningTools.length,
    };
  }
  if (entries.some((entry) => entry.kind === "progress" && entry.active)) {
    return { kind: "progress" };
  }
  if (opts.streaming) {
    const reasoningInFlight = entries.some(
      (entry) => entry.kind === "reasoning" && entry.active,
    );
    return reasoningInFlight ? { kind: "thinking" } : { kind: "waiting" };
  }
  const steps = entries.filter((entry) => entry.kind !== "message").length;
  const failed = entries.filter(
    (entry) => entry.kind === "tool" && entry.failed,
  ).length;
  return { kind: "done", steps: Math.max(1, steps), failed };
}
