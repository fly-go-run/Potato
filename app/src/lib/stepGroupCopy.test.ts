import { describe, expect, it } from "vitest";
import type { ToolPair } from "../components/chat/ToolCard";
import { translate, type Language, type TranslationKey } from "./i18n";
import {
  formatStepGroupObject,
  formatStepGroupVerb,
} from "./stepGroupCopy";
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

  it("compacts a stuffed weather query to the place and day", () => {
    const row = group({
      family: "search",
      object: "北京 今天 2026年8月16日 天气 实时 温度 降水预报",
      objects: ["北京 今天 2026年8月16日 天气 实时 温度 降水预报"],
      pairs: stubs(1),
    });
    expect(formatStepGroupObject(row, t("zh"), "zh")).toBe("北京今天");
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

  it("names the quiet verb for each family", () => {
    expect(formatStepGroupVerb("search", t("zh"))).toBe("搜了");
    expect(formatStepGroupVerb("shell", t("en"))).toBe("Ran");
  });

  it("truncates a shell command to a footnote", () => {
    const row = group({
      family: "shell",
      object: "ssh",
      objects: ["ssh"],
      pairs: [
        {
          arguments: JSON.stringify({
            command: "ssh macbook-m1 'echo FILES; find /Users/liuxu/project'",
          }),
        } as ToolPair,
      ],
    });
    expect(formatStepGroupObject(row, t("zh"), "zh")).toBe(
      "ssh macbook-m1 'echo FILES; find /Users/liu…",
    );
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
