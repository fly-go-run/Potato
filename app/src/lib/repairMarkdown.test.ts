import { describe, expect, it } from "vitest";
import { repairMarkdown } from "./repairMarkdown";

describe("repairMarkdown", () => {
  it("moves the closer before CJK punctuation", () => {
    expect(repairMarkdown("**气温：**约 21~29°C")).toBe(
      "**气温**：约 21~29°C",
    );
    expect(repairMarkdown("**气温： **约 21~29°C")).toBe(
      "**气温**：约 21~29°C",
    );
  });

  it("leaves healthy emphasis alone", () => {
    expect(repairMarkdown("**气温**约 21~29°C")).toBe("**气温**约 21~29°C");
    expect(repairMarkdown("普通文本")).toBe("普通文本");
  });
});
