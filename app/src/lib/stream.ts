import {
  isContentFrame,
  isMessageFrame,
  isRateLimitedFrame,
  isResponseFrame,
  isTurnUsageFrame,
  type ContentBlock,
  type DataContent,
  type MessageFrame,
  type MessageMetadata,
  type RateLimitedFrame,
  type ResponseFrame,
  type RunStatus,
  type SseFrame,
  type TurnUsageFrame,
} from "./protocol/types";
import { t } from "./i18n";

export interface SseParserState {
  buffer: string;
  /** UTF-8 code point cut across two network chunks. */
  trailingBytes: number[];
  errors: string[];
}

export interface ParsedSseChunk {
  frames: SseFrame[];
  state: SseParserState;
}

export interface StreamMessage {
  id: string;
  type: MessageFrame["type"];
  role: MessageFrame["role"];
  status: RunStatus;
  content: ContentBlock[];
  /** Optional answer phase. Missing / null = unknown. */
  metadata: MessageMetadata | null;
  name?: string;
  usage?: MessageFrame["usage"];
}

export interface ConversationStreamState {
  responseId: string | null;
  responseStatus: RunStatus | "idle";
  sessionId: string | null;
  messages: StreamMessage[];
  turnUsage: TurnUsageFrame | null;
  rateLimited: RateLimitedFrame | null;
  error: string | null;
  lastSequenceNumber: number;
  clearHistoryVersion: number;
}

export const initialSseParserState: SseParserState = {
  buffer: "",
  trailingBytes: [],
  errors: [],
};

export const initialConversationStreamState: ConversationStreamState = {
  responseId: null,
  responseStatus: "idle",
  sessionId: null,
  messages: [],
  turnUsage: null,
  rateLimited: null,
  error: null,
  lastSequenceNumber: 0,
  clearHistoryVersion: 0,
};

export function isUnexpectedStreamEof(
  status: ConversationStreamState["responseStatus"],
  aborted: boolean,
): boolean {
  return (
    !aborted &&
    status !== "completed" &&
    status !== "failed" &&
    status !== "cancelled"
  );
}

export function isUnfinishedResponse(
  status: ConversationStreamState["responseStatus"],
): boolean {
  return status === "created" || status === "in_progress";
}

/**
 * Parse decoded SSE text incrementally. A frame is emitted only after its
 * terminating blank line arrives, so a TCP chunk may end anywhere.
 */
export function parseSseChunk(
  chunk: string,
  previous: SseParserState = initialSseParserState,
): ParsedSseChunk {
  const normalized = (previous.buffer + chunk).replace(/\r\n/g, "\n");
  const parts = normalized.split("\n\n");
  const buffer = parts.pop() ?? "";
  const frames: SseFrame[] = [];
  const errors = [...previous.errors];

  for (const event of parts) {
    const data = event
      .split("\n")
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).replace(/^ /, ""))
      .join("\n");
    if (!data || data === "[DONE]") continue;

    try {
      frames.push(JSON.parse(data) as SseFrame);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }

  return {
    frames,
    state: { buffer, trailingBytes: previous.trailingBytes, errors },
  };
}

/**
 * Byte-oriented parser. It retains only an incomplete UTF-8 suffix and is
 * otherwise immutable, which keeps it deterministic and easy to test.
 */
export function parseSseBytes(
  chunk: Uint8Array,
  previous: SseParserState = initialSseParserState,
): ParsedSseChunk {
  const bytes = new Uint8Array(previous.trailingBytes.length + chunk.length);
  bytes.set(previous.trailingBytes);
  bytes.set(chunk, previous.trailingBytes.length);
  const completeLength = completeUtf8PrefixLength(bytes);
  const decoded = new TextDecoder().decode(bytes.subarray(0, completeLength));
  const parsed = parseSseChunk(decoded, {
    ...previous,
    trailingBytes: [],
  });
  return {
    frames: parsed.frames,
    state: {
      ...parsed.state,
      trailingBytes: Array.from(bytes.subarray(completeLength)),
    },
  };
}

function completeUtf8PrefixLength(bytes: Uint8Array): number {
  if (bytes.length === 0) return 0;

  let continuationCount = 0;
  let cursor = bytes.length - 1;
  while (cursor >= 0 && (bytes[cursor]! & 0xc0) === 0x80) {
    continuationCount += 1;
    cursor -= 1;
  }
  if (cursor < 0) return 0;

  const lead = bytes[cursor]!;
  const expected =
    (lead & 0x80) === 0
      ? 1
      : (lead & 0xe0) === 0xc0
      ? 2
      : (lead & 0xf0) === 0xe0
      ? 3
      : (lead & 0xf8) === 0xf0
      ? 4
      : 1;
  const available = continuationCount + 1;
  return expected > available ? cursor : bytes.length;
}

export function reduceStreamFrame(
  state: ConversationStreamState,
  frame: SseFrame,
): ConversationStreamState {
  const sequenceNumber = getSequenceNumber(frame);
  // Each Runtime.run() (one payload) starts Envelope._seq_counter at 0.
  // Follow-ups drain on the same SSE, so a new response id means the
  // watermark must reset or the whole next turn is dropped as a replay.
  const newResponse =
    isResponseFrame(frame) &&
    Boolean(frame.id) &&
    frame.id !== state.responseId;
  const lastSeq = newResponse ? 0 : state.lastSequenceNumber;
  if (
    sequenceNumber !== undefined &&
    sequenceNumber <= lastSeq
  ) {
    return state;
  }

  let next: ConversationStreamState = {
    ...state,
    lastSequenceNumber: sequenceNumber ?? lastSeq,
  };

  if (isResponseFrame(frame)) return reduceResponse(next, frame);
  if (isMessageFrame(frame)) return reduceMessage(next, frame);
  if (isContentFrame(frame)) return reduceContent(next, frame);
  if (isTurnUsageFrame(frame)) {
    return { ...next, turnUsage: frame };
  }
  if (isRateLimitedFrame(frame)) {
    return {
      ...next,
      rateLimited: frame,
      error: frame.error,
      responseStatus: "failed",
    };
  }
  if ("error" in frame) {
    return {
      ...next,
      error: errorMessage(frame.error),
      responseStatus: "failed",
    };
  }
  return next;
}

