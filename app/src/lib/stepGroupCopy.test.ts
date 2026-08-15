import { describe, expect, it } from "vitest";
import type { ToolPair } from "../components/chat/ToolCard";
import { translate, type Language, type TranslationKey } from "./i18n";
import { formatStepGroupObject } from "./stepGroupCopy";
import type { ToolGroupRow } from "./stepGroups";

function stubs(count: number): ToolPair[] {
  return Array.from({ length: count }, () => ({}) as ToolPair);
}

function t(language: Language) {
  return (key: TranslationKey, params?: Record<string, string | number>) =>
    translate(key, language, params);
}

function group(partial: Partial<ToolGroupRow> & Pick<ToolGroupRow, "family">): ToolGroupRow {
  return {
    type: "group",
    key: "g",
    name: partial.name ?? partial.family,
    pairs: partial.pairs ?? stubs(partial.uniqueFiles ?? 2),
    object: "",
    objectVaried: false,
    objects: [],
    additions: 0,
    deletions: 0,
    skillName: "",
    direct: false,
    uniqueFiles: partial.pairs?.length ?? 2,
    ...partial,
  };
}

describe("formatStepGroupObject", () => {
  it("lists up to 3 file basenames and appends 等 when varied", () => {
    const row = group({
      family: "edit",
      objects: ["a.ts", "b.md"],
      object: "a.ts",
      objectVaried: true,
      uniqueFiles: 2,
      additions: 12,
      deletions: 4,
    });
    expect(formatStepGroupObject(row, t("zh"), "zh")).toBe("a.ts, b.md 等 +12 −4");
    expect(formatStepGroupObject(row, t("en"), "en")).toBe("a.ts, b.md etc. +12 −4");
  });

  it("falls back to the file/cmd unit only when no objects exist", () => {
    expect(
      formatStepGroupObject(
        group({ family: "read", uniqueFiles: 3, pairs: stubs(3) }),
        t("zh"),
        "zh",
      ),
    ).toBe("3 个");
    expect(
      formatStepGroupObject(
        group({ family: "read", uniqueFiles: 3, pairs: stubs(3) }),
        t("en"),
        "en",
      ),
    ).toBe("3 files");
    expect(
      formatStepGroupObject(
        group({ family: "shell", pairs: stubs(3) }),
        t("zh"),
        "zh",
      ),
    ).toBe("3 条");
    expect(
      formatStepGroupObject(
        group({ family: "shell", pairs: stubs(3) }),
        t("en"),
        "en",
      ),
    ).toBe("3 cmds");
  });

  it("writes search as 关键词 ×N and omits ×1", () => {
    const many = group({
      family: "search",
      object: "关键词",
      objects: ["关键词"],
      pairs: stubs(3),
    });
    expect(formatStepGroupObject(many, t("zh"), "zh")).toBe("关键词 ×3");
    const once = group({
      family: "search",
      object: "关键词",
      objects: ["关键词"],
      pairs: stubs(1),
    });
    expect(formatStepGroupObject(once, t("zh"), "zh")).toBe("关键词");
  });

  it("appends 等 to a repeated shell argv0 when the group has several cmds", () => {
    const row = group({
      family: "shell",
      object: "wc",
      objects: ["wc"],
      pairs: stubs(2),
    });
    expect(formatStepGroupObject(row, t("zh"), "zh")).toBe("wc 等");
    expect(formatStepGroupObject(row, t("en"), "en")).toBe("wc etc.");
  });
});
