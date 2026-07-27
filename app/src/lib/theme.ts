import { useEffect } from "react";

export type ThemePreference = "light" | "dark" | "system";

const STORAGE_KEY = "qwenpaw_theme";

export function getThemePreference(): ThemePreference {
  const v = localStorage.getItem(STORAGE_KEY);
  return v === "light" || v === "dark" ? v : "system";
}

export function setThemePreference(pref: ThemePreference) {
  if (pref === "system") {
    localStorage.removeItem(STORAGE_KEY);
  } else {
    localStorage.setItem(STORAGE_KEY, pref);
  }
  applyTheme(pref);
}

function applyTheme(pref: ThemePreference) {
  const dark =
    pref === "dark" ||
    (pref === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

/** 在 App 根组件调用一次：初始化主题并跟随系统变化。 */
export function useThemeInit() {
  useEffect(() => {
    applyTheme(getThemePreference());
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (getThemePreference() === "system") applyTheme("system");
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
}
