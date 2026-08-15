import {
  toolData,
  toolPairStatus,
  type ToolPair,
} from "../components/chat/ToolCard";
import { textFromContent } from "./content";
import { pairChangeStats } from "./fileChanges";
import type { StreamMessage } from "./stream";

export type ProcessEntry =
  | { kind: "reasoning"; key: string; message: StreamMessage }
  | { kind: "progress"; key: string; message: StreamMessage }
  | { kind: "pair"; key: string; pair: ToolPair };

export type ToolFamily =
  | "search"
  | "fetch"
  | "grep"
  | "glob"
  | "read"
  | "edit"
  | "shell"
  | "skill"
  | "other";

export type ToolGroupRow = {
  type: "group";
  key: string;
  family: ToolFamily;
  name: string;
  pairs: ToolPair[];
  object: string;
  objectVaried: boolean;
  additions: number;
  deletions: number;
  skillName: string;
  direct: boolean;
  /**
   * read/edit 家族的展示计数:file_path 去重后的文件数。同一文件被
   * 连续改两次报「2 个文件」会和轮末改动卡的「1 个文件」自相矛盾。
   * 有 pair 缺 file_path 时无法证明同一文件,回落 pairs.length。
   */
  uniqueFiles: number;
};

export type ThinkingRow = {
  type: "thinking";
  key: string;
  messages: StreamMessage[];
  text: string;
};

export type ProgressRow = {
  type: "progress";
  key: string;
  message: StreamMessage;
};

export type FoldRow = ToolGroupRow | ThinkingRow | ProgressRow;

export type MaterializedItem =
  | { kind: "fold"; row: FoldRow }
  | { kind: "visible-failed"; key: string; pair: ToolPair };

export const FOLD_WINDOW = 8;

const FAMILY_BY_NAME: Record<string, Exclude<ToolFamily, "other">> = {
  web_search: "search",
  web_fetch: "fetch",
  grep_search: "grep",
  glob_search: "glob",
  read_file: "read",
  write_file: "edit",
  edit_file: "edit",
  append_file: "edit",
  execute_shell_command: "shell",
  skill: "skill",
};

const OBJECT_LIMIT = 32;

export function toolFamily(name: string): ToolFamily {
  const normalized = name.replace(/^mcp__/i, "").toLocaleLowerCase();
  return FAMILY_BY_NAME[normalized] ?? "other";
}

export function rawToolName(pair: ToolPair): string {
  return (
    stringValue(toolData(pair.call).name) ||
    stringValue(toolData(pair.output).name)
  );
}

export function skillNameOf(pair: ToolPair): string {
  const args = parseArgs(pair.arguments);
  return (
    stringField(args, "name") ||
    stringField(args, "skill") ||
    stringField(args, "skill_name")
  );
}

export function extractPairObject(family: ToolFamily, pair: ToolPair): string {
  const args = parseArgs(pair.arguments);
  switch (family) {
    case "search":
      return truncate32(
        stringField(args, "search_term") || stringField(args, "query"),
      );
    case "fetch":
      return truncate32(stringField(args, "url"));
    case "grep":
    case "glob":
      return truncate32(stringField(args, "pattern"));
    case "read":
    case "edit":
      return basename(stringField(args, "file_path"));
    case "shell":
      return argv0(stringField(args, "command"));
    case "skill":
      return skillNameOf(pair);
    default:
      return "";
  }
}

/**
 * 把一段连续过程条目收成 fold-row,失败 pair 升为 visible。
 * 不能先滤成 ToolPair[]:思考 / progress / 失败分界都要保留。
 */
export function materializeRun(entries: ProcessEntry[]): MaterializedItem[] {
  const items: MaterializedItem[] = [];
  let index = 0;
  while (index < entries.length) {
    const entry = entries[index]!;
    if (entry.kind === "reasoning") {
      const messages = [entry.message];
      index += 1;
      while (index < entries.length && entries[index]?.kind === "reasoning") {
        const next = entries[index] as Extract<
          ProcessEntry,
          { kind: "reasoning" }
        >;
        messages.push(next.message);
        index += 1;
      }
      items.push({
        kind: "fold",
        row: {
          type: "thinking",
          key: entry.message.id,
          messages,
          text: joinReasoning(messages),
        },
      });
      continue;
    }
    if (entry.kind === "progress") {
      items.push({
        kind: "fold",
        row: { type: "progress", key: entry.key, message: entry.message },
      });
      index += 1;
      continue;
    }
    if (toolPairStatus(entry.pair).failed) {
      items.push({
        kind: "visible-failed",
        key: entry.key,
        pair: entry.pair,
      });
      index += 1;
      continue;
    }
    const group = [entry];
    const firstPair = entry.pair;
    index += 1;
    if (rawToolName(firstPair) && toolPairStatus(firstPair).completed) {
      while (index < entries.length && entries[index]?.kind === "pair") {
        const next = entries[index] as Extract<ProcessEntry, { kind: "pair" }>;
        if (toolPairStatus(next.pair).failed) break;
        if (!canMerge(firstPair, next.pair)) break;
        group.push(next);
        index += 1;
      }
    }
    items.push({ kind: "fold", row: makeToolGroup(group) });
  }
  return items;
}

