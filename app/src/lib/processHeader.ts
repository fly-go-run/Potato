/** Settled and live turns share one rule: no header for short, quiet work. */
export const PROCESS_HEADER_MS = 60_000;

/**
 * Whether TurnFlow should draw the process header.
 *
 * Codex-style: a single search or a short turn has no "正在搜索 · 38s"
 * marquee. The tool row itself is the live signal. The header comes back
 * for long work, failures, or an overflowed track.
 */
export function shouldShowProcessHeader(opts: {
  elapsedMs: number | null;
  failed: number;
  toolFoldCount: number;
  foldWindow: number;
}): boolean {
  if (opts.failed > 0) return true;
  if (opts.toolFoldCount > opts.foldWindow) return true;
  return opts.elapsedMs !== null && opts.elapsedMs >= PROCESS_HEADER_MS;
}
