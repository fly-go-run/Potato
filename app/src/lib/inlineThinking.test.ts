import { describe, expect, it } from "vitest";
import { splitInlineThinking } from "./inlineThinking";

describe("splitInlineThinking", () => {
  it("无标签文本原样返回", () => {
    const raw = "正常回答,末尾带空行\n\n";
    const split = splitInlineThinking(raw);
    expect(split).toEqual({ thinking: "", text: raw, open: false, changed: false });
  });

  it("抽出完整思考块并拼接前后正文", () => {
    const split = splitInlineThinking(
      "前文<thinking>Listing top-level files</thinking>后文",
    );
    expect(split.thinking).toBe("Listing top-level files");
    expect(split.text).toBe("前文后文");
    expect(split.open).toBe(false);
    expect(split.changed).toBe(true);
  });

  it("支持 <think> 变体与多段合并", () => {
    const split = splitInlineThinking(
      "<think>第一段</think>正文<think>第二段</think>",
    );
    expect(split.thinking).toBe("第一段\n\n第二段");
    expect(split.text).toBe("正文");
  });

  it("未闭合思考块视为流式思考中", () => {
    const split = splitInlineThinking("<thinking>还在想");
    expect(split.thinking).toBe("还在想");
    expect(split.text).toBe("");
    expect(split.open).toBe(true);
  });

  it("末尾残缺开标签先扣住不进正文", () => {
    const split = splitInlineThinking("Hello <thin");
    expect(split.text).toBe("Hello");
    expect(split.thinking).toBe("");
    expect(split.open).toBe(false);
    expect(split.changed).toBe(true);
  });

  it("空思考块只做剥离", () => {
    const split = splitInlineThinking("A<thinking></thinking>B");
    expect(split.thinking).toBe("");
    expect(split.text).toBe("AB");
    expect(split.changed).toBe(true);
  });
});
