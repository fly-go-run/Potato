import { formatDuration } from "./messageTiming";

/**
 * 历史轮次墙钟:上一则用户消息 → 本轮最后一条助手消息。
 * Date.parse 失败或 end<=start 时返回空串,且不把 ms<=0 送进 formatDuration。
 */
export function historyTurnDuration(
  startTimestamp: unknown,
  endTimestamp: unknown,
): string {
  if (typeof startTimestamp !== "string" || typeof endTimestamp !== "string") {
    return "";
  }
  const start = Date.parse(startTimestamp);
  const end = Date.parse(endTimestamp);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) {
    return "";
  }
  return formatDuration(end - start);
}
