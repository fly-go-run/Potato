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
}

function tauriInternals(): TauriInternals | null {
  const holder = window as unknown as { __TAURI_INTERNALS__?: TauriInternals };
  return holder.__TAURI_INTERNALS__ ?? null;
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
