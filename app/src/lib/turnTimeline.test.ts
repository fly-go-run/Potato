import { describe, expect, it } from "vitest";
import {
  buildTimeline,
  copyAnswerText,
  type TimelineSlot,
} from "./turnTimeline";
import type { RunStatus } from "./protocol/types";
import type { StreamMessage } from "./stream";

/* ── 造数:只填 buildTimeline 真正读到的字段(id/type/role/status/content),
     其余按 StreamMessage 形状断言掉,与 executionTrack/MessageList 的既有
     手法一致。 ─────────────────────────────────────────────────────── */

const text = (value: string) => ({ type: "text", text: value });
const data = (value: Record<string, unknown>) => ({
  type: "data",
  data: value,
});

function make(
  id: string,
  type: string,
  content: unknown[],
  overrides: { role?: string; status?: RunStatus } = {},
): StreamMessage {
  return {
    id,
    type,
    role: overrides.role ?? "assistant",
    status: overrides.status ?? "completed",
    content,
    metadata: null,
  } as unknown as StreamMessage;
}

const narration = (id: string, body = "正文") =>
  make(id, "message", [text(body)]);

const reasoning = (id: string, body: string, status: RunStatus = "completed") =>
  make(id, "reasoning", body ? [text(body)] : [], { status });

const toolCall = (id: string, callId: string, name?: string) =>
  make(id, "function_call", [
    data(name === undefined ? { call_id: callId } : { call_id: callId, name }),
  ]);

const toolOutput = (id: string, callId: string, name?: string) =>
  make(id, "function_call_output", [
    data(
      name === undefined
        ? { call_id: callId, output: "ok" }
        : { call_id: callId, name, output: "ok" },
    ),
  ]);

const progress = (id: string) => make(id, "progress", [text("正在压缩上下文")]);

const userText = (id: string) =>
  make(id, "message", [text("帮我看看")], { role: "user" });

/** 断言主要看「有哪些槽、什么类型、什么角色」。 */
const shape = (slots: TimelineSlot[]) =>
  slots.map(({ key, kind, role }) => ({ key, kind, role }));

const roleOf = (slots: TimelineSlot[], key: string) =>
  slots.find((slot) => slot.key === key)?.role;

describe("buildTimeline 工具配对", () => {
  it("pairs a call with its output into a single slot", () => {
    // 一个工具在时间线上只能占一个位置:output 到达是原位填充而不是追加,
    // 否则同一次调用会在用户眼前变成两条,append-only 的承诺就破了。
    const slots = buildTimeline([
      toolCall("c1", "call-1", "execute_shell_command"),
      toolOutput("o1", "call-1"),
    ]);
    expect(slots).toHaveLength(1);
    expect(slots[0]).toMatchObject({
      key: "c1",
      kind: "tool",
      messageId: "c1",
      outputId: "o1",
    });
  });

  it("leaves the pair open until its output arrives", () => {
    // 调用已发出、结果还没回来的间隙里槽位就得先占住,它是「正在运行」
    // 那张卡的位置;outputId 缺席只表示还没填完,不该另起一条。
    const slots = buildTimeline([toolCall("c1", "call-1", "read_file")]);
    expect(slots).toHaveLength(1);
    expect(slots[0]?.outputId).toBeUndefined();
  });

  it("ignores a repeated output for a call that is already paired", () => {
    // 同一 call_id 收到第二条 output(重连/重放)时只认第一条。若第二条
    // 漏成孤儿槽,轨道里就会凭空多出一张重复的工具卡。
    const slots = buildTimeline([
      toolCall("c1", "call-1", "execute_shell_command"),
      toolOutput("o1", "call-1"),
      toolOutput("o2", "call-1"),
    ]);
    expect(shape(slots)).toEqual([{ key: "c1", kind: "tool", role: "fold" }]);
    expect(slots[0]?.outputId).toBe("o1");
  });

  it("gives an orphan output its own slot", () => {
    // 历史裁剪后 call 可能已经不在消息里。丢掉 output 等于丢掉这一步的
    // 证据,所以它独立成槽(messageId 与 outputId 都指向自己)。
    const slots = buildTimeline([toolOutput("o9", "call-gone", "read_file")]);
    expect(slots).toHaveLength(1);
    expect(slots[0]).toMatchObject({
      key: "o9",
      kind: "tool",
      messageId: "o9",
      outputId: "o9",
    });
  });
});

describe("buildTimeline 空 reasoning", () => {
  it("skips an empty reasoning that has already settled", () => {
    // 走 Responses 协议的供应商每轮收口都补一个没有文本的 reasoning 占位
    // (只为回传 reasoning_item_id)。它没有信息量,落地就是一条空行。
    expect(buildTimeline([reasoning("r1", "", "completed")])).toEqual([]);
    expect(buildTimeline([reasoning("r2", "   ", "completed")])).toEqual([]);
  });

  it("keeps an empty reasoning that is still streaming", () => {
    // 思考刚开始、文本还没流过来:先占住时间线位置,文本到了原位长出来。
    // 若等有文本才落地,这条会插在后到的内容之前——那就是插队。
    const slots = buildTimeline([reasoning("r1", "", "in_progress")]);
    expect(shape(slots)).toEqual([
      { key: "r1", kind: "reasoning", role: "fold" },
    ]);
    expect(shape(buildTimeline([reasoning("r2", "", "created")]))).toEqual([
      { key: "r2", kind: "reasoning", role: "fold" },
    ]);
  });
});

