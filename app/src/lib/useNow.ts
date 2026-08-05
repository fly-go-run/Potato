import { useEffect, useState } from "react";

/**
 * 每秒心跳的当前时间,驱动「思考中 · 12s」这类实时计时文案。
 * active 为 false 时冻结,不产生任何定时器。
 */
export function useNow(active: boolean, intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [active, intervalMs]);
  return now;
}