export function focusFoldRowKey(
  rows: FoldRow[],
  liveOrSettling: boolean,
): string | null {
  if (!liveOrSettling) return null;
  for (let index = rows.length - 1; index >= 0; index -= 1) {
    const row = rows[index]!;
    if (
      row.type === "group" &&
      row.pairs.some((pair) => toolPairStatus(pair).running)
    ) {
      return row.key;
    }
  }
  for (let index = rows.length - 1; index >= 0; index -= 1) {
    const row = rows[index]!;
    if (row.type === "thinking" && row.messages.some(isInFlight)) {
      return row.key;
    }
  }
  return null;
}

export function windowFoldRows<T extends { key: string }>(
  rows: T[],
  opts: {
    settled: boolean;
    focusKey: string | null;
    overflowOpen: boolean;
  },
): {
  shownKeys: string[];
  hiddenCount: number;
  overflowAt: "start" | "end" | null;
} {
  if (opts.overflowOpen || rows.length <= FOLD_WINDOW) {
    return {
      shownKeys: rows.map((row) => row.key),
      hiddenCount: 0,
      overflowAt: null,
    };
  }
  if (opts.settled) {
    const shown = rows.slice(0, FOLD_WINDOW - 1);
    return {
      shownKeys: shown.map((row) => row.key),
      hiddenCount: rows.length - shown.length,
      overflowAt: "end",
    };
  }
  let endExclusive = rows.length;
  if (opts.focusKey) {
    const focusIndex = rows.findIndex((row) => row.key === opts.focusKey);
    if (focusIndex >= 0) endExclusive = focusIndex + 1;
  }
  const start = Math.max(0, endExclusive - FOLD_WINDOW);
  const shown = rows.slice(start, endExclusive);
  return {
    shownKeys: shown.map((row) => row.key),
    hiddenCount: rows.length - shown.length,
    overflowAt: start > 0 || endExclusive < rows.length ? "start" : null,
  };
}

function makeToolGroup(
  entries: Extract<ProcessEntry, { kind: "pair" }>[],
): ToolGroupRow {
  const first = entries[0]!;
  const name = rawToolName(first.pair);
  const family = toolFamily(name);
  const pairs = entries.map((entry) => entry.pair);
  const { object, varied } = firstNonEmptyObject(family, pairs);
  const stats =
    family === "edit" ? sumEditStats(pairs) : { additions: 0, deletions: 0 };
  return {
    type: "group",
    key: first.pair.callId || first.key,
    family,
    name,
    pairs,
    object,
    objectVaried: varied,
    additions: stats.additions,
    deletions: stats.deletions,
    skillName: family === "skill" ? skillNameOf(first.pair) : "",
    direct: pairs.length === 1,
    uniqueFiles: uniqueFileCount(family, pairs),
  };
}

function uniqueFileCount(family: ToolFamily, pairs: ToolPair[]): number {
  if (family !== "read" && family !== "edit") return pairs.length;
  const paths = new Set<string>();
  for (const pair of pairs) {
    const path = stringField(parseArgs(pair.arguments), "file_path");
    if (!path) return pairs.length;
    paths.add(path);
  }
  return paths.size;
}

function canMerge(first: ToolPair, next: ToolPair): boolean {
  const firstName = rawToolName(first);
  const nextName = rawToolName(next);
  if (!firstName || !nextName) return false;
  if (!toolPairStatus(first).completed || !toolPairStatus(next).completed) {
    return false;
  }
  const firstFamily = toolFamily(firstName);
  const nextFamily = toolFamily(nextName);
  if (firstFamily !== nextFamily) return false;
  if (firstFamily === "other" && firstName !== nextName) return false;
  if (firstFamily === "skill" && skillNameOf(first) !== skillNameOf(next)) {
    return false;
  }
  return true;
}

function firstNonEmptyObject(
  family: ToolFamily,
  pairs: ToolPair[],
): { object: string; varied: boolean } {
  let first = "";
  let varied = false;
  for (const pair of pairs) {
    const object = extractPairObject(family, pair);
    if (!object) continue;
    if (!first) {
      first = object;
      continue;
    }
    if (object !== first) varied = true;
  }
  return { object: first, varied };
}

function sumEditStats(pairs: ToolPair[]): {
  additions: number;
  deletions: number;
} {
  let additions = 0;
  let deletions = 0;
  for (const pair of pairs) {
    if (!toolPairStatus(pair).completed) continue;
    const stats = pairChangeStats(pair);
    if (!stats) continue;
    additions += stats.additions;
    deletions += stats.deletions;
  }
  return { additions, deletions };
}

function joinReasoning(messages: StreamMessage[]): string {
  return messages
    .map((message) => textFromContent(message.content))
    .filter((text) => text.trim().length > 0)
    .join("\n\n");
}

function isInFlight(message: StreamMessage): boolean {
  return message.status === "created" || message.status === "in_progress";
}

function parseArgs(raw: string): Record<string, unknown> {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
    return {};
  } catch {
    return {};
  }
}

function stringField(data: Record<string, unknown>, key: string): string {
  const value = data[key];
  return typeof value === "string" ? value : "";
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function truncate32(value: string): string {
  return value.length > OBJECT_LIMIT ? value.slice(0, OBJECT_LIMIT) : value;
}

function basename(path: string): string {
  if (!path) return "";
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function argv0(command: string): string {
  const trimmed = command.trim();
  if (!trimmed) return "";
  const token = trimmed.match(/^\S+/);
  return token?.[0] ?? "";
}
