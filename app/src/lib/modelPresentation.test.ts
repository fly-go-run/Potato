import { describe, expect, it } from "vitest";
import { prettyModelName } from "./modelPresentation";

describe("prettyModelName", () => {
  it("prefers backend name when distinct", () => {
    expect(prettyModelName("deepseek-v4-flash", "DeepSeek-V4 Flash")).toBe(
      "DeepSeek-V4 Flash",
    );
  });
  it("whitelist may drop vendor and reorder", () => {
    expect(prettyModelName("gpt-5.6-terra")).toBe("Terra 5.6");
    expect(prettyModelName("grok-4.6")).toBe("Grok 4.6");
  });
  it("fallback never drops the first token", () => {
    expect(prettyModelName("gpt-5.6")).toBe("GPT 5.6");
    expect(prettyModelName("qwen3-coder")).toBe("Qwen3 Coder");
  });
  it("strips date suffixes and keeps channel words", () => {
    expect(prettyModelName("claude-sonnet-20250514")).toBe("Claude Sonnet");
    expect(prettyModelName("gemini-2.5-preview")).toBe("Gemini 2.5 Preview");
  });
  it("takes the last path segment", () => {
    expect(prettyModelName("org/qwen3-coder")).toBe("Qwen3 Coder");
  });
  it("keeps CJK intact, title-cases latin tokens only", () => {
    expect(prettyModelName("智谱-glm4")).toBe("智谱 GLM4");
  });
});
