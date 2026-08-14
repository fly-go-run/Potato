import {
  buildToolPair,
  toolData,
  toolPairStatus,
  type ToolPair,
} from "../components/chat/ToolCard";
import { lineDiff } from "./lineDiff";
import { qpCount, recordLegacyParse } from "./toolMeta";
import type { StreamMessage } from "./stream";

export type FileChangeTool = "write_file" | "edit_file" | "append_file";

/** 一次成功落盘的写入/编辑/追加,保留前后文本供 diff 视图渲染。 */
export interface FileEdit {
  messageId: string;
  tool: FileChangeTool;
  /** edit_file 的 old_text;write/append 没有可比对的旧文本,恒为 ""。 */
  before: string;
  /** edit_file 的 new_text;write/append 的 content。 */
  after: string;
  additions: number;
  deletions: number;
  /** 超出 LCS 预算的大编辑:未做行级对齐,渲染时直接整块红/绿。 */
  oversized?: boolean;
}

/** 按文件合并后的会话(或单轮)改动。 */
export interface FileChange {
  path: string;
  name: string;
  dir: string;
  /** 主导操作:出现过 write 记 write,否则 edit,否则 append。 */
  kind: FileChangeTool;
  additions: number;
  deletions: number;
  /** 时间序的成功编辑;diff 视图按序渲染。 */
  edits: FileEdit[];
  /** 最后一次触碰该文件的工具调用消息,用于「定位到消息」。 */
  lastMessageId: string;
}

const CHANGE_TOOLS = new Set<string>([
  "write_file",
  "edit_file",
  "append_file",
]);

/**
 * 从消息流聚合文件改动。传整个会话得到会话级列表(侧栏「改动」tab),
 * 传单轮消息得到该轮的列表(回合末汇总卡)。只统计成功终态的调用:
 * 运行中(汇总会随流式推进增长)、失败、被取消的都不算改动。
 */
