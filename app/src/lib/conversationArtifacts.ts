import { isSuccessfulArtifactPair } from "../components/chat/FileToolCard";
import { buildToolPair, toolData } from "../components/chat/ToolCard";
import type { RunStatus } from "./protocol/types";
import type { StreamMessage } from "./stream";

export interface ConversationArtifact {
  id: string;
  path: string;
  name: string;
  sourceMessageId: string;
  /** 交付方式:显式 send_file_to_user,或最终答复里的链接点名。 */
  via: "sent" | "linked";
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

  // 写文件只说明执行过程中产生了文件，不等于用户要收下这个文件。
  // 先把成功写入记录成链接解析候选；只有显式发送或最终答复链接到它时，
  // 才进入真正的产物集合。
  const candidatesByPath = new Map<string, ConversationArtifact>();
  const deliveredByPath = new Map<string, ConversationArtifact>();
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
    const key = artifactPathKey(path);
    const sent = pair.name === "send_file_to_user";
    const artifact: ConversationArtifact = {
      id: `${message.id}:${path}`,
      path,
      name: fileBaseName(path) || path,
      sourceMessageId: message.id,
      via: sent ? "sent" : "linked",
    };
    candidatesByPath.set(key, artifact);
    if (sent) {
      deliveredByPath.set(key, artifact);
    }
  }

  const candidates = Array.from(candidatesByPath.values());
  for (const message of messages) {
    if (message.type !== "message" || message.role !== "assistant") continue;
    for (const content of message.content) {
      if (content.type !== "text") continue;
      // 预筛:绝大多数消息没有协议链接,直接跳过,
      // 也避免病态文本(大量未闭合 `[`)进入链接正则的重复扫描。
      if (!/(?:sandbox|file):/i.test(content.text)) continue;
      for (const href of markdownLinkHrefs(content.text)) {
        if (!/^(?:sandbox|file):/i.test(href)) continue;
        const path = resolveConversationFileLink(href, candidates);
        if (!path) continue;
        const key = artifactPathKey(path);
        if (deliveredByPath.has(key)) continue;
        const candidate = candidatesByPath.get(key);
        deliveredByPath.set(
          key,
          candidate
            ? { ...candidate, via: "linked" }
            : {
                id: `${message.id}:${path}`,
                path,
                name: fileBaseName(path) || path,
                sourceMessageId: message.id,
                via: "linked",
              },
        );
      }
    }
  }
  return Array.from(deliveredByPath.values()).reverse();
}

/**
 * 是否应把文件工具从执行轨道提升成面向用户的产物卡。
 *
 * 一个文件只配一张大卡:显式发送的由 send 调用出卡;写入类调用只有在
 * 「正文链接点名交付且没人发送过它」时才补位出卡。此前写入与发送各出
 * 一张再加汇总卡,同一文件在一轮里最多曝光三次——写文件是过程,不是
 * 交付,过程留在轨道里当安静行。
 */
export function shouldPresentArtifactPair(
  pair: ReturnType<typeof buildToolPair>,
  artifacts: ConversationArtifact[],
): boolean {
  // 显式发送本身就是交付动作；运行中也保持在突出位置。
  if (pair.name === "send_file_to_user") return true;
  const path = filePathFromArguments(pair.arguments);
  if (!path) return false;
  const key = artifactPathKey(path);
  return artifacts.some(
    (artifact) =>
      artifact.via === "linked" && artifactPathKey(artifact.path) === key,
  );
}

/** 去重 key:斜杠归一 + Windows 盘符大小写归一(路径本体保持原样)。 */
function artifactPathKey(path: string): string {
  return path
    .replaceAll("\\", "/")
    .replace(/^[a-z]:(?=\/)/i, (drive) => drive.toUpperCase());
}

function* markdownLinkHrefs(text: string): Iterable<string> {
  // 仅识别普通 Markdown 链接；图片语法(![])不是面向用户交付的文件。
  // 标签支持转义方括号且不跨行；裸 href 支持一层平衡括号
  // (下载文件常见 report(1).pdf),带空格/嵌套括号的走 <...> 包裹分支。
  const links =
    /(?<!!)\[(?:\\.|[^\]\\\n])*\]\(\s*(?:<([^>\n]+)>|((?:\([^\s()]*\)|[^\s()])+))\s*\)/g;
  for (const match of text.matchAll(links)) {
    yield match[1] ?? match[2] ?? "";
  }
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
