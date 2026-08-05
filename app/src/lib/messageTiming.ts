import type { ConversationStreamState } from "./stream";

export interface MessageTiming {
  startedAt: number;
  endedAt: number | null;
}

const messageTimings = new Map<string, MessageTiming>();

export function trackMessageTimings(
  previous: ConversationStreamState,
  next: ConversationStreamState,
  now: number = Date.now(),
): void {
  const previousById = new Map(
    previous.messages.map((message) => [message.id, message]),
  );

  for (const message of next.messages) {
    const previousMessage = previousById.get(message.id);
    if (previousMessage === message) continue;

    if (!previousMessage) {
      if (
        !messageTimings.has(message.id) &&
        (message.status === "created" || message.status === "in_progress")
      ) {
        messageTimings.set(message.id, { startedAt: now, endedAt: null });
      }
      continue;
    }

    const timing = messageTimings.get(message.id);
    if (
      timing &&
      timing.endedAt === null &&
      previousMessage.status !== message.status &&
      (message.status === "completed" ||
        message.status === "failed" ||
        message.status === "cancelled")
    ) {
      timing.endedAt = now;
    }
  }
}

export function getMessageTiming(id: string): MessageTiming | null {
  return messageTimings.get(id) ?? null;
}

export function resetMessageTimings(): void {
  messageTimings.clear();
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "";

  if (ms < 10_000) {
    return `${Math.max(ms / 1_000, 0.1).toFixed(1)}s`;
  }
  if (ms < 60_000) {
    return `${Math.floor(ms / 1_000)}s`;
  }
  if (ms < 3_600_000) {
    const totalSeconds = Math.floor(ms / 1_000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  }

  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${hours}h ${String(minutes).padStart(2, "0")}m`;
}
