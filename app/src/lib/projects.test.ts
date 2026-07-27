import { describe, expect, it } from "vitest";
import {
  hasSessionProjectRecord,
  loadLastProject,
  loadRecentProjects,
  loadSessionProject,
  mergeProjects,
  rememberRecentProject,
  saveSessionProject,
  type ProjectBinding,
} from "./projects";

function memoryStorage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  };
}

describe("project selection persistence", () => {
  it("stores a binding per session and uses it as the next-chat default", () => {
    const storage = memoryStorage();
    const project = { path: "/work/repo", name: "repo", isGit: true };

    saveSessionProject("session-a", project, storage);

    expect(loadSessionProject("session-a", storage)).toEqual(project);
    expect(loadLastProject(storage)).toEqual(project);
    expect(hasSessionProjectRecord("session-a", storage)).toBe(true);
    expect(hasSessionProjectRecord("session-b", storage)).toBe(false);
  });

  it("persists an explicit default-workspace choice", () => {
    const storage = memoryStorage();
    saveSessionProject("session-a", null, storage);

    expect(hasSessionProjectRecord("session-a", storage)).toBe(true);
    expect(loadSessionProject("session-a", storage)).toBeNull();
    expect(loadLastProject(storage)).toBeNull();
  });

  it("deduplicates and caps manually browsed directories at eight", () => {
    const storage = memoryStorage();
    for (let index = 0; index < 10; index += 1) {
      rememberRecentProject(
        { path: `/work/${index}`, name: String(index) },
        storage,
      );
    }
    rememberRecentProject({ path: "/work/7", name: "seven" }, storage);

    const recent = loadRecentProjects(storage);
    expect(recent).toHaveLength(8);
    expect(recent[0]).toEqual({ path: "/work/7", name: "seven" });
    expect(new Set(recent.map((item) => item.path)).size).toBe(8);
  });

  it("prefers managed project metadata when merging recent paths", () => {
    const recent: ProjectBinding[] = [
      { path: "/work/repo", name: "old" },
      { path: "/other", name: "other" },
    ];
    const merged = mergeProjects(
      [
        {
          path: "/work/repo",
          name: "repo",
          is_git: true,
          is_active: false,
        },
      ],
      recent,
    );

    expect(merged).toEqual([
      { path: "/work/repo", name: "repo", isGit: true },
      { path: "/other", name: "other" },
    ]);
  });
});
