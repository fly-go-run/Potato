/* 桌面壳环境检测。打包后的 Tauri 页走 tauri:// / tauri.localhost；
 * `tauri dev` 注入 __TAURI_INTERNALS__；旧的 `?desktop=1` 标记仍识别。 */

export function isDesktopShell(): boolean {
  if (typeof window === "undefined") return false;
  if (tauriInternals() !== null) return true;
  const { protocol, hostname } = window.location;
  if (protocol === "tauri:" || hostname === "tauri.localhost") return true;
  try {
    return new URLSearchParams(window.location.search).get("desktop") === "1";
  } catch {
    return false;
  }
}

/** macOS 桌面壳:无标题栏 Overlay 模式,需给左上角红绿灯让位并提供拖拽区 */
export function isMacDesktopShell(): boolean {
  return isDesktopShell() && /Mac/i.test(navigator.platform);
}

/* Tauri v2 对授权页面(capabilities.remote.urls)注入 __TAURI_INTERNALS__,
 * 官方 plugin JS 包也是包着它调 invoke;这里直接用避免给 web 构建带上依赖。 */
interface TauriInternals {
  invoke: (cmd: string, args?: unknown) => Promise<unknown>;
  transformCallback?: (
    callback: (event: unknown) => void,
    once?: boolean,
  ) => number;
}

function tauriInternals(): TauriInternals | null {
  if (typeof window === "undefined") return null;
  const holder = window as unknown as { __TAURI_INTERNALS__?: TauriInternals };
  return holder.__TAURI_INTERNALS__ ?? null;
}

interface TauriEventPluginInternals {
  unregisterListener?: (event: string, eventId: number) => void;
}

export interface DesktopUpdateInfo {
  version: string;
  body?: string | null;
  supportsLaterInstall?: boolean;
}

export interface DesktopUpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface DesktopUpdateError {
  stage: "check" | "download" | "install";
  kind: "network" | "signature" | "appLocation" | "other";
  message: string;
}

/** The backend-hosted app deliberately has no Tauri npm dependency.  Keep the
 * small bridge here so a normal browser build simply degrades to a no-op. */
export function hasDesktopHostBridge(): boolean {
  return isDesktopShell() && tauriInternals() !== null;
}

async function invokeDesktop<T>(
  command: string,
  args?: unknown,
): Promise<T | null> {
  const internals = tauriInternals();
  if (!isDesktopShell() || !internals) return null;
  return (await internals.invoke(command, args)) as T;
}

/** Listen without importing @tauri-apps/api into the backend-hosted bundle. */
export async function listenDesktopEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<(() => void) | null> {
  const internals = tauriInternals();
  if (!isDesktopShell() || !internals?.transformCallback) return null;

  const callbackId = internals.transformCallback((rawEvent) => {
    const payload = (rawEvent as { payload?: T }).payload;
    handler(payload as T);
  });

  let eventId: number;
  try {
    eventId = await invokeDesktop<number>("plugin:event|listen", {
      event,
      target: { kind: "Any" },
      handler: callbackId,
    }).then((value) => {
      if (typeof value !== "number") {
        throw new Error("desktop event listener was not registered");
      }
      return value;
    });
  } catch {
    return null;
  }

  return () => {
    const holder = window as unknown as {
      __TAURI_EVENT_PLUGIN_INTERNALS__?: TauriEventPluginInternals;
    };
    holder.__TAURI_EVENT_PLUGIN_INTERNALS__?.unregisterListener?.(
      event,
      eventId,
    );
    void invokeDesktop("plugin:event|unlisten", { event, eventId });
  };
}

export async function acknowledgeDesktopClose(): Promise<void> {
  await invokeDesktop("ack_close");
}

export async function getDesktopBackendPort(): Promise<number | null> {
  const port = await invokeDesktop<number | null>("backend_port");
  return typeof port === "number" && port > 0 ? port : null;
}

export async function getDesktopBackendStartupError(): Promise<string> {
  return (await invokeDesktop<string | null>("backend_startup_error")) || "";
}

export async function restartDesktopBackend(): Promise<void> {
  await invokeDesktop("restart_backend");
}

export type DesktopCloseAction = "minimize-to-tray" | "quit";

const CLOSE_ACTION_STORAGE_KEY = "potato_desktop_close_action";

export function getRememberedDesktopCloseAction(): DesktopCloseAction | null {
  try {
    const value = globalThis.localStorage?.getItem(CLOSE_ACTION_STORAGE_KEY);
    return value === "minimize-to-tray" || value === "quit" ? value : null;
  } catch {
    return null;
  }
}

export function setRememberedDesktopCloseAction(
  action: DesktopCloseAction,
): void {
  try {
    globalThis.localStorage?.setItem(CLOSE_ACTION_STORAGE_KEY, action);
  } catch {
    // The action still proceeds when storage is unavailable.
  }
}

export async function runDesktopCloseAction(
  action: DesktopCloseAction,
): Promise<void> {
  await invokeDesktop(action === "quit" ? "quit_app" : "minimize_to_tray");
}

export async function setDesktopTrayLabels(
  showWindow: string,
  quit: string,
): Promise<void> {
  await invokeDesktop("set_tray_labels", { showWindow, quit });
}

export interface DesktopWindowStatePreference {
  remember: boolean;
  width?: number | null;
  height?: number | null;
  maximized: boolean;
}

