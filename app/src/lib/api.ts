import type {
  PendingApproval,
  PushMessagesResponse,
} from "./approvals";
import type {
  CronDispatchTarget,
  CronExecutionRecord,
  CronJobSpec,
  CronJobState,
} from "./crons";
import { t } from "./i18n";
import type { InboxEvent, InboxTrace } from "./inbox";
import type {
  CodingProject,
  CodingProjectInfo,
  DirectoryListing,
} from "./projects";

export const AUTH_TOKEN_KEY = "qwenpaw_auth_token";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export function getAuthToken() {
  return localStorage.getItem(AUTH_TOKEN_KEY);
}

export function setAuthToken(token: string) {
  if (token) localStorage.setItem(AUTH_TOKEN_KEY, token);
  else localStorage.removeItem(AUTH_TOKEN_KEY);
}

export function authHeaders(headers?: HeadersInit) {
  const result = new Headers(headers);
  const token = getAuthToken();
  if (token) result.set("Authorization", `Bearer ${token}`);
  return result;
}

export async function apiFetch(
  input: string,
  init: RequestInit = {},
): Promise<Response> {
  const response = await fetch(input, {
    ...init,
    headers: authHeaders(init.headers),
  });

  if (response.status === 401) {
    setAuthToken("");
    if (window.location.hash !== "#/login") {
      window.location.hash = "#/login";
    }
  }
  if (!response.ok) {
    throw new ApiError(await responseErrorMessage(response), response.status);
  }
  return response;
}

