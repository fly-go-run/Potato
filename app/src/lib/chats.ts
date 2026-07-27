import type { ChatSpec } from "./api";

export function sortChats(chats: ChatSpec[]): ChatSpec[] {
  return [...chats].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return Date.parse(b.updated_at) - Date.parse(a.updated_at);
  });
}

export function filterChats(chats: ChatSpec[], query: string): ChatSpec[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return chats;
  return chats.filter((chat) =>
    chat.name.toLocaleLowerCase().includes(normalized),
  );
}
