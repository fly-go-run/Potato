/**
 * 后端 SSE 流协议类型（唯一权威定义，Phase 1 的 stream.ts 必须以此为准）。
 *
 * 来源：src/qwenpaw/schemas.py + src/qwenpaw/runtime/envelope.py
 * 文档与样本：app/docs/api-contract.md、app/fixtures/sse/
 *
 * 每个 SSE 帧是一行 `data: <json>`。JSON 分四类：
 *  1. object === "response"  → 整轮生命周期（created → in_progress → completed/failed）
 *  2. object === "message"   → 消息信封（气泡/工具卡的开始与结束）
 *  3. object === "content"   → 内容块（流式增量或终值），凭 msg_id 归属消息
 *  4. 无 object 的杂帧       → turn_usage / rate_limited / error
 */

export type RunStatus =
  | "created"
  | "in_progress"
  | "completed"
  | "failed"
  | "cancelled";

export type MessageKind =
  | "message"
  | "reasoning"
  | "plugin_call"
  | "plugin_call_output"
  | "function_call"
  | "function_call_output"
  | "mcp_tool_call"
  | "mcp_tool_call_output"
  | "progress"
  | "result";

export type Role = "user" | "assistant" | "system" | "tool";

/** 内容块公共字段（schemas._ContentBase） */
interface ContentBase {
  object: "content";
  delta: boolean;
  index: number | null;
  status: RunStatus | null;
  /** 所属消息 id；据此把增量归到正确气泡 */
  msg_id: string | null;
  sequence_number?: number;
}

export interface TextContent extends ContentBase {
  type: "text";
  text: string;
}

export interface ImageContent extends ContentBase {
  type: "image";
  image_url: string | null;
}

export interface AudioContent extends ContentBase {
  type: "audio";
  data: string | null;
  format: string | null;
}

export interface VideoContent extends ContentBase {
  type: "video";
  video_url: string | null;
}

export interface FileContent extends ContentBase {
  type: "file";
  filename: string | null;
  file_url: string | null;
}

/** 工具调用参数（DataContent.data，流式时 arguments 为增量片段） */
export interface FunctionCallData {
  call_id?: string;
  name?: string;
  arguments?: string;
}

/** 工具结果（DataContent.data；state 仅在 TOOL_RESULT_END 帧出现） */
export interface FunctionCallOutputData {
  call_id?: string;
  name?: string;
  /** 纯文本结果；或含富媒体块时为 JSON 数组字符串（见契约文档 §SSE-4） */
  output?: string;
  state?: string;
}

export interface DataContent extends ContentBase {
  type: "data";
  data: FunctionCallData | FunctionCallOutputData | Record<string, unknown>;
}

export type ContentBlock =
  | TextContent
  | ImageContent
  | AudioContent
  | VideoContent
  | FileContent
  | DataContent;

/**
 * Optional stream-message phase. Missing / null = unknown.
 * Answer values are "commentary" | "final_answer"; compaction and other
 * host uses keep their own strings. Not consumed by fold/render this round.
 */
export type AnswerPhase = "commentary" | "final_answer";

export type MessageMetadata = Record<string, unknown> & {
  phase?: AnswerPhase | string;
};

/** object === "message"：消息信封帧 */
export interface MessageFrame {
  object: "message";
  id: string;
  type: MessageKind;
  role: Role | null;
  /** in_progress 首帧为 []；completed 帧含最终内容块 */
  content: unknown[];
  status: RunStatus;
  metadata: MessageMetadata | null;
  name?: string;
  usage?: TokenUsage;
  sequence_number?: number;
}

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
}

/** object === "response"：整轮生命周期帧 */
export interface ResponseFrame {
  object: "response";
  id: string;
  status: RunStatus;
  output: MessageFrame[];
  created_at: string | null;
  completed_at: string | null;
  session_id?: string;
  usage?: TokenUsage;
  error?: { code: string; message: string };
  metadata: Record<string, unknown> | null;
  sequence_number?: number;
}

/** 尾帧：本轮 token 用量与上下文占用（渲染 composer 上方的用量指示） */
export interface TurnUsageFrame {
  type: "turn_usage";
  session_id: string;
  usage: Record<string, unknown> | null;
  context_usage: {
    context_usage_ratio?: number;
    [k: string]: unknown;
  } | null;
}

/** 限流帧：提示并列出可切换的免费模型 */
export interface RateLimitedFrame {
  type: "rate_limited";
  error: string;
  alternatives: FreeModelAlternative[];
}

export interface FreeModelAlternative {
  provider_id: string;
  provider_name: string;
  model_id: string;
  model_name: string;
}

/** 顶层错误帧（除 response.error 外的兜底） */
export interface ErrorFrame {
  error: string | { code?: string; message?: string };
}

export type SseFrame =
  | ResponseFrame
  | MessageFrame
  | ContentBlock
  | TurnUsageFrame
  | RateLimitedFrame
  | ErrorFrame;

/** 判别工具 */
export function isResponseFrame(f: SseFrame): f is ResponseFrame {
  return (f as ResponseFrame).object === "response";
}
export function isMessageFrame(f: SseFrame): f is MessageFrame {
  return (f as MessageFrame).object === "message";
}
export function isContentFrame(f: SseFrame): f is ContentBlock {
  return (f as ContentBlock).object === "content";
}
export function isTurnUsageFrame(f: SseFrame): f is TurnUsageFrame {
  return (f as TurnUsageFrame).type === "turn_usage";
}
export function isRateLimitedFrame(f: SseFrame): f is RateLimitedFrame {
  return (f as RateLimitedFrame).type === "rate_limited";
}
