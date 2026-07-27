export interface ProjectBinding {
  path: string;
  name: string;
  isGit?: boolean;
}

export interface CodingProject extends ProjectBinding {
  is_git: boolean;
  is_active: boolean;
}

export interface CodingProjectInfo {
  path: string;
  name: string;
  is_workspace_default: boolean;
  workspace_dir: string;
  exists: boolean;
}

export interface DirectoryEntry {
  name: string;
  path: string;
}

export interface DirectoryListing {
  current: string;
  parent: string | null;
  dirs: DirectoryEntry[];
  selectable?: boolean;
}

const SESSION_PREFIX = "qwenpaw_project_session:";
const LAST_PROJECT_KEY = "qwenpaw_project_last";
const RECENT_PROJECTS_KEY = "qwenpaw_project_recent";
const RECENT_LIMIT = 8;

function parseBinding(raw: string | null): ProjectBinding | null {
  if (!raw || raw === "null") return null;
  try {
    const value = JSON.parse(raw) as Partial<ProjectBinding>;
    if (typeof value.path !== "string" || typeof value.name !== "string") {
      return null;
    }
    return {
      path: value.path,
      name: value.name,
      ...(typeof value.isGit === "boolean" ? { isGit: value.isGit } : {}),
    };
  } catch {
    return null;
  }
}

export function loadSessionProject(
  sessionId: string,
  storage: Pick<Storage, "getItem"> = localStorage,
): ProjectBinding | null {
  return parseBinding(storage.getItem(`${SESSION_PREFIX}${sessionId}`));
}

export function hasSessionProjectRecord(
  sessionId: string,
  storage: Pick<Storage, "getItem"> = localStorage,
): boolean {
  return storage.getItem(`${SESSION_PREFIX}${sessionId}`) !== null;
}

export function loadLastProject(
  storage: Pick<Storage, "getItem"> = localStorage,
): ProjectBinding | null {
  return parseBinding(storage.getItem(LAST_PROJECT_KEY));
}

export function saveSessionProject(
  sessionId: string,
  project: ProjectBinding | null,
  storage: Pick<Storage, "setItem"> = localStorage,
) {
  const value = JSON.stringify(project);
  storage.setItem(`${SESSION_PREFIX}${sessionId}`, value);
  storage.setItem(LAST_PROJECT_KEY, value);
}

export function loadRecentProjects(
  storage: Pick<Storage, "getItem"> = localStorage,
): ProjectBinding[] {
  try {
    const value = JSON.parse(storage.getItem(RECENT_PROJECTS_KEY) ?? "[]");
    if (!Array.isArray(value)) return [];
    return value
      .filter(
        (item): item is ProjectBinding =>
          Boolean(item) &&
          typeof item.path === "string" &&
          typeof item.name === "string",
      )
      .slice(0, RECENT_LIMIT);
  } catch {
    return [];
  }
}

export function rememberRecentProject(
  project: ProjectBinding,
  storage: Pick<Storage, "getItem" | "setItem"> = localStorage,
): ProjectBinding[] {
  const projects = [
    project,
    ...loadRecentProjects(storage).filter((item) => item.path !== project.path),
  ].slice(0, RECENT_LIMIT);
  storage.setItem(RECENT_PROJECTS_KEY, JSON.stringify(projects));
  return projects;
}

export function mergeProjects(
  managed: CodingProject[],
  recent: ProjectBinding[],
): ProjectBinding[] {
  const merged = new Map<string, ProjectBinding>();
  for (const project of managed) {
    merged.set(project.path, {
      path: project.path,
      name: project.name,
      isGit: project.is_git,
    });
  }
  for (const project of recent) {
    if (!merged.has(project.path)) merged.set(project.path, project);
  }
  return [...merged.values()];
}

export function projectNameFromPath(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const segments = trimmed.split(/[\\/]/);
  return segments.at(-1) || path;
}
