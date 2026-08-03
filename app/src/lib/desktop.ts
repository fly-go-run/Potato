/* 桌面壳环境检测。Tauri 壳往后端托管页导航时会带 `?desktop=1`
 * (console/src/tauri/backendRuntime.ts 的 withDesktopMarker),
 * SPA 内部路由只改 hash,search 全程保留,可直接同步读取。 */

export function isDesktopShell(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).get("desktop") === "1";
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

export type DesktopCloseAction = "minimize-to-tray" | "quit";

const CLOSE_ACTION_STORAGE_KEY = "qwenpaw_desktop_close_action";

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
 * Normal desktop startup stays native-hidden until auth and the first store
 * initialization are complete. The guard prevents route changes or StrictMode
 * effect replays from stealing focus later in the session.
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
