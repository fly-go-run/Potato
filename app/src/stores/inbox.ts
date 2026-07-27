import { create } from "zustand";
import { inboxApi } from "../lib/api";

interface InboxStore {
  unreadCount: number;
  unreadLoading: boolean;
  refreshUnread: () => Promise<void>;
  setUnreadCount: (count: number) => void;
}

export const useInboxStore = create<InboxStore>((set) => ({
  unreadCount: 0,
  unreadLoading: false,
  refreshUnread: async () => {
    set({ unreadLoading: true });
    try {
      const response = await inboxApi.events({
        unreadOnly: true,
        limit: 500,
      });
      set({ unreadCount: response.events.length, unreadLoading: false });
    } catch {
      set({ unreadLoading: false });
    }
  },
  setUnreadCount: (unreadCount) =>
    set({ unreadCount: Math.max(0, unreadCount) }),
}));
