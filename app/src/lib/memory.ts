import { apiJson } from "./api";

export interface MdFileInfo {
  filename: string;
  path: string;
  size: number;
  created_time: string | number;
  modified_time: string | number;
}

export type MemoryGroupKey = "journal" | "procedure" | "wiki" | "other";

export interface MemoryGroup {
  key: MemoryGroupKey;
  items: MdFileInfo[];
}

const GROUP_ORDER: MemoryGroupKey[] = [
  "journal",
  "procedure",
  "wiki",
  "other",
];
const JOURNAL_PATTERN = /^\d{4}-\d{2}-\d{2}(?:\.md|\/)/;

export function memoryGroupKey(filename: string): MemoryGroupKey {
  if (JOURNAL_PATTERN.test(filename)) return "journal";
  if (filename.startsWith("digest/procedure/")) return "procedure";
  if (filename.startsWith("digest/wiki/")) return "wiki";
  return "other";
}

export function groupMemoryFiles(files: MdFileInfo[]): MemoryGroup[] {
  const groups = new Map<MemoryGroupKey, MdFileInfo[]>(
    GROUP_ORDER.map((key) => [key, []]),
  );
  for (const file of files) {
    groups.get(memoryGroupKey(file.filename))!.push(file);
  }
  return GROUP_ORDER.flatMap((key) => {
    const items = groups
      .get(key)!
      .slice()
      .sort(
        (left, right) =>
          timestampValue(right.modified_time) -
          timestampValue(left.modified_time),
      );
    return items.length > 0 ? [{ key, items }] : [];
  });
}

export function memoryDisplayName(file: MdFileInfo): string {
  switch (memoryGroupKey(file.filename)) {
    case "journal":
      return file.filename.replace(/^\d{4}-\d{2}-\d{2}\//, "");
    case "procedure":
      return file.filename.slice("digest/procedure/".length);
    case "wiki":
      return file.filename.slice("digest/wiki/".length);
    case "other":
      return file.filename;
  }
}

export function formatFileSize(bytes: number, language: "zh" | "en"): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; value >= 1024 && index < units.length; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${new Intl.NumberFormat(language === "zh" ? "zh-CN" : "en", {
    maximumFractionDigits: value >= 10 ? 0 : 1,
  }).format(value)} ${unit}`;
}

export function formatRelativeTime(
  value: string | number,
  language: "zh" | "en",
  now = Date.now(),
): string {
  const timestamp = timestampValue(value);
  if (!timestamp) return String(value);
  const seconds = Math.round((timestamp - now) / 1000);
  const absoluteSeconds = Math.abs(seconds);
  const [amount, unit]: [number, Intl.RelativeTimeFormatUnit] =
    absoluteSeconds < 60
      ? [seconds, "second"]
      : absoluteSeconds < 3600
        ? [Math.round(seconds / 60), "minute"]
        : absoluteSeconds < 86_400
          ? [Math.round(seconds / 3600), "hour"]
          : absoluteSeconds < 2_592_000
            ? [Math.round(seconds / 86_400), "day"]
            : absoluteSeconds < 31_536_000
              ? [Math.round(seconds / 2_592_000), "month"]
              : [Math.round(seconds / 31_536_000), "year"];
  return new Intl.RelativeTimeFormat(language === "zh" ? "zh-CN" : "en", {
    numeric: "auto",
  }).format(amount, unit);
}

function timestampValue(value: string | number): number {
  if (typeof value === "number") {
    return value < 1_000_000_000_000 ? value * 1000 : value;
  }
  const numeric = Number(value);
  if (value.trim() && Number.isFinite(numeric)) {
    return numeric < 1_000_000_000_000 ? numeric * 1000 : numeric;
  }
  const parsed = new Date(value).valueOf();
  return Number.isNaN(parsed) ? 0 : parsed;
}

export interface MemoryEditorState {
  mode: "view" | "editing";
  content: string;
  draft: string;
  saving: boolean;
  error: string | null;
}

export type MemoryEditorAction =
  | { type: "load"; content: string }
  | { type: "edit" }
  | { type: "change"; draft: string }
  | { type: "cancel" }
  | { type: "saveStart" }
  | { type: "saveSuccess" }
  | { type: "saveFailure"; error: string };

export const initialMemoryEditorState: MemoryEditorState = {
  mode: "view",
  content: "",
  draft: "",
  saving: false,
  error: null,
};

export function memoryEditorReducer(
  state: MemoryEditorState,
  action: MemoryEditorAction,
): MemoryEditorState {
  switch (action.type) {
    case "load":
      return {
        mode: "view",
        content: action.content,
        draft: action.content,
        saving: false,
        error: null,
      };
    case "edit":
      return {
        ...state,
        mode: "editing",
        draft: state.content,
        error: null,
      };
    case "change":
      return { ...state, draft: action.draft, error: null };
    case "cancel":
      return {
        ...state,
        mode: "view",
        draft: state.content,
        saving: false,
        error: null,
      };
    case "saveStart":
      return { ...state, saving: true, error: null };
    case "saveSuccess":
      return {
        mode: "view",
        content: state.draft,
        draft: state.draft,
        saving: false,
        error: null,
      };
    case "saveFailure":
      return { ...state, saving: false, error: action.error };
  }
}

function encodeMemoryPath(path: string): string {
  return path
    .replace(/^\/+/, "")
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

export const memoryApi = {
  list: (signal?: AbortSignal) =>
    apiJson<MdFileInfo[]>("/api/workspace/memory", { signal }),
  get: (path: string, signal?: AbortSignal) =>
    apiJson<{ content: string }>(
      `/api/workspace/memory/${encodeMemoryPath(path)}`,
      { signal },
    ),
  update: (path: string, content: string) =>
    apiJson<{ written: true }>(
      `/api/workspace/memory/${encodeMemoryPath(path)}`,
      {
        method: "PUT",
        body: JSON.stringify({ content }),
      },
    ),
};
