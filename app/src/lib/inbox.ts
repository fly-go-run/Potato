export interface InboxEvent {
  id: string;
  agent_id: string;
  source_type: string;
  source_id: string;
  event_type: string;
  status: string;
  severity: string;
  title: string;
  body: string;
  payload: Record<string, unknown>;
  read: boolean;
  created_at: number;
}

export interface InboxTraceEvent {
  at: number;
  event: Record<string, unknown>;
}

export interface InboxTrace {
  run_id: string;
  created_at: number;
  completed_at: number | null;
  status: string;
  meta: Record<string, unknown>;
  events: InboxTraceEvent[];
  error?: string;
}

export function countUnread(events: InboxEvent[]): number {
  return events.reduce((count, event) => count + (event.read ? 0 : 1), 0);
}

export function markEventsRead(
  events: InboxEvent[],
  eventIds?: string[],
): InboxEvent[] {
  const ids = eventIds ? new Set(eventIds) : null;
  return events.map((event) =>
    !event.read && (!ids || ids.has(event.id))
      ? { ...event, read: true }
      : event,
  );
}

export function eventRunId(event: InboxEvent): string | null {
  const value = event.payload?.run_id;
  return typeof value === "string" && value ? value : null;
}

/** AutoDream(记忆整理)结果正文的结构化解析;非该格式返回 null。 */
export function summarizeAutoDream(
  body: string,
): { scanned: number; changed: number; units: number } | null {
  if (!/auto[\s-]?dream/i.test(body)) return null;
  const files = /files:\s*(\d+)\s*scanned,\s*(\d+)\s*changed/i.exec(body);
  if (!files) return null;
  const units = /extracted:\s*(\d+)\s*unit/i.exec(body);
  return {
    scanned: Number(files[1]),
    changed: Number(files[2]),
    units: units ? Number(units[1]) : 0,
  };
}

/** 零结果的例行运行:成功且什么都没产出,应聚合降噪而不是逐条轰炸。 */
export function isRoutineEvent(event: InboxEvent): boolean {
  if (event.status !== "success") return false;
  const summary = summarizeAutoDream(event.body);
  return Boolean(summary && summary.changed === 0 && summary.units === 0);
}

export type TraceStep =
  | { kind: "tool"; name: string }
  | { kind: "message" }
  | { kind: "failed"; detail: string }
  | { kind: "raw"; text: string };

/** 协议消息的工具信息藏在 content[].data 里,先挖出来。 */
function traceEventData(
  event: Record<string, unknown>,
): Record<string, unknown> {
  const content = event.content;
  if (!Array.isArray(content)) return {};
  for (const part of content) {
    if (
      part &&
      typeof part === "object" &&
      (part as { type?: unknown }).type === "data" &&
      typeof (part as { data?: unknown }).data === "object" &&
      (part as { data?: unknown }).data !== null
    ) {
      return (part as { data: Record<string, unknown> }).data;
    }
  }
  return {};
}

/** 把后端事件对象归类成用户可读的时间线步骤;认不出的保底走原摘要。 */
export function presentTraceStep(event: Record<string, unknown>): TraceStep {
  const data = traceEventData(event);
  const type = typeof event.type === "string" ? event.type.toLowerCase() : "";
  const status =
    typeof event.status === "string" ? event.status.toLowerCase() : "";
  const state = typeof data.state === "string" ? data.state.toLowerCase() : "";
  if (
    status === "failed" ||
    status === "error" ||
    status === "cancelled" ||
    (state !== "" &&
      state !== "success" &&
      state !== "completed" &&
      state !== "created" &&
      state !== "in_progress")
  ) {
    return { kind: "failed", detail: traceEventSummary(event) };
  }
  // 工具名优先取协议 data.name;`plugin_call` 这类信封类型不当名字用。
  const name =
    typeof data.name === "string"
      ? data.name
      : typeof event.name === "string"
      ? event.name
      : typeof event.tool_name === "string"
      ? event.tool_name
      : "";
  if (name) return { kind: "tool", name };
  const role = typeof event.role === "string" ? event.role : "";
  if (role === "assistant" || type.includes("message") || type === "result") {
    return { kind: "message" };
  }
  return { kind: "raw", text: traceEventSummary(event) };
}

export function traceEventSummary(event: Record<string, unknown>): string {
  const role = typeof event.role === "string" ? event.role : "";
  const content = event.content;
  if (Array.isArray(content)) {
    const text = content
      .map((part) =>
        part &&
        typeof part === "object" &&
        typeof (part as { text?: unknown }).text === "string"
          ? (part as { text: string }).text
          : "",
      )
      .filter(Boolean)
      .join(" ");
    if (text) return role ? `${role}: ${text}` : text;
  }
  const type = typeof event.type === "string" ? event.type : "";
  const status = typeof event.status === "string" ? event.status : "";
  const text = typeof event.text === "string" ? event.text : "";
  if (text) return text;
  if (type || status) return [type, status].filter(Boolean).join(" · ");
  try {
    return JSON.stringify(event);
  } catch {
    return String(event);
  }
}
