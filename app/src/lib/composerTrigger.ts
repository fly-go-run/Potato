/* Composer 的 `/` 技能、`@` 文件引用触发检测。
 * 触发条件:符号位于文本开头或空白之后,且光标停在该 token 内(符号与光标
 * 之间无空白)。query 为符号到光标之间的文本,用于过滤候选。 */

export type TriggerKind = "slash" | "at";

export interface ComposerTrigger {
  kind: TriggerKind;
  /** 符号在文本中的下标 */
  start: number;
  /** 符号到光标之间的过滤词 */
  query: string;
}

export function detectTrigger(
  text: string,
  caret: number,
): ComposerTrigger | null {
  for (let i = caret - 1; i >= 0; i -= 1) {
    const ch = text[i];
    if (ch === "/" || ch === "@") {
      if (i > 0 && !/\s/.test(text[i - 1])) return null;
      return {
        kind: ch === "/" ? "slash" : "at",
        start: i,
        query: text.slice(i + 1, caret),
      };
    }
    if (/\s/.test(ch)) return null;
  }
  return null;
}

/** 用选中项替换触发 token,返回新文本与新光标位置(尾随一个空格便于续写) */
export function applyTrigger(
  text: string,
  caret: number,
  trigger: ComposerTrigger,
  value: string,
): { text: string; caret: number } {
  const symbol = trigger.kind === "slash" ? "/" : "@";
  const inserted = `${symbol}${value} `;
  const next = text.slice(0, trigger.start) + inserted + text.slice(caret);
  return { text: next, caret: trigger.start + inserted.length };
}