export async function getDesktopWindowStatePreference(): Promise<DesktopWindowStatePreference | null> {
  return invokeDesktop<DesktopWindowStatePreference>(
    "get_window_state_preference",
  );
}

export async function setDesktopWindowStatePreference(
  remember: boolean,
): Promise<boolean> {
  try {
    await invokeDesktop("set_window_state_preference", { remember });
    return hasDesktopHostBridge();
  } catch {
    return false;
  }
}

export async function resetDesktopWindowState(): Promise<boolean> {
  try {
    await invokeDesktop("reset_window_state");
    return hasDesktopHostBridge();
  } catch {
    return false;
  }
}

export async function checkDesktopUpdate(): Promise<DesktopUpdateInfo | null> {
  return invokeDesktop<DesktopUpdateInfo>("check_desktop_update");
}

export async function checkCachedDesktopUpdate(): Promise<string | null> {
  return invokeDesktop<string>("check_cached_update");
}

export async function installDesktopUpdate(): Promise<void> {
  await invokeDesktop("install_desktop_update");
}

export async function downloadDesktopUpdate(): Promise<void> {
  await invokeDesktop("download_desktop_update");
}

export async function installCachedDesktopUpdate(): Promise<void> {
  await invokeDesktop("install_downloaded_update");
}

/** Native window fullscreen (macOS green button / Windows F11), not the
 * web Fullscreen API. Used to drop the traffic-light inset when the lights
 * are hidden. */
export async function getDesktopFullscreen(): Promise<boolean> {
  try {
    const value = await invokeDesktop<boolean>("plugin:window|is_fullscreen");
    return value === true;
  } catch {
    return false;
  }
}

/** Start native window dragging from a custom overlay titlebar. The CSS
 * drag-region remains as a fast path, but invoking the window plugin here
 * also covers WebKit builds where pointer events land on nested text nodes. */
export function startDesktopWindowDrag(): void {
  if (!isMacDesktopShell()) return;
  const internals = tauriInternals();
  if (!internals) return;
  void internals.invoke("plugin:window|start_dragging").catch(() => {
    // Browser/dev builds and older shells may not expose the window plugin.
  });
}

let readyNotified = false;
let readyRequest: Promise<void> | null = null;

/**
 * Acknowledge first paint so later backend restarts cannot steal focus.
 * The window itself is revealed as soon as this bundled app loads.
 */
export async function notifyDesktopReady(): Promise<void> {
  if (readyNotified || !isDesktopShell()) return;
  const internals = tauriInternals();
  if (!internals) return;
  if (readyRequest) return readyRequest;
  readyRequest = (async () => {
    for (const delay of [0, 120, 360]) {
      if (delay > 0) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, delay));
      }
      try {
        await internals.invoke("frontend_ready");
        readyNotified = true;
        return;
      } catch {
        // 原生导航完成的瞬间 invoke bridge 可能尚未稳定，短暂重试。
      }
    }
  })().finally(() => {
    readyRequest = null;
  });
  return readyRequest;
}

/** 绝对本地路径(POSIX 或 Windows 盘符)才允许交给系统打开。 */
export function isAbsoluteLocalPath(path: string): boolean {
  return path.startsWith("/") || /^[a-z]:[\\/]/i.test(path);
}

/** 渲染期同步判断:该路径能否走「系统默认应用打开 / 文件管理器显示」。 */
export function canOpenLocalPathWithSystem(path: string): boolean {
  return isAbsoluteLocalPath(path) && hasDesktopHostBridge();
}

/**
 * 桌面壳内用系统默认应用打开本地文件。返回 false(浏览器模式、相对
 * 路径、文件已不存在等)时由调用方回落应用内预览。
 */
export async function openLocalPathWithSystem(path: string): Promise<boolean> {
  return invokeLocalFileCommand("open_local_path", path);
}

/** 桌面壳内在系统文件管理器(Finder/资源管理器)中定位文件。 */
export async function revealLocalPathInFileManager(
  path: string,
): Promise<boolean> {
  return invokeLocalFileCommand("reveal_local_path", path);
}

/**
 * `target="_blank"` 文件预览链接的桌面壳增强:普通左键且可系统打开时
 * 拦截,改交系统默认应用;浏览器模式与修饰键点击保持原新窗口预览。
 */
export function handleSystemOpenClick(
  event: {
    button: number;
    metaKey: boolean;
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    preventDefault: () => void;
  },
  path: string,
): void {
  if (
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return;
  }
  if (!canOpenLocalPathWithSystem(path)) return;
  event.preventDefault();
  void openLocalPathWithSystem(path);
}

async function invokeLocalFileCommand(
  command: "open_local_path" | "reveal_local_path",
  path: string,
): Promise<boolean> {
  if (!canOpenLocalPathWithSystem(path)) return false;
  try {
    await invokeDesktop(command, { path });
    return true;
  } catch {
    return false;
  }
}

/** 壳内原生对话框可用(dialog 插件已注册,capability 已授 dialog:allow-open) */
export function hasNativeDialogs(): boolean {
  return isDesktopShell() && tauriInternals() !== null;
}

/** 系统原生目录选择器;返回绝对路径,取消或不可用返回 null */
export async function pickDirectoryNative(): Promise<string | null> {
  const internals = tauriInternals();
  if (!internals) return null;
  try {
    const result = await internals.invoke("plugin:dialog|open", {
      options: { directory: true, multiple: false, recursive: false },
    });
    return typeof result === "string" ? result : null;
  } catch {
    return null;
  }
}
