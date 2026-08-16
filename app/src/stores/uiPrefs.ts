import { create } from "zustand";

const TOOL_DETAIL_KEY = "potato.toolDetail";
const CONTEXT_USAGE_KEY = "potato.contextUsage";

function initialDetailedTools(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(TOOL_DETAIL_KEY) === "1";
  } catch {
    return false;
  }
}

function initialShowContextUsage(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(CONTEXT_USAGE_KEY) === "1";
  } catch {
    return false;
  }
}

interface UiPrefsState {
  /** 执行轨道信息密度:true = 详细档(逐条状态图标、失败细节常显)。 */
  detailedTools: boolean;
  setDetailedTools: (value: boolean) => void;
  /**
   * 输入框旁是否常显「上下文已用 x%」。默认关:绝大多数会话这个数字停在
   * 个位数,对用户没有可操作性,只是在输入框边上持续占一块注意力。
   */
  showContextUsage: boolean;
  setShowContextUsage: (value: boolean) => void;
}

/** 轻量 UI 偏好,跨轨道/会话即时生效,localStorage 持久化。 */
export const useUiPrefs = create<UiPrefsState>((set) => ({
  detailedTools: initialDetailedTools(),
  setDetailedTools: (value) => {
    try {
      window.localStorage.setItem(TOOL_DETAIL_KEY, value ? "1" : "0");
    } catch {
      // 存不进去就只在本次会话生效。
    }
    set({ detailedTools: value });
  },
  showContextUsage: initialShowContextUsage(),
  setShowContextUsage: (value) => {
    try {
      window.localStorage.setItem(CONTEXT_USAGE_KEY, value ? "1" : "0");
    } catch {
      // 存不进去就只在本次会话生效。
    }
    set({ showContextUsage: value });
  },
}));

/**
 * C 端默认只需要知道「正在处理」或「已完成」，逐条成功/失败属于调试
 * 细节。开发构建以及显式打开 debug=tools 时保留状态，方便定位工具链问题。
 */
export function showToolDebugStatus(): boolean {
  if (import.meta.env.DEV) return true;
  if (typeof window === "undefined") return false;
  try {
    const locationText = `${window.location.search}${window.location.hash}`;
    return (
      /(?:[?&])debug=tools(?:&|$)/i.test(locationText) ||
      window.localStorage.getItem("potato.toolDebug") === "1"
    );
  } catch {
    return false;
  }
}

/** 用户「详细」档与开发 debug 开关的合并结果,组件统一从这里取。 */
export function useToolDetail(): boolean {
  const detailed = useUiPrefs((state) => state.detailedTools);
  return detailed || showToolDebugStatus();
}
