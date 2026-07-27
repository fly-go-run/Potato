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
