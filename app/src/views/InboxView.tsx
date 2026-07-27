import {
  CheckCheck,
  ChevronDown,
  ChevronRight,
  Circle,
  Inbox,
  LoaderCircle,
  Route,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { inboxApi } from "../lib/api";
import {
  countUnread,
  eventRunId,
  markEventsRead,
  traceEventSummary,
  type InboxEvent,
  type InboxTrace,
} from "../lib/inbox";
import { useTranslation } from "../lib/i18n";
import { useInboxStore } from "../stores/inbox";

export function InboxView() {
  const { language, t } = useTranslation();
  const refreshUnread = useInboxStore((state) => state.refreshUnread);
  const setUnreadCount = useInboxStore((state) => state.setUnreadCount);
  const [events, setEvents] = useState<InboxEvent[]>([]);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [traces, setTraces] = useState<Record<string, InboxTrace>>({});
  const [traceOpen, setTraceOpen] = useState<Record<string, boolean>>({});
  const [traceLoading, setTraceLoading] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [marking, setMarking] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await inboxApi.events({ limit: 200 });
      setEvents(response.events);
      setUnreadCount(countUnread(response.events));
    } catch (reason) {
      setError(t("inbox.loadFailed", { message: readableError(reason) }));
    } finally {
      setLoading(false);
      void refreshUnread();
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const expand = async (event: InboxEvent) => {
    const nextId = expandedId === event.id ? null : event.id;
    setExpandedId(nextId);
    if (nextId && !event.read) {
      try {
        await inboxApi.markRead({ event_ids: [event.id] });
        setEvents((items) => {
          const next = markEventsRead(items, [event.id]);
          setUnreadCount(countUnread(next));
          return next;
        });
      } catch (reason) {
        setError(readableError(reason));
      }
    }
  };

  const markAllRead = async () => {
    setMarking(true);
    setError(null);
    try {
      await inboxApi.markRead({ all: true });
      setEvents((items) => markEventsRead(items));
      setUnreadCount(0);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setMarking(false);
    }
  };

  const remove = async (event: InboxEvent) => {
    if (!window.confirm(t("inbox.deleteConfirm"))) return;
    setDeletingId(event.id);
    setError(null);
    try {
      await inboxApi.delete(event.id);
      setEvents((items) => {
        const next = items.filter((item) => item.id !== event.id);
        setUnreadCount(countUnread(next));
        return next;
      });
      if (expandedId === event.id) setExpandedId(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setDeletingId(null);
    }
  };

  const toggleTrace = async (event: InboxEvent) => {
    const runId = eventRunId(event);
    if (!runId) return;
    const nextOpen = !traceOpen[event.id];
    setTraceOpen((value) => ({ ...value, [event.id]: nextOpen }));
    if (!nextOpen || traces[runId]) return;
    setTraceLoading(event.id);
    setError(null);
    try {
      const trace = await inboxApi.trace(runId);
      setTraces((value) => ({ ...value, [runId]: trace }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setTraceLoading(null);
    }
  };

  const unreadCount = countUnread(events);

  return (
    <div className="h-full overflow-y-auto bg-surface">
      <div className="mx-auto max-w-4xl px-6 py-8 sm:px-10">
        <header className="mb-6 flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-medium tracking-tight text-ink">
              {t("inbox.title")}
            </h1>
            <p className="mt-1 text-sm text-ink-muted">
              {t("inbox.subtitle")}
            </p>
          </div>
          <button
            type="button"
            disabled={unreadCount === 0 || marking}
            onClick={() => void markAllRead()}
            className="flex items-center gap-1.5 rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink disabled:cursor-not-allowed disabled:opacity-40"
          >
            {marking ? (
              <LoaderCircle size={15} className="animate-spin" />
            ) : (
              <CheckCheck size={15} />
            )}
            {marking ? t("inbox.marking") : t("inbox.allRead")}
          </button>
        </header>

        {error && (
          <div className="mb-5 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
            {error}
          </div>
        )}

        {loading && events.length === 0 ? (
          <div className="flex items-center justify-center gap-2 rounded-lg border border-line py-16 text-sm text-ink-muted">
            <LoaderCircle size={16} className="animate-spin" />
            {t("inbox.loading")}
          </div>
        ) : events.length === 0 ? (
          <div className="flex flex-col items-center rounded-lg border border-dashed border-line px-6 py-16 text-center">
            <Inbox size={28} className="text-ink-muted" />
            <h2 className="mt-4 font-medium text-ink">
              {t("inbox.emptyTitle")}
            </h2>
            <p className="mt-1 max-w-sm text-sm text-ink-muted">
              {t("inbox.emptyDescription")}
            </p>
          </div>
        ) : (
          <div className="divide-y divide-line overflow-hidden rounded-lg border border-line">
            {events.map((event) => {
              const expanded = expandedId === event.id;
              const runId = eventRunId(event);
              const trace = runId ? traces[runId] : undefined;
              return (
                <article
                  key={event.id}
                  className={event.read ? "bg-surface" : "bg-accent-soft/40"}
                >
                  <div className="flex items-stretch">
                    <button
                      type="button"
                      onClick={() => void expand(event)}
                      className="flex min-w-0 flex-1 items-start gap-3 px-4 py-3.5 text-left hover:bg-line/30"
                    >
                      <span className="mt-1.5 flex h-3 w-3 shrink-0 items-center justify-center">
                        {!event.read && (
                          <Circle
                            size={8}
                            fill="currentColor"
                            className="text-accent"
                          />
                        )}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                          <span
                            className={`truncate text-sm text-ink ${
                              event.read ? "font-normal" : "font-semibold"
                            }`}
                          >
                            {event.title || t("inbox.untitled")}
                          </span>
                          <EventStatus status={event.status} />
                        </span>
                        <span className="mt-1 line-clamp-2 text-xs leading-5 text-ink-muted">
                          {event.body || t("inbox.details")}
                        </span>
                        <span className="mt-1.5 flex flex-wrap items-center gap-2 text-[11px] text-ink-muted">
                          <span>
                            {t("inbox.source", { source: event.source_type })}
                          </span>
                          <span>·</span>
                          <time>{formatTimestamp(event.created_at, language)}</time>
                        </span>
                      </span>
                      {expanded ? (
                        <ChevronDown
                          size={15}
                          className="mt-1 shrink-0 text-ink-muted"
                        />
                      ) : (
                        <ChevronRight
                          size={15}
                          className="mt-1 shrink-0 text-ink-muted"
                        />
                      )}
                    </button>
                    <button
                      type="button"
                      disabled={deletingId === event.id}
                      title={t("inbox.delete")}
                      onClick={() => void remove(event)}
                      className="m-2 self-start rounded-md p-2 text-ink-muted hover:bg-danger-soft hover:text-danger disabled:opacity-30"
                    >
                      {deletingId === event.id ? (
                        <LoaderCircle size={15} className="animate-spin" />
                      ) : (
                        <Trash2 size={15} />
                      )}
                    </button>
                  </div>

                  {expanded && (
                    <div className="border-t border-line bg-bubble-tool px-5 py-4">
                      <div className="whitespace-pre-wrap text-sm leading-6 text-ink-secondary">
                        {event.body || t("inbox.details")}
                      </div>
                      {runId && (
                        <div className="mt-4">
                          <button
                            type="button"
                            onClick={() => void toggleTrace(event)}
                            className="flex items-center gap-1.5 text-xs font-medium text-accent hover:text-accent-hover"
                          >
                            <Route size={14} />
                            {traceOpen[event.id]
                              ? t("inbox.hideTrace")
                              : t("inbox.showTrace")}
                          </button>
                          {traceOpen[event.id] && (
                            <div className="mt-3 rounded-md border border-line bg-surface p-3">
                              {traceLoading === event.id ? (
                                <div className="flex items-center gap-2 py-3 text-xs text-ink-muted">
                                  <LoaderCircle
                                    size={14}
                                    className="animate-spin"
                                  />
                                  {t("inbox.traceLoading")}
                                </div>
                              ) : trace ? (
                                <>
                                  <div className="text-xs font-medium text-ink-secondary">
                                    {t("inbox.traceStatus", {
                                      status: trace.status,
                                    })}
                                  </div>
                                  {trace.events.length === 0 ? (
                                    <div className="mt-2 text-xs text-ink-muted">
                                      {t("inbox.traceEmpty")}
                                    </div>
                                  ) : (
                                    <div className="mt-3 space-y-2 border-l border-line pl-3">
                                      {trace.events.slice(-12).map((item, index) => (
                                        <div
                                          key={`${item.at}-${index}`}
                                          className="text-xs"
                                        >
                                          <time className="text-[10px] text-ink-muted">
                                            {formatTimestamp(item.at, language)}
                                          </time>
                                          <div className="mt-0.5 break-words text-ink-secondary">
                                            {traceEventSummary(item.event)}
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                  )}
                                </>
                              ) : null}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </article>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function EventStatus({ status }: { status: string }) {
  const tone =
    status === "success"
      ? "bg-accent-soft text-ok"
      : status === "error" || status === "failed"
        ? "bg-danger-soft text-danger"
        : "bg-bubble-tool text-ink-secondary";
  return (
    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${tone}`}>
      {status}
    </span>
  );
}

function formatTimestamp(value: number, language: "zh" | "en") {
  const date = new Date(value * 1000);
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
