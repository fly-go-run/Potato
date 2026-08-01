import { describe, expect, it } from "vitest";
import {
  groupMemoryFiles,
  initialMemoryEditorState,
  memoryDisplayName,
  memoryEditorReducer,
  memoryTimeIso,
  type MdFileInfo,
} from "./memory";

function file(filename: string, modified_time: string): MdFileInfo {
  return {
    filename,
    path: `/memory/${filename}`,
    size: 128,
    created_time: modified_time,
    modified_time,
  };
}

describe("memory grouping", () => {
  it("classifies every supported prefix and sorts each group newest first", () => {
    const groups = groupMemoryFiles([
      file("misc/note.md", "2026-07-20T10:00:00Z"),
      file("digest/wiki/cache.md", "2026-07-23T10:00:00Z"),
      file("2026-07-27/older.md", "2026-07-27T09:00:00Z"),
      file("digest/procedure/deploy.md", "2026-07-22T10:00:00Z"),
      file("2026-07-27.md", "2026-07-27T11:00:00Z"),
    ]);

    expect(groups.map((group) => group.key)).toEqual([
      "journal",
      "procedure",
      "wiki",
      "other",
    ]);
    expect(groups[0].items.map((item) => item.filename)).toEqual([
      "2026-07-27.md",
      "2026-07-27/older.md",
    ]);
  });

  it("turns slugs into natural titles without file extensions", () => {
    expect(
      memoryDisplayName(file("2026-07-27/shell-entry.md", "2026-07-27")),
    ).toBe("Shell entry");
    expect(
      memoryDisplayName(file("digest/procedure/deploy_to_prod.md", "2026-07-27")),
    ).toBe("Deploy to prod");
    expect(memoryDisplayName(file("misc/note.md", "2026-07-27"))).toBe("Note");
    expect(memoryDisplayName(file("digest/wiki/沙箱边界.md", "2026-07-27"))).toBe(
      "沙箱边界",
    );
    // 日记按日期命名，分隔符不能被拆成空格
    expect(memoryDisplayName(file("2026-07-27.md", "2026-07-27"))).toBe(
      "2026-07-27",
    );
  });
});

describe("memoryTimeIso", () => {
  it("normalises epoch seconds, epoch millis and date strings", () => {
    expect(memoryTimeIso(1_774_000_000)).toBe(
      new Date(1_774_000_000_000).toISOString(),
    );
    expect(memoryTimeIso(1_774_000_000_000)).toBe(
      new Date(1_774_000_000_000).toISOString(),
    );
    expect(memoryTimeIso("2026-07-27T11:00:00Z")).toBe(
      "2026-07-27T11:00:00.000Z",
    );
    expect(memoryTimeIso("not-a-date")).toBeNull();
  });
});

describe("memory editor state", () => {
  it("discards edits on cancel", () => {
    let state = memoryEditorReducer(initialMemoryEditorState, {
      type: "load",
      content: "original",
    });
    state = memoryEditorReducer(state, { type: "edit" });
    state = memoryEditorReducer(state, { type: "change", draft: "changed" });
    state = memoryEditorReducer(state, { type: "cancel" });

    expect(state).toMatchObject({
      mode: "view",
      content: "original",
      draft: "original",
      saving: false,
    });
  });

  it("keeps the draft after failure and commits it after success", () => {
    let state = memoryEditorReducer(initialMemoryEditorState, {
      type: "load",
      content: "original",
    });
    state = memoryEditorReducer(state, { type: "edit" });
    state = memoryEditorReducer(state, { type: "change", draft: "changed" });
    state = memoryEditorReducer(state, { type: "saveStart" });
    state = memoryEditorReducer(state, {
      type: "saveFailure",
      error: "offline",
    });

    expect(state).toMatchObject({
      mode: "editing",
      content: "original",
      draft: "changed",
      saving: false,
      error: "offline",
    });

    state = memoryEditorReducer(state, { type: "saveStart" });
    state = memoryEditorReducer(state, { type: "saveSuccess" });
    expect(state).toMatchObject({
      mode: "view",
      content: "changed",
      draft: "changed",
      saving: false,
      error: null,
    });
  });
});
