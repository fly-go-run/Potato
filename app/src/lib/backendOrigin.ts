import {
  getDesktopBackendPort,
  getDesktopBackendStartupError,
  isDesktopShell,
  restartDesktopBackend,
} from "./desktop";

export const BACKEND_POLL_INTERVAL_MS = 250;
export const BACKEND_POLL_TIMEOUT_SECONDS = 180;
export const BACKEND_VERSION_TIMEOUT_MS = 2500;

type ResolverState = "idle" | "connecting" | "ready" | "error";
type Waiter = {
  resolve: (nextOrigin: string) => void;
  reject: (error: Error) => void;
};

let origin = "";
let resolverState: ResolverState = "idle";
let pollGeneration = 0;
let waiters: Waiter[] = [];

export function getBackendOrigin(): string {
  return origin;
}

export function setBackendOrigin(next: string): void {
  settleReady(next);
}

export function resetBackendOrigin(): void {
  pollGeneration += 1;
  origin = "";
  resolverState = "idle";
  rejectWaiters(new Error("backend origin reset"));
}

export function backendHttpOriginFromPort(port: number): string {
  return `http://127.0.0.1:${port}`;
}

/** Absolute http(s) / data / blob URLs stay untouched. */
export function resolveBackendUrl(path: string): string {
  if (!path || /^[a-z][a-z\d+\-.]*:/i.test(path)) return path;
  if (!path.startsWith("/")) return path;
  return origin ? `${origin}${path}` : path;
}

export function isBackendHostedConsole(): boolean {
  if (typeof window === "undefined") return false;
  const { protocol, hostname, pathname } = window.location;
  return (
    protocol === "http:" &&
    (hostname === "127.0.0.1" || hostname === "localhost") &&
    /^\/console(?:\/|$)/.test(pathname)
  );
}

/** Bundled / Vite-hosted desktop UI must prefix API calls with the sidecar origin. */
export function needsDesktopBackendOrigin(): boolean {
  return isDesktopShell() && !isBackendHostedConsole();
}

export async function probeBackendVersion(
  nextOrigin: string,
  signal?: AbortSignal,
): Promise<boolean> {
  const controller = new AbortController();
  const timer = setTimeout(
    () => controller.abort(),
    BACKEND_VERSION_TIMEOUT_MS,
  );
  const onAbort = () => controller.abort();
  signal?.addEventListener("abort", onAbort);
  try {
    const response = await fetch(`${nextOrigin}/api/version`, {
      signal: controller.signal,
      cache: "no-store",
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", onAbort);
  }
}

/** Start discovering the sidecar in the background. Never blocks first paint. */
export function ensureBackendOriginResolver(): void {
  if (!needsDesktopBackendOrigin()) {
    if (resolverState === "idle") resolverState = "ready";
    return;
  }
  if (resolverState === "connecting" || resolverState === "ready") return;
  resolverState = "connecting";
  const generation = ++pollGeneration;
  void pollBackendOrigin(generation);
}

/**
 * Resolve when the sidecar is healthy. Browser / backend-hosted pages
 * return immediately. After a hard failure, the next wait restarts the sidecar.
 */
export function waitForBackendOrigin(
  timeoutMs = BACKEND_POLL_TIMEOUT_SECONDS * 1000,
): Promise<string> {
  if (resolverState === "error") {
    resolverState = "idle";
    void restartDesktopBackend().catch(() => undefined);
  }
  ensureBackendOriginResolver();
  if (!needsDesktopBackendOrigin() || resolverState === "ready") {
    return Promise.resolve(origin);
  }
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      waiters = waiters.filter((waiter) => waiter.resolve !== wrappedResolve);
      reject(
        new Error(
          `backend did not become healthy within ${Math.round(timeoutMs / 1000)} seconds`,
        ),
      );
    }, timeoutMs);
    const wrappedResolve = (value: string) => {
      clearTimeout(timer);
      resolve(value);
    };
    const wrappedReject = (error: Error) => {
      clearTimeout(timer);
      reject(error);
    };
    waiters.push({ resolve: wrappedResolve, reject: wrappedReject });
  });
}

function settleReady(nextOrigin: string): void {
  origin = nextOrigin.replace(/\/+$/, "");
  resolverState = "ready";
  const pending = waiters;
  waiters = [];
  pending.forEach((waiter) => waiter.resolve(origin));
}

function settleError(message: string): void {
  resolverState = "error";
  rejectWaiters(new Error(message));
}

function rejectWaiters(error: Error): void {
  const pending = waiters;
  waiters = [];
  pending.forEach((waiter) => waiter.reject(error));
}

async function pollBackendOrigin(generation: number): Promise<void> {
  const startedAt = Date.now();
  while (generation === pollGeneration) {
    const nativeError = await getDesktopBackendStartupError().catch(() => "");
    if (generation !== pollGeneration) return;
    if (nativeError) {
      settleError(nativeError);
      return;
    }

    const port = await getDesktopBackendPort().catch(() => null);
    if (generation !== pollGeneration) return;
    if (port) {
      const nextOrigin = backendHttpOriginFromPort(port);
      const healthy = await probeBackendVersion(nextOrigin);
      if (generation !== pollGeneration) return;
      if (healthy) {
        settleReady(nextOrigin);
        return;
      }
    }

    if ((Date.now() - startedAt) / 1000 >= BACKEND_POLL_TIMEOUT_SECONDS) {
      settleError(
        `backend did not become healthy within ${BACKEND_POLL_TIMEOUT_SECONDS} seconds`,
      );
      return;
    }

    await new Promise((resolve) =>
      setTimeout(resolve, BACKEND_POLL_INTERVAL_MS),
    );
  }
}
