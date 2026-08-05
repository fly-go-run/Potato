/**
 * 折叠执行窗口与摘要状态机的纯逻辑,UI 无关,供 MessageList 与测试共用。
 *
 * 契约(与并行 agent 收敛,2026-08-05):折叠态是 append-only 的有界
 * 窗口——行只增不减、同 key 原位换状态,淘汰由 UI 层按 key diff 播退出
 * 动画;摘要状态机在 streaming 期间只进不退,工具间隙归入 thinking,
 * done 只在轮次收口时出现一次。
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

/** 摘要行之下的内容窗口容量:当前步、最新叙述与最近完成行共享。 */
export const COLLAPSED_WINDOW_CAPACITY = 3;

export type CollapsedRowRole = "done" | "narration" | "current";

export interface CollapsedRow {
  key: string;
  role: CollapsedRowRole;
}

/**
 * 折叠态窗口选行。streaming=false 返回空数组(整轮收口,只剩摘要行)。
 * 槽位优先级:当前步(最后一个 active 的 tool/progress)> 最新叙述 >
 * 最近完成的 tool pair;返回顺序 = entries 原始顺序(即时间序)。
 * reasoning 不入窗:摘要行已表达「思考中」,再列一行标题就是复读。
 */
export function selectCollapsedWindow(
  entries: TrackEntrySnapshot[],
  opts: { streaming: boolean },
): CollapsedRow[] {
  if (!opts.streaming) return [];
  const roles = new Map<string, CollapsedRowRole>();
  let capacity = COLLAPSED_WINDOW_CAPACITY;
  const current = findLast(
    entries,
    (entry) =>
      entry.active && (entry.kind === "tool" || entry.kind === "progress"),
  );
  if (current) {
    roles.set(current.key, "current");
    capacity -= 1;
  }
  const narration = findLast(entries, (entry) => entry.kind === "message");
  if (narration && capacity > 0) {
    roles.set(narration.key, "narration");
    capacity -= 1;
  }
  for (let index = entries.length - 1; index >= 0 && capacity > 0; index -= 1) {
    const entry = entries[index]!;
    if (entry.kind !== "tool" || !entry.completed || roles.has(entry.key)) {
      continue;
    }
    roles.set(entry.key, "done");
    capacity -= 1;
  }
  const rows: CollapsedRow[] = [];
  for (const entry of entries) {
    const role = roles.get(entry.key);
    if (role) rows.push({ key: entry.key, role });
  }
  return rows;
}

export type SummaryState =
  | { kind: "waiting" }
  | { kind: "runningTool"; toolName: string }
  | { kind: "progress" }
  | { kind: "thinking" }
  | { kind: "done"; steps: number; failed: number };

/**
 * 摘要状态机。streaming 期间绝不返回 done:没有活动条目的工具间隙
 * 归入 thinking,消除「正在… ↔ 已完成 N 步」的回摆。
 */
export function summarizeTrack(
  entries: TrackEntrySnapshot[],
  opts: { streaming: boolean; waiting: boolean },
): SummaryState {
  if (opts.waiting) return { kind: "waiting" };
  const runningTool = findLast(
    entries,
    (entry) => entry.kind === "tool" && entry.active,
  );
  if (runningTool) {
    return { kind: "runningTool", toolName: runningTool.toolName ?? "" };
  }
  if (entries.some((entry) => entry.kind === "progress" && entry.active)) {
    return { kind: "progress" };
  }
  if (opts.streaming) return { kind: "thinking" };
  const steps = entries.filter((entry) => entry.kind !== "message").length;
  const failed = entries.filter(
    (entry) => entry.kind === "tool" && entry.failed,
  ).length;
  return { kind: "done", steps: Math.max(1, steps), failed };
}

/** 目标 lib 未含 ES2023,自实现 findLast。 */
function findLast<T>(
  items: T[],
  predicate: (item: T) => boolean,
): T | undefined {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    if (predicate(items[index]!)) return items[index];
  }
  return undefined;
}
