import type { TranslationKey } from "./i18n";

/** 相对时间描述符：由调用方交给 i18n 渲染，保持本模块与语言无关。 */
export interface RelativeTime {
  key: TranslationKey;
  params: { count: number };
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * 把 ISO 时间戳转成「刚刚 / N 分钟前 / N 小时前 / N 天前」。
 * 无值或无法解析时返回 null（调用方自行留白）；未来时间按「刚刚」处理，
 * 以容忍前后端时钟偏差。
 */
export function relativeTime(
  value: string | null | undefined,
  now: number = Date.now(),
): RelativeTime | null {
  if (!value) return null;
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return null;

  const elapsed = now - timestamp;
  if (elapsed < MINUTE) return { key: "time.justNow", params: { count: 0 } };
  if (elapsed < HOUR) {
    return {
      key: "time.minutesAgo",
      params: { count: Math.floor(elapsed / MINUTE) },
    };
  }
  if (elapsed < DAY) {
    return {
      key: "time.hoursAgo",
      params: { count: Math.floor(elapsed / HOUR) },
    };
  }
  return { key: "time.daysAgo", params: { count: Math.floor(elapsed / DAY) } };
}
