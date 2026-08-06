/**
 * 折叠执行窗口与摘要状态机的纯逻辑,UI 无关,供 MessageList 与测试共用。
 *
 * 契约:折叠态是容量固定的有界窗口——满员后老行会被换掉(不是
 * append-only)。因此 UI 层必须原位替换、不得让淘汰行继续占据文档流
 * 播收高动画,否则窗口高度会先涨后缩,底部吸附会把这个回摆投射成上方
 * 已定稿内容的抖动。同 key 的行只换状态、不重放入场动画。
 *
 * 换代受两条约束,都是为了「行要读得完」:
 *   1. 最短驻留:行进窗口后 MIN_ROW_DWELL_MS 内不得被顶掉。没有到期
 *      的空位可腾时,新行本帧不进——等下一次心跳重算,而不是把还没读
 *      完的行挤掉。
 *   2. 无内容不占位:文本还没流过来的叙述/详情还没解析出的当前步不
 *      参与选行。否则它占着槽位却渲染成空,视觉上就是「上一行凭空消
 *      失、下一行慢半拍才出现」。
 * 选行仍是纯函数:时间与上一帧结果都由调用方传入,便于测试。
 *
 * 摘要状态机在 streaming 期间只进不退,工具间隙归入 thinking,
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
  /**
   * 这一条现在有可展示的内容吗(叙述文本已到、当前步详情已解析)。
   * 省略视为 true。为 false 的条目不参与选行——占着槽位却渲染成空,
   * 就是「行凭空消失」的来源。
   */
  displayable?: boolean;
}

/** 摘要行之下的内容窗口容量:当前步、最新叙述与最近完成行共享。 */
export const COLLAPSED_WINDOW_CAPACITY = 3;

/**
 * 行进窗口后的最短驻留。心跳是每秒一次,所以实际驻留落在
 * [MIN_ROW_DWELL_MS, MIN_ROW_DWELL_MS + 1s) 区间,足够读完一行。
 */
export const MIN_ROW_DWELL_MS = 900;

export type CollapsedRowRole = "done" | "narration" | "current";

export interface CollapsedRow {
  key: string;
  role: CollapsedRowRole;
  /** 进入窗口的时刻,用于最短驻留判定;由调用方原样透传回来。 */
  since: number;
}

/**
 * 折叠态窗口选行。streaming=false 返回空数组(整轮收口,只剩摘要行)。
 * 槽位优先级:当前步(最后一个 active 的 tool/progress)> 最新叙述 >
 * 最近完成的 tool pair;返回顺序 = entries 原始顺序(即时间序)。
 * reasoning 不入窗:摘要行已表达「思考中」,再列一行标题就是复读。
 */
export function selectCollapsedWindow(
  entries: TrackEntrySnapshot[],
  opts: { streaming: boolean; now: number; prev?: CollapsedRow[] },
): CollapsedRow[] {
  if (!opts.streaming) return [];
  const { now } = opts;
  const wanted = rankCandidates(entries);
  const rankOf = new Map(wanted.map((row, index) => [row.key, index]));
  const roleOf = new Map(wanted.map((row) => [row.key, row.role]));
  const byKey = new Map(entries.map((entry) => [entry.key, entry]));

  // 上一帧的行留任(角色可能已从 current 变 done),其余按优先级补位。
  // 留任要重新体检:条目还在、仍有内容可展示,且要么还是候选、要么是
  // 叙述(叙述被更新的一条顶替后仍值得多留一会儿,这正是驻留的意义)。
  // 不体检的话,一个 active 工具失败后会既掉出候选、又顶着旧的
  // current 角色赖在窗口里,一直显示成「运行中」。
  const held: CollapsedRow[] = (opts.prev ?? [])
    .filter((row) => {
      const entry = byKey.get(row.key);
      if (!entry || entry.displayable === false) return false;
      return rankOf.has(row.key) || entry.kind === "message";
    })
    .map((row) => ({ ...row, role: roleOf.get(row.key) ?? row.role }))
    .slice(-COLLAPSED_WINDOW_CAPACITY);

  for (const candidate of wanted) {
    if (held.some((row) => row.key === candidate.key)) continue;
    if (held.length >= COLLAPSED_WINDOW_CAPACITY) {
      const victim = pickVictim(held, rankOf, rankOf.get(candidate.key)!, now);
      // 没有可腾的位置:让新行等下一次心跳,而不是挤掉还没读完的行。
      if (!victim) continue;
      held.splice(held.indexOf(victim), 1);
    }
    held.push({ ...candidate, since: now });
  }

  const order = new Map(entries.map((entry, index) => [entry.key, index]));
  return held.sort((a, b) => (order.get(a.key) ?? 0) - (order.get(b.key) ?? 0));
}

/** 想上窗的条目,按槽位优先级排:当前步 > 最新叙述 > 最近完成(新→旧)。 */
function rankCandidates(
  entries: TrackEntrySnapshot[],
): { key: string; role: CollapsedRowRole }[] {
  const shown = entries.filter((entry) => entry.displayable !== false);
  const wanted: { key: string; role: CollapsedRowRole }[] = [];
  const taken = new Set<string>();
  const claim = (key: string, role: CollapsedRowRole) => {
    if (taken.has(key)) return;
    taken.add(key);
    wanted.push({ key, role });
  };
  const current = findLast(
    shown,
    (entry) =>
      entry.active && (entry.kind === "tool" || entry.kind === "progress"),
  );
  if (current) claim(current.key, "current");
  const narration = findLast(shown, (entry) => entry.kind === "message");
  if (narration) claim(narration.key, "narration");
  for (let index = shown.length - 1; index >= 0; index -= 1) {
    const entry = shown[index]!;
    if (entry.kind === "tool" && entry.completed) claim(entry.key, "done");
  }
  return wanted;
}

/**
 * 选一个可以腾出来的行:必须比新行更不受欢迎(rank 更大,不在候选里
 * 记作无穷大),且已过最短驻留期。同时满足的取最不受欢迎的那个。
 */
function pickVictim(
  held: CollapsedRow[],
  rankOf: Map<string, number>,
  incomingRank: number,
  now: number,
): CollapsedRow | undefined {
  let victim: CollapsedRow | undefined;
  let victimRank = incomingRank;
  for (const row of held) {
    if (now - row.since < MIN_ROW_DWELL_MS) continue;
    const rank = rankOf.get(row.key) ?? Number.POSITIVE_INFINITY;
    if (rank > victimRank) {
      victim = row;
      victimRank = rank;
    }
  }
  return victim;
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
