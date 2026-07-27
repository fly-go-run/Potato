import { describe, expect, it } from "vitest";
import {
  groupMemoryFiles,
  initialMemoryEditorState,
  memoryDisplayName,
  memoryEditorReducer,
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

  it("removes only known group prefixes from display names", () => {
    expect(
      memoryDisplayName(file("2026-07-27/shell-entry.md", "2026-07-27")),
    ).toBe("shell-entry.md");
    expect(
      memoryDisplayName(file("digest/procedure/deploy.md", "2026-07-27")),
    ).toBe("deploy.md");
    expect(memoryDisplayName(file("misc/note.md", "2026-07-27"))).toBe(
      "misc/note.md",
    );
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