describe("buildTimeline 答案边界", () => {
  it("treats trailing narration as the answer", () => {
    // 后面没有任何实质工作的文字就是最终回答:收口时它必须留在外面,
    // 不能随执行轨道一起折走。
    const slots = buildTimeline([
      toolCall("c1", "call-1", "execute_shell_command"),
      toolOutput("o1", "call-1"),
      narration("m1"),
      narration("m2"),
    ]);
    expect(shape(slots)).toEqual([
      { key: "c1", kind: "tool", role: "fold" },
      { key: "m1", kind: "narration", role: "answer" },
      { key: "m2", kind: "narration", role: "answer" },
    ]);
  });

  it("folds narration once real reasoning follows it", () => {
    // 「我先看看配置」后面又开始思考,说明这句只是过程旁白。
    const slots = buildTimeline([
      narration("m1"),
      reasoning("r1", "得先确认依赖版本"),
      narration("m2"),
    ]);
    expect(roleOf(slots, "m1")).toBe("fold");
    expect(roleOf(slots, "m2")).toBe("answer");
  });

  it("folds narration once a known tool follows it", () => {
    const slots = buildTimeline([
      narration("m1"),
      toolCall("c1", "call-1", "execute_shell_command"),
      toolOutput("o1", "call-1"),
      narration("m2"),
    ]);
    expect(roleOf(slots, "m1")).toBe("fold");
    expect(roleOf(slots, "m2")).toBe("answer");
  });

  it("reads the tool name off the output when the call has none yet", () => {
    // 工具名可能只随 output 回来。两处都要看,否则同一次调用会因为读名字
    // 的路径不同而分类劈叉。
    const slots = buildTimeline([
      narration("m1"),
      toolCall("c1", "call-1"),
      toolOutput("o1", "call-1", "execute_shell_command"),
    ]);
    expect(roleOf(slots, "m1")).toBe("fold");
  });

  it("does not fold on an empty reasoning placeholder", () => {
    // 占位思考排在最终正文之后。它若算工作,整段答案会被折进轨道再随
    // 收口一起藏掉——用户看到回答凭空消失。
    expect(
      roleOf(buildTimeline([narration("m1"), reasoning("r1", "")]), "m1"),
    ).toBe("answer");
    // 流式中的空思考同理:它落了槽,但仍不是工作。
    expect(
      roleOf(
        buildTimeline([narration("m1"), reasoning("r1", "", "in_progress")]),
        "m1",
      ),
    ).toBe("answer");
  });

  it("does not fold on file delivery", () => {
    // send_file_to_user 是把结果交给用户,属于回答的一部分而不是过程。
    const slots = buildTimeline([
      narration("m1"),
      toolCall("c1", "call-1", "send_file_to_user"),
      toolOutput("o1", "call-1"),
    ]);
    expect(roleOf(slots, "m1")).toBe("answer");
    // 槽位本身仍归 fold:突出卡由渲染层就地升级,不影响折叠边界。
    expect(roleOf(slots, "c1")).toBe("fold");
  });

  it("does not fold while the tool name is still unknown", () => {
    // 名字还没流过来时保守地不折:先折后弹回(比如它其实是交付调用)
    // 正是这层承诺要消灭的「内容改主意」。
    const slots = buildTimeline([narration("m1"), toolCall("c1", "call-1")]);
    expect(roleOf(slots, "m1")).toBe("answer");
  });

  it("does not fold on progress", () => {
    // 上下文压缩这类进度可能发生在最终回答之后,它不是助手「转去干活」。
    const slots = buildTimeline([
      narration("m1"),
      progress("p1"),
      narration("m2"),
    ]);
    expect(shape(slots)).toEqual([
      { key: "m1", kind: "narration", role: "answer" },
      { key: "p1", kind: "progress", role: "fold" },
      { key: "m2", kind: "narration", role: "answer" },
    ]);
  });
});

describe("buildTimeline 槽位序列", () => {
  it("ignores user messages", () => {
    // 时间线只描述一轮助手回复的内部结构;用户气泡由上层单独渲染。
    expect(buildTimeline([userText("u1")])).toEqual([]);
  });

  it("ignores an assistant message with no content yet", () => {
    // 信封先到、文本未到的空壳落地就是一条空行。
    expect(buildTimeline([make("m1", "message", [])])).toEqual([]);
  });

  it("keeps slots in original message order", () => {
    // append-only 的地基:槽位顺序恒等于消息到达顺序,已落地的条目永远
    // 不会因为后来的消息而换位置。角色可以在收口前摇摆,位置不行。
    const slots = buildTimeline([
      userText("u1"),
      reasoning("r1", "先想想"),
      narration("m1"),
      toolCall("c1", "call-1", "execute_shell_command"),
      toolOutput("o1", "call-1"),
      progress("p1"),
      narration("m2"),
    ]);
    expect(slots.map((slot) => slot.key)).toEqual([
      "r1",
      "m1",
      "c1",
      "p1",
      "m2",
    ]);
  });
});

describe("copyAnswerText", () => {
  it("copies only trailing answer narration", () => {
    const messages = [
      narration("m1", "我查一下北京今天的实时天气和预报。"),
      toolCall("c1", "call-1", "web_search"),
      toolOutput("o1", "call-1"),
      narration("m2", "北京今天多云，约 26°C。"),
    ];
    const slots = buildTimeline(messages);
    expect(copyAnswerText(messages, slots)).toBe("北京今天多云，约 26°C。");
  });

  it("falls back to all assistant prose when nothing is classified as the answer", () => {
    const messages = [narration("m1", "只有这一句")];
    const slots = buildTimeline(messages);
    expect(copyAnswerText(messages, slots)).toBe("只有这一句");
  });
});
