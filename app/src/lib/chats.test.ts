import { describe, expect, it } from "vitest";
import type { ChatSpec } from "./api";
import { filterChats, sortChats } from "./chats";

function chat(
  id: string,
  name: string,
  pinned: boolean,
  updatedAt: string,
): ChatSpec {
  return {
    id,
    name,
    pinned,
    updated_at: updatedAt,
    created_at: updatedAt,
    session_id: `session-${id}`,
    user_id: "default",
    channel: "console",
    status: "idle",
  };
}

describe("chat sorting and search", () => {
  const chats = [
    chat("recent", "Release Notes", false, "2026-07-27T10:00:00Z"),
    chat("pinned", "Release Plan", true, "2026-07-26T10:00:00Z"),
    chat("older", "Design review", false, "2026-07-25T10:00:00Z"),
  ];

  it("sorts pinned chats first, then by latest update", () => {
    expect(sortChats(chats).map((item) => item.id)).toEqual([
      "pinned",
      "recent",
      "older",
    ]);
  });

  it("filters names immediately and preserves sidebar order", () => {
    const sorted = sortChats(chats);
    expect(filterChats(sorted, "  RELEASE ").map((item) => item.id)).toEqual([
      "pinned",
      "recent",
    ]);
    expect(filterChats(sorted, "")).toEqual(sorted);
  });
});
