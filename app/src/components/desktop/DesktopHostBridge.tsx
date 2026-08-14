import * as Dialog from "@radix-ui/react-dialog";
import { Download, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "../ui";
import {
  acknowledgeDesktopClose,
  checkCachedDesktopUpdate,
  checkDesktopUpdate,
  downloadDesktopUpdate,
  getRememberedDesktopCloseAction,
  hasDesktopHostBridge,
  installCachedDesktopUpdate,
  installDesktopUpdate,
  listenDesktopEvent,
  runDesktopCloseAction,
  setDesktopTrayLabels,
  setRememberedDesktopCloseAction,
  type DesktopCloseAction,
  type DesktopUpdateError,
  type DesktopUpdateProgress,
} from "../../lib/desktop";
import { useTranslation } from "../../lib/i18n";

const CLOSE_REQUESTED_EVENT = "qwenpaw-close-requested";

type UpdatePhase =
  | "idle"
  | "checking"
  | "downloading"
  | "installing"
  | "failed";
type UpdateSource = "available" | "cached" | null;

interface UpdateState {
  source: UpdateSource;
  version: string;
  supportsLaterInstall: boolean;
  phase: UpdatePhase;
  downloaded: number;
  total: number | null;
  error: DesktopUpdateError | null;
}

const INITIAL_UPDATE_STATE: UpdateState = {
  source: null,
  version: "",
  supportsLaterInstall: false,
  phase: "idle",
  downloaded: 0,
  total: null,
  error: null,
};

/**
 * The desktop WebView navigates from its small Tauri bootstrap page to this
 * backend-hosted app. Keep all native-only integration in one mounted bridge
 * so browser users get neither a runtime dependency nor a broken UI.
 */
export function DesktopHostBridge() {
  const { language, t } = useTranslation();
  const [closeOpen, setCloseOpen] = useState(false);
  const [rememberCloseAction, setRememberCloseAction] = useState(false);
  const [closeAction, setCloseAction] = useState<DesktopCloseAction | null>(
    null,
  );
  const [closeError, setCloseError] = useState(false);
  const [update, setUpdate] = useState<UpdateState>(INITIAL_UPDATE_STATE);
  const [updateDismissed, setUpdateDismissed] = useState(false);

  const executeCloseAction = useCallback(
    async (action: DesktopCloseAction, remember: boolean) => {
      setCloseAction(action);
      setCloseError(false);
      if (remember) setRememberedDesktopCloseAction(action);
      try {
        await runDesktopCloseAction(action);
        if (action === "minimize-to-tray") setCloseOpen(false);
      } catch {
        // The Rust fallback remains available if the bridge is interrupted.
        setCloseError(true);
        setCloseOpen(true);
      } finally {
        setCloseAction(null);
      }
    },
    [],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listenDesktopEvent<void>(CLOSE_REQUESTED_EVENT, () => {
      // Tell Rust the app owns this request before an async UI render happens.
      void acknowledgeDesktopClose().catch(() => {});
      const remembered = getRememberedDesktopCloseAction();
      if (remembered) {
        void executeCloseAction(remembered, false);
        return;
      }
      setCloseError(false);
      setRememberCloseAction(false);
      setCloseOpen(true);
    }).then((cleanup) => {
      if (!cleanup) return;
      if (disposed) cleanup();
      else unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [executeCloseAction]);

  useEffect(() => {
    if (!hasDesktopHostBridge()) return;
    void setDesktopTrayLabels(
      t("desktop.tray.showWindow"),
      t("desktop.close.quit"),
    ).catch(() => {});
  }, [language, t]);

  useEffect(() => {
    if (!hasDesktopHostBridge()) return;
    let disposed = false;
    const cleanups: Array<() => void> = [];

    void checkCachedDesktopUpdate()
      .then((version) => {
        if (disposed || !version) return;
        setUpdate((current) => ({
          ...current,
          source: "cached",
          version,
          supportsLaterInstall: true,
        }));
      })
      // A desktop build may intentionally omit updater endpoints. Initial
      // probing must never make the app unusable in that configuration.
      .catch(() => {});

    void checkDesktopUpdate()
      .then((info) => {
        if (disposed || !info) return;
        setUpdate((current) =>
          current.source === "cached"
            ? current
            : {
                ...current,
                source: "available",
                version: info.version,
                supportsLaterInstall: Boolean(info.supportsLaterInstall),
              },
        );
      })
      .catch(() => {});

    const events = Promise.all([
      listenDesktopEvent<void>("update:check-start", () => {
        setUpdate((current) => ({
          ...current,
          phase: "checking",
          error: null,
        }));
      }),
      listenDesktopEvent<DesktopUpdateProgress>(
        "update:download-progress",
        (progress) => {
          setUpdate((current) => ({
            ...current,
            phase: "downloading",
            downloaded: progress.downloaded,
            total: progress.total ?? null,
          }));
        },
      ),
      listenDesktopEvent<void>("update:install-start", () => {
        setUpdate((current) => ({
          ...current,
          phase: "installing",
          error: null,
        }));
      }),
      listenDesktopEvent<{ version: string }>(
        "update:download-done",
        (payload) => {
          setUpdate((current) => ({
            ...current,
            source: "cached",
            version: payload.version || current.version,
            supportsLaterInstall: true,
            phase: "idle",
          }));
        },
      ),
      listenDesktopEvent<DesktopUpdateError>("update:error", (error) => {
        setUpdate((current) => ({ ...current, phase: "failed", error }));
      }),
    ]);

    void events.then((listeners) => {
      for (const listener of listeners) {
        if (!listener) continue;
        if (disposed) listener();
        else cleanups.push(listener);
      }
    });

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, []);

  const startInstall = useCallback(async () => {
    setUpdateDismissed(false);
    setUpdate((current) => ({ ...current, phase: "checking", error: null }));
    try {
      if (update.source === "cached") await installCachedDesktopUpdate();
      else await installDesktopUpdate();
    } catch (error) {
      setUpdate((current) => ({
        ...current,
        phase: "failed",
        error: {
          stage: "install",
          kind: "other",
          message: errorMessage(error),
        },
      }));
    }
  }, [update.source]);

  const downloadLater = useCallback(async () => {
    setUpdateDismissed(false);
    setUpdate((current) => ({
      ...current,
      phase: "checking",
      downloaded: 0,
      total: null,
      error: null,
    }));
    try {
      await downloadDesktopUpdate();
    } catch (error) {
      setUpdate((current) => ({
        ...current,
        phase: "failed",
        error: {
          stage: "download",
          kind: "other",
          message: errorMessage(error),
        },
      }));
    }
  }, []);

  if (!hasDesktopHostBridge()) return null;

  const updateVisible =
    !updateDismissed &&
    (update.source !== null ||
      update.phase !== "idle" ||
      update.error !== null);

  return (
    <>
      <Dialog.Root open={closeOpen} onOpenChange={() => {}}>
        <Dialog.Portal>
          <Dialog.Overlay className="qp-overlay fixed inset-0 z-[70] bg-overlay backdrop-blur-[1px]" />
          <Dialog.Content
            onEscapeKeyDown={(event) => event.preventDefault()}
            onInteractOutside={(event) => event.preventDefault()}
            className="qp-pop fixed left-1/2 top-1/2 z-[71] w-[calc(100%-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-lg)] border border-line bg-raised p-5 shadow-[var(--shadow-lg)] outline-none"
          >
            <Dialog.Title className="text-sm font-semibold text-ink">
              {t("desktop.close.title")}
            </Dialog.Title>
            <Dialog.Description className="mt-1.5 text-sm leading-6 text-ink-secondary">
              {t("desktop.close.description")}
            </Dialog.Description>
            {closeError && (
              <p role="alert" className="mt-3 text-xs text-danger">
                {t("desktop.close.error")}
              </p>
            )}
            <label className="mt-4 flex cursor-pointer items-center gap-2 text-xs text-ink-secondary">
              <input
                type="checkbox"
                checked={rememberCloseAction}
                disabled={closeAction !== null}
                onChange={(event) =>
                  setRememberCloseAction(event.target.checked)
                }
                className="h-4 w-4 accent-[var(--color-btn-primary)]"
              />
              {t("desktop.close.remember")}
            </label>
            <div className="mt-5 flex justify-end gap-2">
              <Button
                size="sm"
                disabled={closeAction !== null}
                onClick={() =>
                  void executeCloseAction(
                    "minimize-to-tray",
                    rememberCloseAction,
                  )
                }
              >
                {t("desktop.close.minimize")}
              </Button>
              <Button
                variant="danger"
                size="sm"
                disabled={closeAction !== null}
                onClick={() =>
                  void executeCloseAction("quit", rememberCloseAction)
                }
              >
                {closeAction === "quit"
                  ? t("desktop.close.quitting")
                  : t("desktop.close.quit")}
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {updateVisible && (
        <section
          role={update.phase === "failed" ? "alert" : "status"}
          className="fixed bottom-4 left-1/2 z-[60] flex w-[min(32rem,calc(100%-2rem))] -translate-x-1/2 items-start gap-3 rounded-[var(--radius-md)] border border-line bg-raised px-4 py-3 text-sm text-ink shadow-[var(--shadow-lg)]"
        >
          <RefreshCw
            size={16}
            strokeWidth={1.75}
            className={
              update.phase === "checking" || update.phase === "downloading"
                ? "mt-0.5 shrink-0 animate-spin text-accent"
                : "mt-0.5 shrink-0 text-accent"
            }
          />
          <div className="min-w-0 flex-1">
            <p className="font-medium">{updateMessage(update, t)}</p>
            {update.phase === "downloading" && (
              <p className="mt-1 text-xs text-ink-secondary">
                {t("desktop.update.progress", {
                  done: formatBytes(update.downloaded),
                  total: update.total ? formatBytes(update.total) : "—",
                })}
              </p>
            )}
            {update.phase === "failed" && update.error?.message && (
              <p className="mt-1 break-words text-xs text-danger">
                {update.error.message}
              </p>
            )}
            <div className="mt-3 flex flex-wrap gap-2">
              {update.phase === "idle" && (
                <>
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => void startInstall()}
                  >
                    {update.source === "cached"
                      ? t("desktop.update.restart")
                      : t("desktop.update.install")}
                  </Button>
                  {update.source === "available" &&
                    update.supportsLaterInstall && (
                      <Button size="sm" onClick={() => void downloadLater()}>
                        <Download size={14} strokeWidth={1.8} />
                        {t("desktop.update.downloadLater")}
                      </Button>
                    )}
                </>
              )}
              {update.phase === "failed" && (
                <Button size="sm" onClick={() => void startInstall()}>
                  {t("desktop.update.retry")}
                </Button>
              )}
              {update.phase !== "checking" &&
                update.phase !== "downloading" &&
                update.phase !== "installing" && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setUpdateDismissed(true)}
                  >
                    {t("desktop.update.dismiss")}
                  </Button>
                )}
            </div>
          </div>
        </section>
      )}
    </>
  );
}

function updateMessage(
  update: UpdateState,
  t: (
    key:
      | "desktop.update.available"
      | "desktop.update.cached"
      | "desktop.update.checking"
      | "desktop.update.downloading"
      | "desktop.update.installing"
      | "desktop.update.failed",
    params?: Record<string, string | number>,
  ) => string,
): string {
  if (update.phase === "checking") return t("desktop.update.checking");
  if (update.phase === "downloading") return t("desktop.update.downloading");
  if (update.phase === "installing") return t("desktop.update.installing");
  if (update.phase === "failed") return t("desktop.update.failed");
  return update.source === "cached"
    ? t("desktop.update.cached", { version: update.version })
    : t("desktop.update.available", { version: update.version });
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Unknown desktop update error";
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let unit = 0;
  let value = bytes;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}
