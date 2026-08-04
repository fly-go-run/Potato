/**
 * 部分模型(如 Qwen 系)不走结构化 reasoning 通道,而是把思考过程用
 * `<think>…</think>` / `<thinking>…</thinking>` 内联在正文文本里。
 * 这里在展示层把思考段拆出来,正文只留干净文本;渲染层再把思考段
 * 并入执行轨道当思考条目。
 */

export interface InlineThinkingSplit {
  /** 抽出的思考文本(多段时以空行合并)。 */
  thinking: string;
  /** 剥离思考标签后的正文。 */
  text: string;
  /** 末尾停在未闭合的思考块里(流式输出进行中)。 */
  open: boolean;
  /** 内容与原文不同(含仅截掉末尾残缺开标签的情况)。 */
  changed: boolean;
}

const OPEN_TAG = /<think(?:ing)?>/;
const CLOSE_TAG = /<\/think(?:ing)?>/;
const FULL_OPEN_TAG = "<thinking>";

export function splitInlineThinking(raw: string): InlineThinkingSplit {
  let text = "";
  const thinkingParts: string[] = [];
  let rest = raw;
  let open = false;
  let heldPartial = 0;
  let sawTag = false;
  for (;;) {
    const openMatch = OPEN_TAG.exec(rest);
    if (openMatch) sawTag = true;
    if (!openMatch) {
      // 流式切片可能停在开标签中间("…<thin"),先扣住不渲染,
      // 等后续增量补齐后再归类,避免正文闪出半个标签。
      heldPartial = partialOpenTagLength(rest);
      text += heldPartial ? rest.slice(0, rest.length - heldPartial) : rest;
      break;
    }
    text += rest.slice(0, openMatch.index);
    rest = rest.slice(openMatch.index + openMatch[0].length);
    const closeMatch = CLOSE_TAG.exec(rest);
    if (!closeMatch) {
      if (rest.trim()) thinkingParts.push(rest.trim());
      open = true;
      break;
    }
    const inner = rest.slice(0, closeMatch.index).trim();
    if (inner) thinkingParts.push(inner);
    rest = rest.slice(closeMatch.index + closeMatch[0].length);
  }
  const thinking = thinkingParts.join("\n\n");
  // 无标签的普通文本原样返回,不做任何清理,保持展示层快速路径。
  const modified = sawTag || heldPartial > 0;
  return {
    thinking,
    text: modified ? text.replace(/^\s+/, "").replace(/\s+$/, "") : text,
    open,
    changed: modified,
  };
}

/** rest 的末尾是否是 "<thinking>"/"<think>" 的残缺前缀,返回残缺长度。 */
function partialOpenTagLength(value: string): number {
  const max = Math.min(value.length, FULL_OPEN_TAG.length - 1);
  for (let length = max; length > 0; length -= 1) {
    if (value.endsWith(FULL_OPEN_TAG.slice(0, length))) return length;
  }
  return 0;
}