export function reduceStreamFrames(
  frames: SseFrame[],
  initial: ConversationStreamState = initialConversationStreamState,
): ConversationStreamState {
  return frames.reduce(reduceStreamFrame, initial);
}

function reduceResponse(
  state: ConversationStreamState,
  frame: ResponseFrame,
): ConversationStreamState {
  let next: ConversationStreamState = {
    ...state,
    responseId: frame.id,
    responseStatus: frame.status,
    sessionId: frame.session_id ?? state.sessionId,
    error: frame.error ? errorMessage(frame.error) : state.error,
  };

  if (frame.output?.length) {
    for (const message of frame.output) {
      next = upsertCompletedMessage(next, message);
    }
  }
  return next;
}

function reduceMessage(
  state: ConversationStreamState,
  frame: MessageFrame,
): ConversationStreamState {
  if (frame.metadata?.clear_history === true) {
    if (state.messages.length === 0) return state;
    return {
      ...state,
      messages: [],
      clearHistoryVersion: state.clearHistoryVersion + 1,
    };
  }

  const existingIndex = state.messages.findIndex(
    (message) => message.id === frame.id,
  );
  const existing = state.messages[existingIndex];
  const message: StreamMessage = {
    id: frame.id,
    type: frame.type,
    role: frame.role,
    status: frame.status,
    content:
      frame.content.length > 0
        ? normalizeContent(frame.content)
        : existing?.content ?? [],
    metadata: frame.metadata,
    name: frame.name,
    usage: frame.usage,
  };
  return replaceOrAppendMessage(state, existingIndex, message);
}

function reduceContent(
  state: ConversationStreamState,
  frame: ContentBlock,
): ConversationStreamState {
  if (!frame.msg_id) return state;
  const messageIndex = state.messages.findIndex(
    (message) => message.id === frame.msg_id,
  );
  if (messageIndex < 0) return state;

  const message = state.messages[messageIndex]!;
  const contentIndex = frame.index ?? 0;
  const content = [...message.content];
  const previous = content[contentIndex];
  content[contentIndex] = mergeContent(previous, frame);
  const messages = [...state.messages];
  messages[messageIndex] = { ...message, content };
  return { ...state, messages };
}

function mergeContent(
  previous: ContentBlock | undefined,
  frame: ContentBlock,
): ContentBlock {
  if (frame.type === "text") {
    const previousText = previous?.type === "text" ? previous.text : "";
    return {
      ...frame,
      text: frame.delta ? previousText + frame.text : frame.text,
    };
  }
  if (frame.type !== "data") return frame;

  const oldData =
    previous?.type === "data" ? (previous.data as Record<string, unknown>) : {};
  const incoming = frame.data as Record<string, unknown>;
  const merged = { ...oldData, ...incoming };

  if (typeof incoming.output === "string") {
    merged.output = incoming.output;
  } else if (typeof incoming.arguments === "string") {
    const oldArguments =
      typeof oldData.arguments === "string" ? oldData.arguments : "";
    merged.arguments = frame.delta
      ? oldArguments + incoming.arguments
      : incoming.arguments;
  }
  return { ...frame, data: merged } as DataContent;
}

function upsertCompletedMessage(
  state: ConversationStreamState,
  frame: MessageFrame,
): ConversationStreamState {
  if (frame.metadata?.clear_history === true) {
    if (state.messages.length === 0) return state;
    return {
      ...state,
      messages: [],
      clearHistoryVersion: state.clearHistoryVersion + 1,
    };
  }
  const index = state.messages.findIndex((message) => message.id === frame.id);
  const existing = state.messages[index];
  const message: StreamMessage = {
    id: frame.id,
    type: frame.type,
    role: frame.role,
    status: frame.status,
    content:
      frame.content.length > 0
        ? normalizeContent(frame.content)
        : existing?.content ?? [],
    metadata: frame.metadata,
    name: frame.name,
    usage: frame.usage,
  };
  return replaceOrAppendMessage(state, index, message);
}

function replaceOrAppendMessage(
  state: ConversationStreamState,
  index: number,
  message: StreamMessage,
): ConversationStreamState {
  const messages = [...state.messages];
  if (index >= 0) messages[index] = message;
  else messages.push(message);
  return { ...state, messages };
}

function normalizeContent(content: unknown[]): ContentBlock[] {
  return content.filter(
    (item): item is ContentBlock =>
      Boolean(item) &&
      typeof item === "object" &&
      (item as { object?: unknown }).object === "content",
  );
}

function getSequenceNumber(frame: SseFrame): number | undefined {
  if (!("sequence_number" in frame)) return undefined;
  return typeof frame.sequence_number === "number"
    ? frame.sequence_number
    : undefined;
}

function errorMessage(
  error: string | { code?: string; message?: string },
): string {
  if (typeof error === "string") return error;
  return error.message || error.code || t("stream.requestFailed");
}
