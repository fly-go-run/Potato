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
