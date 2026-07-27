import { apiFetch, apiJson } from "./api";

export interface SkillInfo {
  name: string;
  emoji?: string;
  description: string;
  enabled: boolean;
  version_text?: string;
  tags?: string[];
  source?: string;
  installed_from?: string;
}

export interface PoolSkillInfo extends Omit<SkillInfo, "enabled"> {
  protected?: boolean;
  external?: boolean;
}

export interface WorkspaceSkillSummary {
  agent_id: string;
  agent_name?: string;
  workspace_dir: string;
  skills: SkillInfo[];
}

export interface HubSkillInfo {
  slug: string;
  name: string;
  description: string;
  version?: string;
  source_url: string;
  author?: string;
  icon_url?: string;
}

export type HubInstallStatus =
  | "pending"
  | "importing"
  | "completed"
  | "failed"
  | "cancelled";

export interface HubInstallTask {
  task_id: string;
  bundle_url: string;
  version?: string;
  status: HubInstallStatus;
  error?: string | null;
  result?: Record<string, unknown> | null;
}

export interface PluginInfo {
  id: string;
  name: string;
  description?: string;
  version?: string;
  source?: string;
  installed_from?: string;
  author?: string;
  enabled?: boolean;
  loaded?: boolean;
  plugin_type?: string;
  tool_count?: number;
  tools?: unknown[];
}

export interface CatalogPlugin {
  id: string;
  plugin_id: string;
  name: string;
  description?: string;
  description_i18n?: Record<string, string>;
  version?: string;
  author?: string;
  install_url: string;
  installed: boolean;
  installed_version?: string | null;
  upgrade_available?: boolean;
}

export interface PluginCatalog {
  updated_at?: string | null;
  plugins: CatalogPlugin[];
  error?: string | null;
}

export const skillApi = {
  list: () => apiJson<SkillInfo[]>("/api/skills"),
  pool: () => apiJson<PoolSkillInfo[]>("/api/skills/pool"),
  workspaces: () =>
    apiJson<WorkspaceSkillSummary[]>("/api/skills/workspaces"),
  setEnabled: (name: string, enabled: boolean) =>
    apiJson<Record<string, unknown>>(
      `/api/skills/${encodeURIComponent(name)}/${enabled ? "enable" : "disable"}`,
      { method: "POST" },
    ),
  delete: (name: string) =>
    apiJson<{ deleted: boolean }>(
      `/api/skills/${encodeURIComponent(name)}`,
      { method: "DELETE" },
    ),
  importFromPool: (name: string, workspaceId: string) =>
    apiJson<{ downloaded: Array<{ name: string }> }>(
      "/api/skills/pool/download",
      {
        method: "POST",
        body: JSON.stringify({
          skill_name: name,
          targets: [{ workspace_id: workspaceId }],
        }),
      },
    ),
  searchHub: (query: string) =>
    apiJson<HubSkillInfo[]>(
      `/api/skills/hub/search?q=${encodeURIComponent(query)}&limit=20`,
    ),
  startHubInstall: (skill: HubSkillInfo) =>
    apiJson<HubInstallTask>("/api/skills/hub/install/start", {
      method: "POST",
      body: JSON.stringify({
        bundle_url: skill.source_url,
        version: skill.version ?? "",
        enable: true,
      }),
    }),
  hubInstallStatus: (taskId: string) =>
    apiJson<HubInstallTask>(
      `/api/skills/hub/install/status/${encodeURIComponent(taskId)}`,
    ),
  cancelHubInstall: (taskId: string) =>
    apiJson<{ task_id: string; status: HubInstallStatus }>(
      `/api/skills/hub/install/cancel/${encodeURIComponent(taskId)}`,
      { method: "POST" },
    ),
  upload: (file: File) => {
    const body = new FormData();
    body.append("file", file);
    return apiJson<Record<string, unknown>>("/api/skills/upload", {
      method: "POST",
      body,
    });
  },
};

export const pluginApi = {
  list: () => apiJson<PluginInfo[]>("/api/plugins"),
  catalog: () => apiJson<PluginCatalog>("/api/plugins/catalog"),
  install: (source: string) =>
    apiJson<PluginInfo>("/api/plugins/install", {
      method: "POST",
      body: JSON.stringify({ source }),
    }),
  upload: (file: File) => {
    const body = new FormData();
    body.append("file", file);
    return apiJson<PluginInfo>("/api/plugins/upload", {
      method: "POST",
      body,
    });
  },
  delete: (id: string) =>
    apiFetch(`/api/plugins/${encodeURIComponent(id)}`, {
      method: "DELETE",
    }),
};

export function applySkillToggle(
  skills: SkillInfo[],
  name: string,
  enabled: boolean,
) {
  return skills.map((skill) =>
    skill.name === name ? { ...skill, enabled } : skill,
  );
}

export async function runOptimisticSkillToggle({
  skills,
  name,
  enabled,
  onUpdate,
  mutate,
}: {
  skills: SkillInfo[];
  name: string;
  enabled: boolean;
  onUpdate: (skills: SkillInfo[]) => void;
  mutate: () => Promise<unknown>;
}) {
  onUpdate(applySkillToggle(skills, name, enabled));
  try {
    await mutate();
  } catch (error) {
    onUpdate(skills);
    throw error;
  }
}

export async function pollHubInstall(
  getStatus: (taskId: string) => Promise<HubInstallTask>,
  taskId: string,
  wait: () => Promise<void> = () =>
    new Promise((resolve) => globalThis.setTimeout(resolve, 900)),
) {
  while (true) {
    const task = await getStatus(taskId);
    if (
      task.status === "completed" ||
      task.status === "failed" ||
      task.status === "cancelled"
    ) {
      return task;
    }
    await wait();
  }
}

export function mergeCatalogInstalled(
  catalog: CatalogPlugin[],
  installed: PluginInfo[],
) {
  const installedById = new Map(
    installed.map((plugin) => [plugin.id, plugin.version ?? ""]),
  );
  return catalog.map((plugin) => {
    const installedVersion = installedById.get(plugin.plugin_id);
    return {
      ...plugin,
      installed: installedVersion !== undefined || plugin.installed,
      installed_version:
        installedVersion !== undefined
          ? installedVersion
          : plugin.installed_version,
    };
  });
}

export function pluginToolCount(plugin: PluginInfo) {
  if (typeof plugin.tool_count === "number") return plugin.tool_count;
  return Array.isArray(plugin.tools) ? plugin.tools.length : null;
}
