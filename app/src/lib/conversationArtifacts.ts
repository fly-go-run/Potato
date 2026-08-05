import { isSuccessfulArtifactPair } from "../components/chat/FileToolCard";
import { buildToolPair, toolData } from "../components/chat/ToolCard";
import type { RunStatus } from "./protocol/types";
import type { StreamMessage } from "./stream";

export interface ConversationArtifact {
  id: string;
  path: string;
  name: string;
  sourceMessageId: string;
}

export function collectConversationArtifacts(
  messages: StreamMessage[],
): ConversationArtifact[] {
  const outputsByCallId = new Map<string, StreamMessage>();
  for (const message of messages) {
    if (!isToolOutput(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    if (callId) outputsByCallId.set(callId, message);
  }

  const byPath = new Map<string, ConversationArtifact>();
  for (const message of messages) {
    if (!isToolCall(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    const pair = buildToolPair(
      message,
      callId ? outputsByCallId.get(callId) ?? null : null,
    );
    if (!isSuccessfulArtifactPair(pair)) continue;
    const path = filePathFromArguments(pair.arguments);
    if (!path) continue;
    byPath.set(path, {
      id: `${message.id}:${path}`,
      path,
      name: fileBaseName(path) || path,
      sourceMessageId: message.id,
    });
  }
  return Array.from(byPath.values()).reverse();
}

/**
 * Resolve an assistant-authored Markdown link to a surfaced local file.
 * Models often link a friendly basename even though the tool call contains
 * the useful absolute path. Ambiguous basenames remain ordinary links.
 */
export function resolveConversationFileLink(
  href: string,
  artifacts: ConversationArtifact[],
): string | null {
  const raw = href.trim();
  if (!raw || raw.startsWith("#") || raw.startsWith("//")) return null;
  // OpenAI 系模型习惯用 sandbox: 协议链接沙箱产物,携带的就是本地绝对路径。
  // 协议名至少两位:单字母加冒号是 Windows 盘符(C:\)而非 URL scheme。
  if (/^[a-z][a-z0-9+.-]+:/i.test(raw) && !/^(?:file|sandbox):/i.test(raw)) {
    return null;
  }

  let decoded = raw;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    // Keep the original value when a model emits a malformed percent escape.
  }
  // 剥掉协议;file:///x 的空 authority 双斜杠一并去掉,
  // sandbox:/x 单斜杠属于路径本体,必须保留。
  // Windows file URI(file:///C:/x、file://C:/x)盘符前的斜杠不属于路径。
  const path = decoded
    .replace(/^(?:file|sandbox):/i, "")
    .replace(/^\/\/(?=\/)/, "")
    .split(/[?#]/, 1)[0]!
    .replaceAll("\\", "/")
    .replace(/^\/+(?=[a-z]:\/)/i, "");
  if (!path) return null;
  if (path.startsWith("/") || /^[a-z]:\//i.test(path)) return path;

  const relative = path.replace(/^\.\//, "");
  const basename = relative.split("/").at(-1) ?? relative;
  const matches = artifacts.filter((artifact) => {
    const artifactPath = artifact.path.replaceAll("\\", "/");
    return (
      artifactPath === relative ||
      artifactPath.endsWith(`/${relative}`) ||
      artifact.name === basename
    );
  });
  return matches.length === 1 ? matches[0]!.path : null;
}

export function presentRunStatus(status: RunStatus | "idle"): {
  label:
    | "chat.panel.running"
    | "chat.panel.completed"
    | "chat.panel.failed"
    | "chat.panel.cancelled";
  dotClass: string;
} {
  if (status === "created" || status === "in_progress") {
    return { label: "chat.panel.running", dotClass: "animate-pulse bg-ok" };
  }
  if (status === "failed") {
    return { label: "chat.panel.failed", dotClass: "bg-danger" };
  }
  if (status === "cancelled") {
    return { label: "chat.panel.cancelled", dotClass: "bg-warn" };
  }
  return { label: "chat.panel.completed", dotClass: "bg-ink-muted" };
}

function filePathFromArguments(argumentsValue: string): string {
  try {
    const parsed = JSON.parse(argumentsValue) as Record<string, unknown>;
    return typeof parsed.file_path === "string" ? parsed.file_path : "";
  } catch {
    return "";
  }
}

function fileBaseName(path: string): string {
  return path.split(/[/\\\\]/).at(-1) ?? "";
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
