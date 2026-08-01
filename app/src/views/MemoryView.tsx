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
  Button,
  Card,
  EmptyState,
  IconButton,
  PageContainer,
  PageHeader,
  SkeletonRows,
  inputClasses,
} from "../components/ui";
import {
  formatFileSize,
  groupMemoryFiles,
  initialMemoryEditorState,
  memoryApi,
  memoryDisplayName,
  memoryEditorReducer,
  memoryGroupKey,
  memoryTimeIso,
  type MdFileInfo,
  type MemoryGroupKey,
} from "../lib/memory";
import { relativeTime } from "../lib/relativeTime";
import { useTranslation, type TranslationKey } from "../lib/i18n";

const GROUP_LABEL_KEYS: Record<MemoryGroupKey, TranslationKey> = {
  journal: "memory.group.journal",
  procedure: "memory.group.procedure",
  wiki: "memory.group.wiki",
  other: "memory.group.other",
};

/** 次行「来源」文案：把内部目录结构翻译成用户能读懂的记忆来源。 */
const SOURCE_LABEL_KEYS: Record<MemoryGroupKey, TranslationKey> = {
  journal: "memory.source.journal",
  procedure: "memory.source.procedure",
  wiki: "memory.source.wiki",
  other: "memory.source.other",
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
    <>
      <PageContainer width="reading">
        <PageHeader
          title={t("memory.title")}
          subtitle={t("memory.subtitle")}
        />

        {error && (
          <div
            role="alert"
            className="mt-5 flex items-start justify-between gap-3 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger"
          >
            <span>{error}</span>
            <IconButton
              size="sm"
              title={t("memory.dismiss")}
              onClick={() => setError(null)}
            >
              <X size={14} />
            </IconButton>
          </div>
        )}

        {loading ? (
          <Card className="mt-6 p-4">
            <SkeletonRows rows={5} />
          </Card>
        ) : files.length === 0 ? (
          <div className="mt-6">
            <EmptyState
              icon={<NotebookPen size={20} />}
              title={t("memory.emptyTitle")}
              description={t("memory.emptyDescription")}
            />
          </div>
        ) : (
          <div className="mt-8 space-y-6">
            {groups.map((group) => (
              <section key={group.key}>
                <div className="mb-2 flex items-baseline gap-2">
                  <h2 className="text-[13px] font-medium text-ink-secondary">
                    {t(GROUP_LABEL_KEYS[group.key])}
                  </h2>
                  <span className="text-[11px] text-ink-muted">
                    {t("memory.itemCount", { count: group.items.length })}
                  </span>
                </div>
                <Card className="min-w-0 divide-y divide-line overflow-hidden">
                  {group.items.map((file) => {
                    const title = memoryDisplayName(file);
                    const modifiedIso = memoryTimeIso(file.modified_time);
                    const updated = relativeTime(modifiedIso);
                    return (
                      <button
                        key={file.filename}
                        type="button"
                        aria-label={t("memory.open", { name: title })}
                        onClick={() => setSelected(file)}
                        className="group flex w-full items-center gap-3 px-4 py-3 text-left outline-none transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover focus-visible:bg-fill-active"
                      >
                        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-bubble-tool text-ink-muted transition-colors group-hover:text-accent">
                          <FileText size={16} />
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium leading-5 text-ink">
                            {title}
                          </span>
                          <span className="mt-0.5 flex items-center gap-1.5 truncate text-[13px] leading-5 text-ink-tertiary">
                            <span className="truncate">
                              {t(SOURCE_LABEL_KEYS[group.key])}
                            </span>
                            {updated && (
                              <>
                                <span aria-hidden="true">·</span>
                                <time
                                  className="shrink-0"
                                  dateTime={modifiedIso ?? undefined}
                                  title={formatAbsoluteTime(
                                    file.modified_time,
                                    language,
                                  )}
                                >
                                  {t(updated.key, updated.params)}
                                </time>
                              </>
                            )}
                          </span>
                        </span>
                        <ChevronRight
                          size={15}
                          className="shrink-0 text-ink-muted transition-colors group-hover:text-ink-secondary"
                        />
                      </button>
                    );
                  })}
                </Card>
              </section>
            ))}
          </div>
        )}
      </PageContainer>

      <MemoryDetails
        file={selected}
        onOpenChange={(open) => !open && setSelected(null)}
        onSaved={refreshFiles}
      />
    </>
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
  const { language, t } = useTranslation();
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
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-drawer fixed inset-y-0 right-0 z-50 flex w-[min(40rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[var(--radius-md)] border border-line bg-bubble-tool text-ink-muted">
              <NotebookPen size={18} />
            </span>
            <div className="min-w-0 flex-1">
              <Dialog.Title className="truncate font-medium text-ink">
                {file ? memoryDisplayName(file) : ""}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 truncate text-xs text-ink-muted">
                {file
                  ? t(SOURCE_LABEL_KEYS[memoryGroupKey(file.filename)])
                  : t("memory.detailsDescription")}
              </Dialog.Description>
            </div>
            {!loading && !loadError && editor.mode === "view" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => {
                  setNotice(null);
                  dispatch({ type: "edit" });
                }}
              >
                <Pencil size={14} />
                {t("memory.edit")}
              </Button>
            )}
            <Dialog.Close asChild>
              <IconButton
                size="sm"
                title={t("memory.close")}
                disabled={editor.saving}
              >
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto p-5 sm:p-6">
            {notice && (
              <div
                role="status"
                className="mb-5 rounded-md bg-fill-active px-3 py-2 text-xs text-ok"
              >
                {notice}
              </div>
            )}
            {loading ? (
              <div className="py-8">
                <SkeletonRows rows={6} />
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
                  className={`${inputClasses} min-h-[calc(100vh-13rem)] resize-none py-3 font-mono leading-6`}
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

            {file && !loading && !loadError && editor.mode === "view" && (
              <TechnicalDetails file={file} language={language} />
            )}
          </div>

          {editor.mode === "editing" && !loading && !loadError && (
            <footer className="flex justify-end gap-2 border-t border-line p-4">
              <Button
                variant="ghost"
                size="sm"
                disabled={editor.saving}
                onClick={() => dispatch({ type: "cancel" })}
              >
                {t("memory.cancel")}
              </Button>
              <Button
                variant="primary"
                size="sm"
                disabled={editor.saving || editor.draft === editor.content}
                onClick={() => void save()}
              >
                {editor.saving ? (
                  <LoaderCircle size={14} className="animate-spin" />
                ) : (
                  <Save size={14} />
                )}
                {editor.saving ? t("memory.saving") : t("memory.save")}
              </Button>
            </footer>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/**
 * 技术信息区：列表行只讲“记忆”，文件名/大小/路径这类实现细节收在这里。
 */
function TechnicalDetails({
  file,
  language,
}: {
  file: MdFileInfo;
  language: "zh" | "en";
}) {
  const { t } = useTranslation();
  const rows: Array<{ label: string; value: string; mono?: boolean }> = [
    { label: t("memory.tech.filename"), value: file.filename, mono: true },
    { label: t("memory.tech.size"), value: formatFileSize(file.size, language) },
    { label: t("memory.tech.path"), value: file.path, mono: true },
  ];
  return (
    <section className="mt-8 border-t border-line pt-4">
      <h3 className="text-[11px] font-medium text-ink-muted">
        {t("memory.tech.title")}
      </h3>
      <dl className="mt-2 space-y-1">
        {rows.map((row) => (
          <div key={row.label} className="flex gap-3 text-xs leading-5">
            <dt className="w-14 shrink-0 text-ink-muted">{row.label}</dt>
            <dd
              className={`min-w-0 flex-1 break-all text-ink-tertiary ${
                row.mono ? "font-mono" : ""
              }`}
            >
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
    </section>
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
