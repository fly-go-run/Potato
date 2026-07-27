import * as Dialog from "@radix-ui/react-dialog";
import {
  ChevronRight,
  FileText,
  LoaderCircle,
  NotebookPen,
  Pencil,
  Save,
  X,
} from "lucide-react";
import { useEffect, useMemo, useReducer, useState } from "react";
import { Markdown } from "../components/chat/Markdown";
import {
  formatFileSize,
  formatRelativeTime,
  groupMemoryFiles,
  initialMemoryEditorState,
  memoryApi,
  memoryDisplayName,
  memoryEditorReducer,
  type MdFileInfo,
  type MemoryGroupKey,
} from "../lib/memory";
import { useTranslation, type TranslationKey } from "../lib/i18n";

const GROUP_LABEL_KEYS: Record<MemoryGroupKey, TranslationKey> = {
  journal: "memory.group.journal",
  procedure: "memory.group.procedure",
  wiki: "memory.group.wiki",
  other: "memory.group.other",
};

export function MemoryView() {
  const { language, t } = useTranslation();
  const [files, setFiles] = useState<MdFileInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<MdFileInfo | null>(null);
  const groups = useMemo(() => groupMemoryFiles(files), [files]);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void memoryApi
      .list(controller.signal)
      .then(setFiles)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(
            t("memory.loadFailed", { message: readableError(reason) }),
          );
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, []);

  const refreshFiles = async () => {
    try {
      setFiles(await memoryApi.list());
    } catch (reason) {
      setError(t("memory.loadFailed", { message: readableError(reason) }));
    }
  };

  return (
    <div className="h-full overflow-y-auto bg-surface">
      <div className="mx-auto max-w-4xl px-6 py-8 sm:px-10">
        <header>
          <h1 className="text-2xl font-medium tracking-tight text-ink">
            {t("memory.title")}
          </h1>
          <p className="mt-1 text-sm text-ink-muted">
            {t("memory.subtitle")}
          </p>
        </header>

        {error && (
          <div
            role="alert"
            className="mt-5 flex items-start justify-between gap-3 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger"
          >
            <span>{error}</span>
            <button
              type="button"
              title={t("memory.dismiss")}
              onClick={() => setError(null)}
              className="shrink-0 rounded-sm p-0.5 hover:bg-surface/50"
            >
              <X size={14} />
            </button>
          </div>
        )}

        {loading ? (
          <div className="mt-6 flex items-center justify-center gap-2 rounded-lg border border-line py-16 text-sm text-ink-muted">
            <LoaderCircle size={16} className="animate-spin" />
            {t("memory.loading")}
          </div>
        ) : files.length === 0 ? (
          <div className="mt-6 flex flex-col items-center rounded-lg border border-dashed border-line px-6 py-16 text-center text-ink-muted">
            <NotebookPen size={28} />
            <h2 className="mt-4 font-medium text-ink">
              {t("memory.emptyTitle")}
            </h2>
            <p className="mt-1 max-w-sm text-sm">
              {t("memory.emptyDescription")}
            </p>
          </div>
        ) : (
          <div className="mt-8 space-y-8">
            {groups.map((group) => (
              <section
                key={group.key}
                className="grid gap-2 sm:grid-cols-[7rem_minmax(0,1fr)] sm:gap-5"
              >
                <header className="flex items-baseline justify-between gap-2 sm:block">
                  <h2 className="text-sm font-medium text-ink-secondary">
                    {t(GROUP_LABEL_KEYS[group.key])}
                  </h2>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {t("memory.itemCount", { count: group.items.length })}
                  </p>
                </header>
                <div className="min-w-0 divide-y divide-line border-y border-line">
                  {group.items.map((file) => (
                    <button
                      key={file.filename}
                      type="button"
                      title={file.filename}
                      aria-label={t("memory.open", {
                        name: memoryDisplayName(file),
                      })}
                      onClick={() => setSelected(file)}
                      className="group flex w-full items-center gap-3 px-1 py-3 text-left outline-none transition-colors hover:bg-line/30 focus-visible:bg-accent-soft"
                    >
                      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-bubble-tool text-ink-muted transition-colors group-hover:text-accent">
                        <FileText size={16} />
                      </span>
                      <span className="min-w-0 flex-1 truncate text-sm font-medium text-ink">
                        {memoryDisplayName(file)}
                      </span>
                      <span className="flex shrink-0 items-center gap-2 text-[11px] text-ink-muted">
                        <span>{formatFileSize(file.size, language)}</span>
                        <span aria-hidden="true">·</span>
                        <time
                          dateTime={String(file.modified_time)}
                          title={formatAbsoluteTime(
                            file.modified_time,
                            language,
                          )}
                        >
                          {formatRelativeTime(file.modified_time, language)}
                        </time>
                      </span>
                      <ChevronRight
                        size={15}
                        className="shrink-0 text-ink-muted"
                      />
                    </button>
                  ))}
                </div>
              </section>
            ))}
          </div>
        )}
      </div>

      <MemoryDetails
        file={selected}
        onOpenChange={(open) => !open && setSelected(null)}
        onSaved={refreshFiles}
      />
    </div>
  );
}

