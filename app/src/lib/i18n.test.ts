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
  });
});
