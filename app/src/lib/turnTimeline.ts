/**
 * 一轮助手回复的呈现承诺层(纯逻辑,UI 无关)。
 *
 * 解决的问题:旧实现每一帧都从消息日志重算「这段内容属于执行过程还是
 * 最终回答」,而这个分类依赖**未来的消息**(文字后面出现工具调用才算
 * 叙述)。于是流式期间同一段文字会先以正文形态落地,下一个工具调用一到
 * 又被抽进执行轨道、换容器换字号——内容在用户眼前"改变主意",这是整个
 * 界面「散装感」的第一根源。
 *
 * 这里的承诺模型:
 *   1. 时间线 append-only——条目只在末尾出现,已落地的条目不换位置、
 *      不换形态。工具对(call+output)共用一个槽位,output 到达只是原位
 *      填充,不新增条目。
 *   2. 叙述与答案在视觉上**完全同构**(渲染层保证同字号同容器),因此
 *      fold/answer 的角色摇摆在流式期间没有任何视觉后果;
 *   3. 角色唯一被消费的时刻是收口(流结束整轨折叠),那时全部信息已知,
 *      边界一次算准,不再变。
 *
 * 折叠边界规则(与旧 startsTrackWork 语义一致):一段文字属于最终回答,
 * 当且仅当它后面没有「实质工作」——非空思考,或已知名且非交付类
 * (send_file_to_user)的工具调用。进度消息(如上下文压缩)可能出现在
 * 答案之后,不算工作;交付文件属于结果的一部分,也不算。
 */

import { textFromContent } from "./content";
import type { StreamMessage } from "./stream";

export type TimelineKind = "reasoning" | "tool" | "narration" | "progress";

/**
 * fold   — 属于执行过程,收口时折进摘要之下。
 * answer — 最终回答,永远可见。
 * (「突出卡」——交付产物、失败进度——由渲染层在 fold 槽位上就地升级,
 *  不属于本层的职责:它不影响折叠边界,只影响渲染形态。)
 */
export type TimelineRole = "fold" | "answer";

export interface TimelineSlot {
  /** 稳定 key:工具对 = call(孤儿对 = output)消息 id;其余 = 消息 id。 */
  key: string;
  kind: TimelineKind;
  role: TimelineRole;
  /** 槽位的主消息:tool = call(孤儿对 = output),其余 = 消息本身。 */
  messageId: string;
  /** tool:配对的 output 消息 id(未到达 = undefined)。 */
  outputId?: string;
}

/** 从消息日志构建时间线。输入应已做过内联 thinking 拆分。 */
export function buildTimeline(messages: StreamMessage[]): TimelineSlot[] {
  // call_id → 首个 output(与旧 buildToolPair 配对口径一致:重复 output 忽略)。
  const outputByCall = new Map<string, StreamMessage>();
  // 有 call 认领的 call_id。两张表都先扫全量再落槽,配对结果就与消息到达
  // 顺序无关;认领按 call_id(而不是 output 消息 id)记,重复 output 才是
  // 真的「忽略」——否则第二条 output 会漏成孤儿槽,轨道里多出一张凭空的卡。
  const pairedCalls = new Set<string>();
  for (const message of messages) {
    if (isToolCallType(message.type)) {
      pairedCalls.add(stringField(dataOf(message), "call_id"));
      continue;
    }
    if (!isToolOutputType(message.type)) continue;
    const callId = stringField(dataOf(message), "call_id");
    if (!outputByCall.has(callId)) outputByCall.set(callId, message);
  }

  const slots: TimelineSlot[] = [];
  for (const message of messages) {
    if (message.role === "user") continue;
    if (message.type === "reasoning") {
      // Responses 协议每轮收口会补一个无文本的 reasoning 占位(只为回传
      // reasoning_item_id)。已收口且始终为空的思考没有信息量,不落地;
      // 仍在流式的空思考保留——文本可能马上到,先占住时间线位置。
      const empty = textFromContent(message.content).trim().length === 0;
      const inFlight =
        message.status === "created" || message.status === "in_progress";
      if (empty && !inFlight) continue;
      slots.push({
        key: message.id,
        kind: "reasoning",
        role: "fold",
        messageId: message.id,
      });
      continue;
    }
    if (message.type === "progress") {
      slots.push({
        key: message.id,
        kind: "progress",
        role: "fold",
        messageId: message.id,
      });
      continue;
    }
    if (isToolCallType(message.type)) {
      const callId = stringField(dataOf(message), "call_id");
      const output = outputByCall.get(callId);
      slots.push({
        key: message.id,
        kind: "tool",
        role: "fold",
        messageId: message.id,
        outputId: output?.id,
      });
      continue;
    }
    if (isToolOutputType(message.type)) {
      // 没等到 call 的孤儿 output(历史裁剪等),独立成槽。
      if (pairedCalls.has(stringField(dataOf(message), "call_id"))) continue;
      slots.push({
        key: message.id,
        kind: "tool",
        role: "fold",
        messageId: message.id,
        outputId: message.id,
      });
      continue;
    }
    if (message.role === "assistant" && message.content.length > 0) {
      slots.push({
        key: message.id,
        kind: "narration",
        role: "fold",
        messageId: message.id,
      });
    }
  }

  // 答案边界:从末尾回扫,实质工作出现之前的叙述都是答案。
  const byId = new Map(messages.map((message) => [message.id, message]));
  let sawWork = false;
  for (let index = slots.length - 1; index >= 0; index -= 1) {
    const slot = slots[index]!;
    if (slot.kind === "narration") {
      if (!sawWork) slot.role = "answer";
      continue;
    }
    if (isWorkSlot(slot, byId)) sawWork = true;
  }
  return slots;
}

/**
 * 「实质工作」判定,决定其前方叙述是否折叠。工具名可能只出现在 output
 * 上;名字仍未知(还在流式)时按非工作处理——此时不折叠是保守选择,
 * 与旧 startsTrackWork 的口径一致。
 */
function isWorkSlot(
  slot: TimelineSlot,
  byId: Map<string, StreamMessage>,
): boolean {
  if (slot.kind === "reasoning") {
    const message = byId.get(slot.messageId);
    return (
      !!message && textFromContent(message.content).trim().length > 0
    );
  }
  if (slot.kind !== "tool") return false;
  const name =
    stringField(dataOf(byId.get(slot.messageId) ?? null), "name") ||
    stringField(
      dataOf(slot.outputId ? byId.get(slot.outputId) ?? null : null),
      "name",
    );
  if (!name) return false;
  return name !== "send_file_to_user";
}

/* ── 协议小工具(与组件层的同名判定保持一致;为避免 lib → components
     的反向依赖,这里自带一份,改动时两处同步) ─────────────────── */

function isToolCallType(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call" ||
    type === "function_call" ||
    type === "mcp_tool_call"
  );
}

function isToolOutputType(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call_output" ||
    type === "function_call_output" ||
    type === "mcp_tool_call_output"
  );
}

function dataOf(message: StreamMessage | null): Record<string, unknown> {
  const block = message?.content.find((part) => part.type === "data");
  return block && "data" in block
    ? ((block.data ?? {}) as Record<string, unknown>)
    : {};
}

function stringField(data: Record<string, unknown>, key: string): string {
  const value = data[key];
  return typeof value === "string" ? value : "";
}