export function collectFileChanges(messages: StreamMessage[]): FileChange[] {
  const outputsByCallId = new Map<string, StreamMessage>();
  for (const message of messages) {
    if (!isToolOutput(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    if (callId) outputsByCallId.set(callId, message);
  }

  const byPath = new Map<string, FileChange>();
  for (const message of messages) {
    if (!isToolCall(message.type)) continue;
    const callData = toolData(message);
    if (!CHANGE_TOOLS.has(stringValue(callData.name))) continue;
    const callId = stringValue(callData.call_id);
    const pair = buildToolPair(
      message,
      callId ? outputsByCallId.get(callId) ?? null : null,
    );
    const edit = successfulEdit(message.id, pair);
    if (!edit) continue;
    const path = filePathOf(pair);
    if (!path) continue;

    const existing = byPath.get(path);
    if (existing) {
      existing.additions += edit.additions;
      existing.deletions += edit.deletions;
      existing.edits.push(edit);
      existing.lastMessageId = message.id;
      existing.kind = mergeKind(existing.kind, edit.tool);
    } else {
      byPath.set(path, {
        path,
        name: fileBaseName(path) || path,
        dir: directoryOf(path),
        kind: edit.tool,
        additions: edit.additions,
        deletions: edit.deletions,
        edits: [edit],
        lastMessageId: message.id,
      });
    }
  }
  return Array.from(byPath.values());
}

export function totalChangeStats(changes: FileChange[]): {
  files: number;
  additions: number;
  deletions: number;
} {
  let additions = 0;
  let deletions = 0;
  for (const change of changes) {
    additions += change.additions;
    deletions += change.deletions;
  }
  return { files: changes.length, additions, deletions };
}

/** write 覆盖 edit/append;edit 覆盖 append。表达"这个文件本会话被整写过"。 */
function mergeKind(
  current: FileChangeTool,
  next: FileChangeTool,
): FileChangeTool {
  if (current === "write_file" || next === "write_file") return "write_file";
  if (current === "edit_file" || next === "edit_file") return "edit_file";
  return "append_file";
}

/**
 * LCS 是 O(N×M) 时间/内存,超预算的编辑直接按"整块删+整块加"计,
 * 避免上万行的 old/new_text 在打开会话时分配巨型矩阵。
 */
const LCS_LINE_BUDGET = 200_000;

/**
 * 按消息 id 缓存:流式期间 MessageList/侧栏每次重渲染都会重新聚合,
 * 已完成的工具调用参数不会再变,没必要反复跑行级 diff。
 */
const editCache = new Map<string, FileEdit | null>();

function successfulEdit(messageId: string, pair: ToolPair): FileEdit | null {
  // 未到终态不缓存:等待 output 期间的 null 不能定格。
  if (!toolPairStatus(pair).completed) return null;
  // 带参数长度防 id 意外重复时串缓存(正常路径 id 全局唯一)。
  const cacheKey = `${messageId}:${pair.arguments.length}`;
  if (editCache.has(cacheKey)) return editCache.get(cacheKey) ?? null;
  const edit = computeEdit(messageId, pair);
  editCache.set(cacheKey, edit);
  return edit;
}

function computeEdit(messageId: string, pair: ToolPair): FileEdit | null {
  const parameters = parseArguments(pair.arguments);
  const tool = pair.name as FileChangeTool;
  // ±行数优先读后端 qp meta:那是对执行前后真实内容做的 diff,
  // 覆盖全局多次替换与覆盖写这两个本地估算天生算不准的情形。
  // before/after 仍取自参数——diff 面板展示的是模型提交的改动原文。
  const metaAdditions = qpCount(pair.meta, "additions");
  const metaDeletions = qpCount(pair.meta, "deletions");
  const metaCounts =
    metaAdditions !== null && metaDeletions !== null
      ? { additions: metaAdditions, deletions: metaDeletions }
      : null;

  if (tool === "edit_file") {
    const before =
      typeof parameters.old_text === "string" ? parameters.old_text : "";
    const after =
      typeof parameters.new_text === "string" ? parameters.new_text : "";
    if (!before && !after) return null;
    if (metaCounts) {
      return { messageId, tool, before, after, ...metaCounts };
    }
    recordLegacyParse("F7:lcs-estimate");
    const beforeLines = lineCount(before);
    const afterLines = lineCount(after);
    // 注意:后端 edit_file 是全局替换,old_text 多次出现时实际改动
    // 会多于这里的估计。精确数字以 git diff 视图为准。
    if (beforeLines * afterLines > LCS_LINE_BUDGET) {
      return {
        messageId,
        tool,
        before,
        after,
        additions: afterLines,
        deletions: beforeLines,
        oversized: true,
      };
    }
    let additions = 0;
    let deletions = 0;
    for (const line of lineDiff(before, after)) {
      if (line.kind === "add") additions += 1;
      else if (line.kind === "remove") deletions += 1;
    }
    return { messageId, tool, before, after, additions, deletions };
  }

  // write/append 没有旧文本可比。无 meta 时按"新增全部行"计
  // (覆盖写会高估,精确数字以 git diff 视图为准);不跑 LCS。
  // content 缺失/非字符串视为畸形调用跳过;空字符串是合法的"清空写入"。
  const content = parameters.content;
  if (typeof content !== "string") return null;
  if (metaCounts) {
    return { messageId, tool, before: "", after: content, ...metaCounts };
  }
  recordLegacyParse("F7:write-full-count");
  return {
    messageId,
    tool,
    before: "",
    after: content,
    additions: lineCount(content),
    deletions: 0,
  };
}

function filePathOf(pair: ToolPair): string {
  const parameters = parseArguments(pair.arguments);
  return typeof parameters.file_path === "string" ? parameters.file_path : "";
}

function lineCount(value: string): number {
  // 尾换行是行终止符不是新一行:"x\n" 是 1 行,不是 2 行。
  if (value === "") return 0;
  return value.replace(/\n$/, "").split("\n").length;
}

function parseArguments(value: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

/**
 * 展示用路径缩短:项目内文件显示仓库相对路径,其余把用户主目录缩成 ~。
 * 只用于显示;定位/预览仍应传原始绝对路径。
 */
export function shortenPath(path: string, projectDir?: string | null): string {
  const normalized = path.replaceAll("\\", "/");
  if (projectDir) {
    const prefix = projectDir.replaceAll("\\", "/").replace(/\/$/, "");
    if (prefix && normalized.startsWith(`${prefix}/`)) {
      return normalized.slice(prefix.length + 1);
    }
  }
  // macOS/Linux 主目录 → ~;Windows 的 C:/Users/<name> 同样缩写。
  return normalized
    .replace(/^\/(?:Users|home)\/[^/]+(?=\/)/, "~")
    .replace(/^[a-zA-Z]:\/Users\/[^/]+(?=\/)/, "~");
}

export function fileBaseName(path: string): string {
  return path.split(/[/\\]/).at(-1) ?? "";
}

export function directoryOf(path: string): string {
  const directory = path.slice(0, path.length - fileBaseName(path).length);
  return directory.replace(/[/\\]$/, "") || "";
}

function isToolCall(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call" ||
    type === "function_call" ||
    type === "mcp_tool_call"
  );
}

function isToolOutput(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call_output" ||
    type === "function_call_output" ||
    type === "mcp_tool_call_output"
  );
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}