function MemoryDetails({
  file,
  onOpenChange,
  onSaved,
}: {
  file: MdFileInfo | null;
  onOpenChange: (open: boolean) => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [editor, dispatch] = useReducer(
    memoryEditorReducer,
    initialMemoryEditorState,
  );
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!file) return;
    const controller = new AbortController();
    setLoading(true);
    setLoadError(null);
    setNotice(null);
    dispatch({ type: "load", content: "" });
    void memoryApi
      .get(file.filename, controller.signal)
      .then(({ content }) => dispatch({ type: "load", content }))
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setLoadError(
            t("memory.contentLoadFailed", {
              message: readableError(reason),
            }),
          );
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [file?.filename]);

  const save = async () => {
    if (!file) return;
    dispatch({ type: "saveStart" });
    setNotice(null);
    try {
      await memoryApi.update(file.filename, editor.draft);
    } catch (reason) {
      dispatch({
        type: "saveFailure",
        error: t("memory.saveFailed", { message: readableError(reason) }),
      });
      return;
    }
    dispatch({ type: "saveSuccess" });
    setNotice(t("memory.saved"));
    await onSaved();
  };

  const close = (open: boolean) => {
    if (!open && editor.saving) return;
    onOpenChange(open);
  };

  return (
    <Dialog.Root open={file !== null} onOpenChange={close}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed inset-y-0 right-0 z-50 flex w-[min(40rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-bubble-tool text-accent">
              <NotebookPen size={18} />
            </span>
            <div className="min-w-0 flex-1">
              <Dialog.Title className="truncate font-medium text-ink">
                {file ? memoryDisplayName(file) : ""}
              </Dialog.Title>
              <Dialog.Description
                className="mt-0.5 truncate text-xs text-ink-muted"
                title={file?.filename}
              >
                {file?.filename || t("memory.detailsDescription")}
              </Dialog.Description>
            </div>
            {!loading && !loadError && editor.mode === "view" && (
              <button
                type="button"
                onClick={() => {
                  setNotice(null);
                  dispatch({ type: "edit" });
                }}
                className="flex shrink-0 items-center gap-1.5 rounded-md border border-line px-2.5 py-1.5 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink"
              >
                <Pencil size={14} />
                {t("memory.edit")}
              </button>
            )}
            <Dialog.Close asChild>
              <button
                type="button"
                title={t("memory.close")}
                disabled={editor.saving}
                className="shrink-0 rounded-md p-1 text-ink-muted hover:bg-line/50 hover:text-ink disabled:opacity-40"
              >
                <X size={16} />
              </button>
            </Dialog.Close>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto p-5 sm:p-6">
            {notice && (
              <div
                role="status"
                className="mb-5 rounded-md bg-accent-soft px-3 py-2 text-xs text-ok"
              >
                {notice}
              </div>
            )}
            {loading ? (
              <div className="flex items-center justify-center gap-2 py-20 text-sm text-ink-muted">
                <LoaderCircle size={16} className="animate-spin" />
                {t("memory.contentLoading")}
              </div>
            ) : loadError ? (
              <div
                role="alert"
                className="rounded-md bg-danger-soft px-3 py-2 text-xs text-danger"
              >
                {loadError}
              </div>
            ) : editor.mode === "editing" ? (
              <div className="flex min-h-full flex-col">
                {editor.error && (
                  <div
                    role="alert"
                    className="mb-3 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger"
                  >
                    {editor.error}
                  </div>
                )}
                <label htmlFor="memory-editor" className="sr-only">
                  {t("memory.editorLabel")}
                </label>
                <textarea
                  id="memory-editor"
                  autoFocus
                  value={editor.draft}
                  disabled={editor.saving}
                  onChange={(event) =>
                    dispatch({ type: "change", draft: event.target.value })
                  }
                  className="min-h-[calc(100vh-13rem)] w-full resize-none rounded-md border border-line bg-surface px-4 py-3 font-mono text-sm leading-6 text-ink outline-none transition-colors placeholder:text-ink-muted focus:border-line-strong disabled:opacity-60"
                />
              </div>
            ) : editor.content ? (
              <Markdown>{editor.content}</Markdown>
            ) : (
              <div className="flex flex-col items-center py-20 text-center text-ink-muted">
                <FileText size={24} />
                <p className="mt-3 text-sm">{t("memory.emptyContent")}</p>
              </div>
            )}
          </div>

          {editor.mode === "editing" && !loading && !loadError && (
            <footer className="flex justify-end gap-2 border-t border-line p-4">
              <button
                type="button"
                disabled={editor.saving}
                onClick={() => dispatch({ type: "cancel" })}
                className="rounded-md px-3 py-2 text-xs font-medium text-ink-secondary transition-colors hover:bg-line/50 hover:text-ink disabled:opacity-40"
              >
                {t("memory.cancel")}
              </button>
              <button
                type="button"
                disabled={editor.saving || editor.draft === editor.content}
                onClick={() => void save()}
                className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                {editor.saving ? (
                  <LoaderCircle size={14} className="animate-spin" />
                ) : (
                  <Save size={14} />
                )}
                {editor.saving ? t("memory.saving") : t("memory.save")}
              </button>
            </footer>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function formatAbsoluteTime(
  value: string | number,
  language: "zh" | "en",
): string {
  const numeric = typeof value === "number" ? value : Number(value);
  const date =
    Number.isFinite(numeric) && String(value).trim()
      ? new Date(numeric < 1_000_000_000_000 ? numeric * 1000 : numeric)
      : new Date(String(value));
  if (Number.isNaN(date.valueOf())) return String(value);
  return new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
