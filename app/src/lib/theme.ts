import { useEffect } from "react";

export type ThemePreference = "light" | "dark" | "system";

const STORAGE_KEY = "qwenpaw_theme";
const CUSTOM_THEMES_KEY = "qwenpaw_custom_themes";
const CUSTOM_PREFIX = "custom:";

/**
 * 自定义主题:base 决定明暗骨架(dark class),tokens 覆盖设计变量。
 * token 名不带 `--` 前缀,对应 `styles/tokens.css` 里的变量
 * (bg/surface/ink/accent/line/shadow-md/radius-md/…)。
 */
export interface CustomTheme {
  id: string;
  name: string;
  base: "light" | "dark";
  tokens: Record<string, string>;
}

const TOKEN_NAME_PATTERN = /^[a-z][a-z0-9-]{0,40}$/;
// 禁止能逃出 CSS 属性值上下文的字符,防主题文件夹带样式注入。
const TOKEN_VALUE_PATTERN = /^[^;{}<>]{1,200}$/;
const MAX_TOKENS = 120;

export function getThemePreference(): ThemePreference {
  const v = localStorage.getItem(STORAGE_KEY);
  if (v === "light" || v === "dark") return v;
  if (v?.startsWith(CUSTOM_PREFIX)) {
    const theme = findCustomTheme(v.slice(CUSTOM_PREFIX.length));
    return theme?.base ?? "system";
  }
  return "system";
}

export function setThemePreference(pref: ThemePreference) {
  // 选择基础主题即退出自定义主题。
  if (pref === "system") {
    localStorage.removeItem(STORAGE_KEY);
  } else {
    localStorage.setItem(STORAGE_KEY, pref);
  }
  applyThemeState();
}

export function listCustomThemes(): CustomTheme[] {
  try {
    const parsed = JSON.parse(
      localStorage.getItem(CUSTOM_THEMES_KEY) ?? "[]",
    ) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isCustomTheme);
  } catch {
    return [];
  }
}

export function getActiveCustomThemeId(): string | null {
  const v = localStorage.getItem(STORAGE_KEY);
  if (!v?.startsWith(CUSTOM_PREFIX)) return null;
  const id = v.slice(CUSTOM_PREFIX.length);
  return findCustomTheme(id) ? id : null;
}

export function setActiveCustomTheme(id: string | null) {
  if (id === null) {
    // 回落系统主题
    localStorage.removeItem(STORAGE_KEY);
  } else if (findCustomTheme(id)) {
    localStorage.setItem(STORAGE_KEY, `${CUSTOM_PREFIX}${id}`);
  }
  applyThemeState();
}

/** 解析并校验主题文件;不合法时抛出带原因的 Error。成功后已持久化。 */
export function importCustomTheme(raw: string): CustomTheme {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("not valid JSON");
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("root must be an object");
  }
  const candidate = parsed as Record<string, unknown>;
  const name = typeof candidate.name === "string" ? candidate.name.trim() : "";
  if (!name || name.length > 60) {
    throw new Error("`name` must be a 1-60 char string");
  }
  const base = candidate.base;
  if (base !== "light" && base !== "dark") {
    throw new Error('`base` must be "light" or "dark"');
  }
  const tokensRaw = candidate.tokens;
  if (!tokensRaw || typeof tokensRaw !== "object" || Array.isArray(tokensRaw)) {
    throw new Error("`tokens` must be an object");
  }
  const entries = Object.entries(tokensRaw as Record<string, unknown>);
  if (entries.length === 0) throw new Error("`tokens` is empty");
  if (entries.length > MAX_TOKENS) {
    throw new Error(`too many tokens (max ${MAX_TOKENS})`);
  }
  const tokens: Record<string, string> = {};
  for (const [key, value] of entries) {
    const normalized = key.replace(/^--/, "");
    if (!TOKEN_NAME_PATTERN.test(normalized)) {
      throw new Error(`invalid token name: ${key}`);
    }
    if (typeof value !== "string" || !TOKEN_VALUE_PATTERN.test(value)) {
      throw new Error(`invalid value for token: ${key}`);
    }
    tokens[normalized] = value;
  }

  const theme: CustomTheme = {
    id: crypto.randomUUID(),
    name,
    base,
    tokens,
  };
  const themes = listCustomThemes();
  saveCustomThemes([...themes, theme]);
  return theme;
}

export function removeCustomTheme(id: string) {
  saveCustomThemes(listCustomThemes().filter((theme) => theme.id !== id));
  if (localStorage.getItem(STORAGE_KEY) === `${CUSTOM_PREFIX}${id}`) {
    localStorage.removeItem(STORAGE_KEY);
  }
  applyThemeState();
}

/** 以当前生效值生成主题模板,用户照着改数值即可。 */
export function buildThemeTemplate(): string {
  const style = getComputedStyle(document.documentElement);
  const sample = [
    "canvas",
    "bg",
    "surface",
    "raised",
    "ink",
    "ink-secondary",
    "ink-tertiary",
    "ink-muted",
    "line",
    "line-strong",
    "fill-hover",
    "fill-active",
    "accent",
    "accent-hover",
    "btn-primary",
    "btn-primary-hover",
    "btn-primary-ink",
    "bubble-user",
    "bubble-tool",
    "ok",
    "warn",
    "danger",
    "danger-soft",
  ];
  const tokens: Record<string, string> = {};
  for (const name of sample) {
    const value = style.getPropertyValue(`--${name}`).trim();
    if (value) tokens[name] = value;
  }
  return JSON.stringify(
    { name: "My Theme", base: isDarkActive() ? "dark" : "light", tokens },
    null,
    2,
  );
}

function isCustomTheme(value: unknown): value is CustomTheme {
  if (!value || typeof value !== "object") return false;
  const theme = value as Record<string, unknown>;
  return (
    typeof theme.id === "string" &&
    typeof theme.name === "string" &&
    (theme.base === "light" || theme.base === "dark") &&
    Boolean(theme.tokens) &&
    typeof theme.tokens === "object"
  );
}

function findCustomTheme(id: string): CustomTheme | null {
  return listCustomThemes().find((theme) => theme.id === id) ?? null;
}

function saveCustomThemes(themes: CustomTheme[]) {
  localStorage.setItem(CUSTOM_THEMES_KEY, JSON.stringify(themes));
}

function isDarkActive(): boolean {
  return document.documentElement.classList.contains("dark");
}

/** 已应用的覆盖 token,切主题时先清掉,避免残留。 */
let appliedTokens: string[] = [];

function clearTokenOverrides() {
  for (const name of appliedTokens) {
    document.documentElement.style.removeProperty(`--${name}`);
  }
  appliedTokens = [];
}

function applyTokenOverrides(tokens: Record<string, string>) {
  for (const [name, value] of Object.entries(tokens)) {
    document.documentElement.style.setProperty(`--${name}`, value);
  }
  appliedTokens = Object.keys(tokens);
}

function applyThemeState() {
  const activeId = getActiveCustomThemeId();
  const custom = activeId ? findCustomTheme(activeId) : null;
  const pref = getThemePreference();
  const dark = custom
    ? custom.base === "dark"
    : pref === "dark" ||
      (pref === "system" &&
        window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
  clearTokenOverrides();
  if (custom) applyTokenOverrides(custom.tokens);
}

/** 在 App 根组件调用一次：初始化主题并跟随系统变化。 */
export function useThemeInit() {
  useEffect(() => {
    applyThemeState();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (
        !getActiveCustomThemeId() &&
        localStorage.getItem(STORAGE_KEY) === null
      ) {
        applyThemeState();
      }
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
}
