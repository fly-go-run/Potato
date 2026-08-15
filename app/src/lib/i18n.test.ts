import { describe, expect, it } from "vitest";
import { dictionaries, translate } from "./i18n";

describe("i18n dictionaries", () => {
  it("keeps zh and en key sets complete and identical", () => {
    expect(Object.keys(dictionaries.en).sort()).toEqual(
      Object.keys(dictionaries.zh).sort(),
    );
  });

  it("interpolates parameters in both languages", () => {
    expect(translate("chat.contextUsed", "zh", { ratio: "12.5" })).toBe(
      "上下文已用 12.5%",
    );
    expect(translate("chat.contextUsed", "en", { ratio: "12.5" })).toBe(
      "Context used 12.5%",
    );
    expect(
      translate("chat.diff.inlineTruncated", "zh", { count: 12 }),
    ).toBe("已截断 12 行 · 在侧栏查看完整改动");
    expect(
      translate("chat.diff.inlineTruncated", "en", { count: 12 }),
    ).toBe("Truncated 12 lines · View full change in sidebar");
    expect(translate("chat.workedFor", "zh", { duration: "8.4s" })).toBe("8.4s");
    expect(
      translate("chat.workedForWithFailures", "en", {
        duration: "8.4s",
        failed: 2,
      }),
    ).toBe("8.4s · 2 failed");
    expect(translate("chat.step.files", "zh", { count: 3 })).toBe("3 个");
    expect(translate("chat.step.cmds", "en", { count: 3 })).toBe("3 cmds");
  });
});
