import { create } from "zustand";

/**
 * 全局 UI 外壳状态（与业务数据无关）：目前只有侧栏收起态。
 * chat/inbox store 都是数据域，这类纯外观状态单独放，避免污染。
 */
const SIDEBAR_COLLAPSED_KEY = "qwenpaw_sidebar_collapsed";

function readCollapsed(): boolean {
  if (typeof globalThis.localStorage?.getItem !== "function") return false;
  return globalThis.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
}

function persistCollapsed(collapsed: boolean) {
  if (typeof globalThis.localStorage?.setItem !== "function") return;
  globalThis.localStorage.setItem(SIDEBAR_COLLAPSED_KEY, collapsed ? "1" : "0");
}

interface UiStore {
  sidebarCollapsed: boolean;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
}

export const useUiStore = create<UiStore>((set, get) => ({
  sidebarCollapsed: readCollapsed(),
  setSidebarCollapsed: (collapsed) => {
    persistCollapsed(collapsed);
    set({ sidebarCollapsed: collapsed });
  },
  toggleSidebar: () => {
    get().setSidebarCollapsed(!get().sidebarCollapsed);
  },
}));