export async function apiJson<T>(
  input: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !(init.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  const response = await apiFetch(input, { ...init, headers });
  return (await response.json()) as T;
}

async function responseErrorMessage(response: Response) {
  try {
    const body = (await response.json()) as {
      detail?: string;
      message?: string;
      error?: string;
    };
    return (
      body.detail ||
      body.message ||
      body.error ||
      t("api.requestFailed", { status: response.status })
    );
  } catch {
    return (
      response.statusText ||
      t("api.requestFailed", { status: response.status })
    );
  }
}

export interface AuthStatus {
  enabled: boolean;
  has_users: boolean;
}

export interface ActiveModel {
  provider_id: string;
  model: string;
}

export interface ActiveModelInfo {
  active_llm: ActiveModel | null;
  effective_max_input_length: number | null;
}

export interface ChatSpec {
  id: string;
  name: string;
  session_id: string;
  user_id: string;
  channel: string;
  created_at: string;
  updated_at: string;
  status: "idle" | "running" | string;
  pinned: boolean;
  archived?: boolean;
  archived_at?: string | null;
}

export interface ChatHistory {
  messages: unknown[];
  status: "idle" | "running" | string;
}

export interface ModelInfo {
  id: string;
  name: string;
  supports_multimodal?: boolean | null;
  supports_image?: boolean | null;
  supports_video?: boolean | null;
  is_free?: boolean;
  max_tokens?: number;
  max_input_length?: number;
  reasoning_effort?: string | null;
  thinking_param_style?: "effort" | "budget" | null;
  reasoning_effort_options?: string[] | null;
}

export interface ProviderInfo {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  chat_model: string;
  models: ModelInfo[];
  extra_models: ModelInfo[];
  api_key_prefix: string;
  api_key_prefixes: string[];
  is_local: boolean;
  freeze_url: boolean;
  require_api_key: boolean;
  is_custom: boolean;
  thinking_param_style?: "effort" | "budget" | null;
  reasoning_effort_options?: string[] | null;
}

/** Whether a provider can be selected without waiting for another setup step. */
export function providerReady(provider: ProviderInfo): boolean {
  return provider.is_local || !provider.require_api_key || Boolean(provider.api_key);
}

/** Whether the user saved a real provider connection in settings. */
export function providerConfigured(provider: ProviderInfo): boolean {
  if (provider.api_key) return true;
  // 自定义供应商:只有声明不需要 key 的,才能仅凭 URL 算已配置;
  // 需要 key 却没有 key(比如刚被清除)必须诚实地显示未配置。
  return (
    provider.is_custom &&
    Boolean(provider.base_url) &&
    !provider.require_api_key
  );
}

export interface UploadResponse {
  url: string;
  file_name: string;
  size: number;
}

export interface UploadLimit {
  upload_max_size_mb: number | null;
}

export const authApi = {
  status: () => apiJson<AuthStatus>("/api/auth/status"),
  authenticate: (
    mode: "login" | "register",
    username: string,
    password: string,
  ) =>
    apiJson<{ token: string; username: string }>(`/api/auth/${mode}`, {
      method: "POST",
      body: JSON.stringify({ username, password }),
    }),
};

export const chatApi = {
  list: () => apiJson<ChatSpec[]>("/api/chats?archived=false"),
  get: (id: string, signal?: AbortSignal) =>
    apiJson<ChatHistory>(`/api/chats/${encodeURIComponent(id)}`, { signal }),
  update: (id: string, update: { name?: string; pinned?: boolean }) =>
    apiJson<ChatSpec>(`/api/chats/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify(update),
    }),
  delete: (id: string) =>
    apiJson<{ deleted: boolean }>(`/api/chats/${encodeURIComponent(id)}`, {
      method: "DELETE",
    }),
  stop: (id: string) =>
    apiJson<{ stopped: boolean }>(
      `/api/console/chat/stop?chat_id=${encodeURIComponent(id)}`,
      { method: "POST" },
    ),
  stream: (body: Record<string, unknown>, signal: AbortSignal) =>
    apiFetch("/api/console/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal,
    }),
  pushMessages: (sessionId: string, signal?: AbortSignal) =>
    apiJson<PushMessagesResponse>(
      `/api/console/push-messages?session_id=${encodeURIComponent(sessionId)}`,
      { signal },
    ),
  upload: (file: File) => {
    const body = new FormData();
    body.append("file", file);
    return apiJson<UploadResponse>("/api/console/upload", {
      method: "POST",
      body,
    });
  },
};

export const modelApi = {
  active: () => apiJson<ActiveModelInfo>("/api/models/active"),
  list: () => apiJson<ProviderInfo[]>("/api/models"),
  setActive: async (providerId: string, model: string) => {
    // The unscoped GET resolves the current agent's effective model. Persist
    // to that same agent scope so an existing override cannot mask the update.
    const { agent_id } = await apiJson<{ agent_id: string }>(
      "/api/workspace/language",
    );
    return apiJson<ActiveModelInfo>("/api/models/active", {
      method: "PUT",
      body: JSON.stringify({
        provider_id: providerId,
        model,
        scope: "agent",
        agent_id,
      }),
    });
  },
  discover: (providerId: string) =>
    apiJson<{
      success: boolean;
      models: ModelInfo[];
      message: string;
      added_count: number;
    }>(`/api/models/${encodeURIComponent(providerId)}/discover?save=true`, {
      method: "POST",
    }),
  configureModel: (
    providerId: string,
    modelId: string,
    config: {
      reasoning_effort?: string | null;
      thinking_param_style?: string | null;
      reasoning_effort_options?: string[] | null;
    },
  ) =>
    apiJson<ProviderInfo>(
      `/api/models/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId).replace(/%2F/g, "/")}/config`,
      { method: "PUT", body: JSON.stringify(config) },
    ),
  removeModel: (providerId: string, modelId: string) =>
    apiJson<ProviderInfo>(
      // 后端路由是 {model_id:path},斜杠须保留原样(org/model 形式的 id)
      `/api/models/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId).replace(/%2F/g, "/")}`,
      { method: "DELETE" },
    ),
  addModel: (providerId: string, model: { id: string; name: string }) =>
    apiJson<ProviderInfo>(
      `/api/models/${encodeURIComponent(providerId)}/models`,
      { method: "POST", body: JSON.stringify(model) },
    ),
  configure: async (
    providerId: string,
    config: { api_key?: string; base_url?: string },
  ) => {
    const path = `/api/models/${encodeURIComponent(providerId)}/config`;
    return apiJson<ProviderInfo>(path, {
      method: "PUT",
      body: JSON.stringify(config),
    });
  },
  removeProvider: (providerId: string) =>
    apiJson<ProviderInfo[]>(
      `/api/models/custom-providers/${encodeURIComponent(providerId)}`,
      { method: "DELETE" },
    ),
  /** 探活:可携带未保存的 key/url 试连,后端不落盘。 */
  testProvider: (
    providerId: string,
    overrides?: { api_key?: string; base_url?: string },
  ) =>
    apiJson<{ success: boolean; message: string }>(
      `/api/models/${encodeURIComponent(providerId)}/test`,
      { method: "POST", body: JSON.stringify(overrides ?? {}) },
    ),
  createCustomProvider: (input: {
    id: string;
    name: string;
    default_base_url: string;
  }) =>
    apiJson<ProviderInfo>("/api/models/custom-providers", {
      method: "POST",
      body: JSON.stringify(input),
    }),
};

export const approvalApi = {
  act: (
    action: "approve" | "deny",
    approval: Pick<PendingApproval, "request_id" | "root_session_id">,
    scope: "exact" | "similar" = "exact",
  ) =>
    apiJson<{
      success: boolean;
      message: string;
      tool_name: string | null;
      request_id: string;
    }>(`/api/approval/${action}`, {
      method: "POST",
      body: JSON.stringify({
        request_id: approval.request_id,
        session_id: approval.root_session_id,
        scope,
      }),
    }),
};

export const workspaceApi = {
  runningConfig: () =>
    apiJson<{ approval_level?: string }>("/api/workspace/running-config"),
};

export const projectApi = {
  current: () =>
    apiJson<CodingProjectInfo>("/api/workspace/coding-project"),
  list: () =>
    apiJson<CodingProject[]>("/api/workspace/coding-project/list"),
  browse: (path = "~") =>
    apiJson<DirectoryListing>(
      `/api/workspace/coding-project/browse-dirs?path=${encodeURIComponent(path)}`,
    ),
  create: (name: string) =>
    apiJson<{ path: string; name: string }>(
      "/api/workspace/coding-project/create",
      {
        method: "POST",
        body: JSON.stringify({ name }),
      },
    ),
};

export const cronApi = {
  list: () => apiJson<CronJobSpec[]>("/api/cron/jobs"),
  create: (spec: CronJobSpec) =>
    apiJson<CronJobSpec>("/api/cron/jobs", {
      method: "POST",
      body: JSON.stringify(spec),
    }),
  replace: (jobId: string, spec: CronJobSpec) =>
    apiJson<CronJobSpec>(`/api/cron/jobs/${encodeURIComponent(jobId)}`, {
      method: "PUT",
      body: JSON.stringify(spec),
    }),
  delete: (jobId: string) =>
    apiJson<{ deleted: boolean }>(
      `/api/cron/jobs/${encodeURIComponent(jobId)}`,
      { method: "DELETE" },
    ),
  state: (jobId: string) =>
    apiJson<CronJobState>(
      `/api/cron/jobs/${encodeURIComponent(jobId)}/state`,
    ),
  history: (jobId: string) =>
    apiJson<CronExecutionRecord[]>(
      `/api/cron/jobs/${encodeURIComponent(jobId)}/history`,
    ),
  action: (jobId: string, action: "pause" | "resume" | "run") =>
    apiJson<Record<string, boolean>>(
      `/api/cron/jobs/${encodeURIComponent(jobId)}/${action}`,
      { method: "POST" },
    ),
  dispatchTargets: () =>
    apiJson<{ channels: string[]; items: CronDispatchTarget[] }>(
      "/api/cron/dispatch-targets",
    ),
};

export const inboxApi = {
  events: (options?: { unreadOnly?: boolean; limit?: number }) => {
    const query = new URLSearchParams();
    query.set("unread_only", String(options?.unreadOnly ?? false));
    query.set("limit", String(options?.limit ?? 100));
    return apiJson<{ events: InboxEvent[] }>(
      `/api/console/inbox/events?${query.toString()}`,
    );
  },
  markRead: (payload: { all?: boolean; event_ids?: string[] }) =>
    apiJson<{ updated: number }>("/api/console/inbox/read", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  delete: (eventId: string) =>
    apiJson<{ deleted: boolean; trace_deleted: boolean; run_id: string | null }>(
      `/api/console/inbox/events/${encodeURIComponent(eventId)}`,
      { method: "DELETE" },
    ),
  trace: (runId: string) =>
    apiJson<InboxTrace>(
      `/api/console/inbox/traces/${encodeURIComponent(runId)}`,
    ),
};

export interface SandboxStatus {
  enabled: boolean;
  /** 配置开启但本会话是否真正生效（Windows 非管理员 / 平台不支持时为 false）。 */
  effective: boolean;
  /** effective !== enabled 时的原因：not_admin | unsupported。 */
  reason: string | null;
}

export const settingsApi = {
  uploadLimit: () => apiJson<UploadLimit>("/api/settings/upload-limit"),
  sandboxStatus: () =>
    apiJson<SandboxStatus>("/api/config/security/sandbox"),
  setSandbox: (enabled: boolean) =>
    apiJson<SandboxStatus>("/api/config/security/sandbox", {
      method: "PUT",
      body: JSON.stringify({ enabled }),
    }),
};

export interface GitChangedFile {
  path: string;
  status: string;
  staged: boolean;
}

export interface GitStatus {
  branch: string;
  changes: GitChangedFile[];
  ahead: number;
  behind: number;
}

/**
 * 工作区 git 只读接口。后端作用于 coding_mode.project_dir(未配置则
 * agent workspace),不能按会话传目录 —— 调用方需自己做「拿不到就回落」。
 */
export const workspaceGitApi = {
  status: (signal?: AbortSignal) =>
    apiJson<GitStatus>("/api/workspace/git/status", { signal }),
  diff: (
    path: string,
    options?: { staged?: boolean; untracked?: boolean },
    signal?: AbortSignal,
  ) => {
    const params = new URLSearchParams({ path });
    if (options?.staged) params.set("staged", "true");
    if (options?.untracked) params.set("untracked", "true");
    return apiJson<{ diff: string }>(
      `/api/workspace/git/diff?${params.toString()}`,
      { signal },
    );
  },
  /** 丢弃工作区改动:tracked 走 git restore,untracked 走 git clean。 */
  discard: (paths: string[]) =>
    apiJson<{ discarded: string[] }>("/api/workspace/git/discard", {
      method: "POST",
      body: JSON.stringify({ paths }),
    }),
};

/** 拉取工作区文件的原始文本(预览端点),供前端高亮/渲染。 */
export async function fetchFileText(
  path: string,
  signal?: AbortSignal,
  maxBytes = 2_000_000,
): Promise<string> {
  const url = filePreviewUrl(path);
  // 先 HEAD 看体积,超限直接失败让调用方回落 iframe,
  // 避免把几百 MB 的日志整个拉进内存后才发现太大。
  try {
    const head = await apiFetch(url, { method: "HEAD", signal });
    const length = Number(head.headers.get("content-length"));
    if (Number.isFinite(length) && length > maxBytes) {
      throw new ApiError("file too large for text preview", 413);
    }
  } catch (error) {
    if (error instanceof ApiError && error.status === 413) throw error;
    // HEAD 不可用时继续走 GET,由调用方的字符数上限兜底。
  }
  const response = await apiFetch(url, { signal });
  return response.text();
}

export function filePreviewUrl(value: string): string {
  if (!value) return "";
  if (/^https?:\/\//i.test(value)) return value;
  const cleaned = value.replace(/^file:\/\//, "").replace(/^\/+/, "");
  const path = cleaned
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
  const token = getAuthToken();
  return `/api/files/preview/${path}${
    token ? `?token=${encodeURIComponent(token)}` : ""
  }`;
}
