import { describe, expect, it } from "vitest";
import { applyTrigger, detectTrigger } from "./composerTrigger";

describe("detectTrigger", () => {
  it("文本开头的 / 触发技能", () => {
    expect(detectTrigger("/", 1)).toEqual({
      kind: "slash",
      start: 0,
      query: "",
    });
    expect(detectTrigger("/doc", 4)).toEqual({
      kind: "slash",
      start: 0,
      query: "doc",
    });
  });

  it("空白之后的 @ 触发文件引用", () => {
    expect(detectTrigger("看下 @re", 6)).toEqual({
      kind: "at",
      start: 3,
      query: "re",
    });
  });

  it("紧贴文字的符号不触发(路径、邮箱)", () => {
    expect(detectTrigger("a/b", 3)).toBeNull();
    expect(detectTrigger("me@qq", 5)).toBeNull();
  });

  it("token 内出现空白后不再触发", () => {
    expect(detectTrigger("/doc 转换", 7)).toBeNull();
  });

  it("光标不在 token 内不触发", () => {
    expect(detectTrigger("/doc", 0)).toBeNull();
  });
});

describe("applyTrigger", () => {
  it("替换触发 token 并保留后文", () => {
    const trigger = detectTrigger("先 /do 然后", 5);
    expect(trigger).not.toBeNull();
    const result = applyTrigger("先 /do 然后", 5, trigger!, "doc-convert");
    expect(result.text).toBe("先 /doc-convert  然后");
    expect(result.caret).toBe(2 + "/doc-convert ".length);
  });

  it("@ 文件引用带回文件名", () => {
    const trigger = detectTrigger("@", 1);
    const result = applyTrigger("@", 1, trigger!, "报告.pdf");
    expect(result.text).toBe("@报告.pdf ");
    expect(result.caret).toBe("@报告.pdf ".length);
  });
});
