/**
 * Models often write `**气温：**约` or `**气温： **约`. GFM will not close
 * emphasis when `**` sits after a fullwidth colon or a space, so the
 * asterisks leak onto the page.
 */
const BROKEN_CJK_BOLD =
  /\*\*([^*\n]+?)([：:，,。；;！!？?])\s*\*\*/g;

export function repairMarkdown(text: string): string {
  return text.replace(BROKEN_CJK_BOLD, "**$1**$2");
}
