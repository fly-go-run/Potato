/**
 * Codex-style phase title: the first closed **bold** phrase in
 * in-flight reasoning. An unclosed opener is ignored so a half-typed
 * `**Explor` does not flicker the TurnFlow header.
 */

const CLOSED_BOLD = /\*\*([^*\n]+)\*\*/;

export function extractFirstBold(text: string): string | null {
  const match = CLOSED_BOLD.exec(text);
  if (!match) return null;
  const title = match[1]!.trim();
  return title.length > 0 ? title : null;
}
