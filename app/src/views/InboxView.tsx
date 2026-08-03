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
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Badge,
  Button,
  Card,
  ConfirmDialog,
  EmptyState,
  IconButton,
  PageContainer,
  PageHeader,
  Skeleton,
  SkeletonRows,
} from "../components/ui";
import { inboxApi } from "../lib/api";
import { presentError } from "../lib/errorPresentation";
import {
  countUnread,
  eventRunId,
  isRoutineEvent,
  markEventsRead,
  presentTraceStep,
  summarizeAutoDream,
  type InboxEvent,
  type InboxTrace,
} from "../lib/inbox";
import { useChatStore } from "../stores/chat";
import {
  useTranslation,
  type TranslationKey,
} from "../lib/i18n";
import { relativeTime } from "../lib/relativeTime";
import { useInboxStore } from "../stores/inbox";

export function InboxView() {
  const { language, t } = useTranslation();
  const navigate = useNavigate();
  const refreshUnread = useInboxStore((state) => state.refreshUnread);
  const setUnreadCount = useInboxStore((state) => state.setUnreadCount);
  const chats = useChatStore((state) => state.chats);
  const [events, setEvents] = useState<InboxEvent[]>([]);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [traces, setTraces] = useState<Record<string, InboxTrace>>({});
  const [traceOpen, setTraceOpen] = useState<Record<string, boolean>>({});
  const [traceLoading, setTraceLoading] = useState<string | null>(null);
  const [traceErrors, setTraceErrors] = useState<Record<string, string>>({});
  const [routineOpen, setRoutineOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [marking, setMarking] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<InboxEvent | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 零结果的例行运行折叠成一组,不和真正有信息量的事件抢注意力。
  const { mainEvents, routineEvents } = useMemo(() => {
    const routine: InboxEvent[] = [];
    const main: InboxEvent[] = [];
    for (const event of events) {
      (isRoutineEvent(event) ? routine : main).push(event);
    }
    return { mainEvents: main, routineEvents: routine };
  }, [events]);

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
      setPendingDelete(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setDeletingId(null);
    }
  };

  /** 拉取轨迹本体:与开关状态解耦,重试直接调它。 */
  const loadTrace = async (event: InboxEvent) => {
    const runId = eventRunId(event);
    if (!runId) return;
    setTraceLoading(event.id);
    // 轨迹失败要落在对应的轨迹框里,不能飘到页顶变成一块空框。
    setTraceErrors((value) => ({ ...value, [event.id]: "" }));
    try {
      const trace = await inboxApi.trace(runId);
      setTraces((value) => ({ ...value, [runId]: trace }));
    } catch (reason) {
      const presented = presentError(reason);
      setTraceErrors((value) => ({
        ...value,
        [event.id]: t(presented.summaryKey),
      }));
    } finally {
      setTraceLoading(null);
    }
  };

  const toggleTrace = async (event: InboxEvent) => {
    const runId = eventRunId(event);
    if (!runId) return;
    const nextOpen = !traceOpen[event.id];
    setTraceOpen((value) => ({ ...value, [event.id]: nextOpen }));
    if (!nextOpen || traces[runId]) return;
    await loadTrace(event);
  };

  /** 展开例行分组时把里面的未读一次性清掉。 */
  const openRoutineGroup = async () => {
    const next = !routineOpen;
    setRoutineOpen(next);
    if (!next) return;
    const unreadIds = routineEvents
      .filter((event) => !event.read)
      .map((event) => event.id);
    if (unreadIds.length === 0) return;
    try {
      await inboxApi.markRead({ event_ids: unreadIds });
      setEvents((items) => markEventsRead(items, unreadIds));
      // 本页只加载了部分事件,全局未读数以服务端为准
      void refreshUnread();
    } catch {
      /* 已读标记失败不打断浏览 */
    }
  };

  const unreadCount = countUnread(events);

  return (
    <>
      <PageContainer width="reading">
        <PageHeader
          title={t("inbox.title")}
          subtitle={t("inbox.subtitle")}
          actions={
          <Button
            variant="secondary"
            size="sm"
            disabled={unreadCount === 0 || marking}
            onClick={() => void markAllRead()}
          >
            {marking ? (
              <LoaderCircle size={15} className="animate-spin" />
            ) : (
              <CheckCheck size={15} />
            )}
            {marking ? t("inbox.marking") : t("inbox.allRead")}
          </Button>
          }
        />

        {error && events.length > 0 && (
          <div className="mb-5 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
            {error}
          </div>
        )}

        {loading && events.length === 0 ? (
          <Card className="p-4">
            <SkeletonRows rows={6} />
          </Card>
        ) : error && events.length === 0 ? (
          // 加载失败不能伪装成"暂无消息"
          <Card className="flex items-center justify-between gap-3 p-4 text-sm text-ink-secondary">
            <span>{error}</span>
            <Button variant="secondary" size="sm" onClick={() => void load()}>
              {t("common.retry")}
            </Button>
          </Card>
        ) : events.length === 0 ? (
          <EmptyState
            icon={<Inbox size={20} />}
            title={t("inbox.emptyTitle")}
            description={t("inbox.emptyDescription")}
          />
        ) : (
          <Card className="divide-y divide-line overflow-hidden">
            {mainEvents.map((event) => {
              const expanded = expandedId === event.id;
              const runId = eventRunId(event);
              const trace = runId ? traces[runId] : undefined;
              const presented = presentInboxEvent(event, t);
              const presentedBody = presentEventBody(event, t);
              const createdAt = relativeEventTime(event.created_at);
              return (
                <article
                  key={event.id}
                  className={event.read ? "bg-surface" : "bg-accent-soft/40"}
                >
                  <div className="group flex items-stretch">
                    <button
                      type="button"
                      onClick={() => void expand(event)}
                      className="flex min-w-0 flex-1 items-start gap-3.5 px-5 py-4 text-left transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
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
                            {presented.title || t("inbox.untitled")}
                          </span>
                          <EventStatus
                            status={event.status}
                            label={presented.status}
                          />
                        </span>
                        <span className="mt-1 line-clamp-2 text-[13px] leading-5 text-ink-tertiary">
                          {presentedBody ?? (event.body || t("inbox.details"))}
                        </span>
                        <span className="mt-1.5 flex flex-wrap items-center gap-2 text-[11px] text-ink-tertiary">
                          <span>
                            {t("inbox.source", {
                              source: presented.sourceType,
                            })}
                          </span>
                          <span>·</span>
                          <time>
                            {createdAt ? t(createdAt.key, createdAt.params) : ""}
                          </time>
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
                    <IconButton
                      tone="danger"
                      size="sm"
                      disabled={deletingId === event.id}
                      title={t("inbox.delete")}
                      onClick={() => setPendingDelete(event)}
                      className="m-2 self-start opacity-0 transition-opacity duration-[var(--dur-fast)] focus-visible:opacity-100 group-hover:opacity-100 group-focus-within:opacity-100"
                    >
                      {deletingId === event.id ? (
                        <LoaderCircle size={15} className="animate-spin" />
                      ) : (
                        <Trash2 size={15} />
                      )}
                    </IconButton>
                  </div>

                  {expanded && (
                    <div className="border-t border-line bg-bubble-tool px-5 py-4">
                      <time className="mb-2 block text-[11px] text-ink-tertiary">
                        {formatTimestamp(event.created_at, language)}
                      </time>
                      <div className="whitespace-pre-wrap text-sm leading-6 text-ink-secondary">
                        {presentedBody ?? (event.body || t("inbox.details"))}
                      </div>
                      {presentedBody && event.body && (
                        <details className="mt-2">
                          <summary className="cursor-pointer text-[11px] text-ink-muted hover:text-ink-secondary">
                            {t("common.technicalDetail")}
                          </summary>
                          <pre className="mt-1.5 whitespace-pre-wrap break-words rounded bg-bg px-2.5 py-2 font-mono text-[11px] leading-5 text-ink-tertiary">
                            {event.body}
                          </pre>
                        </details>
                      )}
                      <SourceJumps
                        event={event}
                        chats={chats}
                        onNavigate={navigate}
                      />
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
                                <div className="space-y-2 py-2">
                                  <Skeleton className="h-3 w-1/3" />
                                  <Skeleton className="h-3 w-4/5" />
                                  <Skeleton className="h-3 w-3/5" />
                                </div>
                              ) : traceErrors[event.id] ? (
                                <div className="flex items-center justify-between gap-3 text-xs text-danger">
                                  <span>{t("inbox.trace.loadFailed")}</span>
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => void loadTrace(event)}
                                  >
                                    {t("common.retry")}
                                  </Button>
                                </div>
                              ) : trace ? (
                                <>
                                  <div className="text-xs font-medium text-ink-secondary">
                                    {t("inbox.traceStatus", {
                                      status: presentValue(
                                        trace.status,
                                        STATUS_KEYS,
                                        t,
                                      ),
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
                                            <TraceStepLine event={item.event} />
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
            {routineEvents.length > 0 && (
              <div className="bg-surface">
                <button
                  type="button"
                  onClick={() => void openRoutineGroup()}
                  className="flex w-full items-center gap-3.5 px-5 py-3 text-left transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
                >
                  <span className="flex h-3 w-3 shrink-0 items-center justify-center">
                    {routineEvents.some((event) => !event.read) && (
                      <Circle
                        size={8}
                        fill="currentColor"
                        className="text-ink-muted"
                      />
                    )}
                  </span>
                  <span className="min-w-0 flex-1 truncate text-[13px] text-ink-tertiary">
                    {t("inbox.routineGroup", {
                      count: routineEvents.length,
                    })}
                  </span>
                  {routineOpen ? (
                    <ChevronDown size={15} className="shrink-0 text-ink-muted" />
                  ) : (
                    <ChevronRight
                      size={15}
                      className="shrink-0 text-ink-muted"
                    />
                  )}
                </button>
                {routineOpen &&
                  routineEvents.map((event) => {
                    const createdAt = relativeEventTime(event.created_at);
                    return (
                      <div
                        key={event.id}
                        className="group flex items-center gap-3 border-t border-line py-1.5 pl-12 pr-2 text-xs text-ink-tertiary"
                      >
                        <span className="min-w-0 flex-1 truncate">
                          {presentInboxEvent(event, t).title}
                          {" · "}
                          {createdAt
                            ? t(createdAt.key, createdAt.params)
                            : ""}
                        </span>
                        <IconButton
                          tone="danger"
                          size="sm"
                          disabled={deletingId === event.id}
                          title={t("inbox.delete")}
                          onClick={() => setPendingDelete(event)}
                          className="opacity-0 transition-opacity duration-[var(--dur-fast)] focus-visible:opacity-100 group-hover:opacity-100"
                        >
                          {deletingId === event.id ? (
                            <LoaderCircle
                              size={14}
                              className="animate-spin"
                            />
                          ) : (
                            <Trash2 size={14} />
                          )}
                        </IconButton>
                      </div>
                    );
                  })}
              </div>
            )}
          </Card>
        )}
      </PageContainer>
      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("inbox.delete")}
        description={t("inbox.deleteConfirm")}
        tone="danger"
        busy={deletingId !== null}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        onConfirm={() => pendingDelete && void remove(pendingDelete)}
      />
    </>
  );
}

function EventStatus({
  status,
  label,
}: {
  status: string;
  label: string;
}) {
  const tone =
    status === "success"
      ? "ok"
      : status === "error" || status === "failed"
        ? "danger"
        : "neutral";
  return <Badge tone={tone}>{label}</Badge>;
}

type Translate = ReturnType<typeof useTranslation>["t"];

const STATUS_KEYS: Partial<Record<string, TranslationKey>> = {
  success: "inbox.status.success",
  completed: "inbox.status.success",
  error: "inbox.status.error",
  failed: "inbox.status.error",
  running: "inbox.status.running",
  in_progress: "inbox.status.running",
};

const SOURCE_TYPE_KEYS: Partial<Record<string, TranslationKey>> = {
  memory: "inbox.sourceType.memory",
  cron: "inbox.sourceType.cron",
};

const TITLE_KEYS: Partial<Record<string, TranslationKey>> = {
  "Auto-dream result": "inbox.title.autoMemory",
  "Auto-memory result": "inbox.title.autoMemory",
};

function presentInboxEvent(event: InboxEvent, t: Translate) {
  return {
    title: presentValue(event.title, TITLE_KEYS, t),
    status: presentValue(event.status, STATUS_KEYS, t),
    sourceType: presentValue(event.source_type, SOURCE_TYPE_KEYS, t),
  };
}

function presentValue(
  value: string,
  keys: Partial<Record<string, TranslationKey>>,
  t: Translate,
) {
  const key = keys[value];
  return key ? t(key) : value;
}

/** 结构化正文(记忆整理等)转成一句人话;认不出的返回 null 用原文。 */
function presentEventBody(event: InboxEvent, t: Translate): string | null {
  const summary = summarizeAutoDream(event.body);
  if (!summary) return null;
  if (summary.changed === 0 && summary.units === 0) {
    return t("inbox.autodream.none");
  }
  return t("inbox.autodream.some", {
    files: String(summary.scanned),
    units: String(summary.units),
  });
}

/** 事件来源跳转:收到结果后能回到产生它的地方。 */
function SourceJumps({
  event,
  chats,
  onNavigate,
}: {
  event: InboxEvent;
  chats: { id: string; session_id: string }[];
  onNavigate: (path: string) => void;
}) {
  const { t } = useTranslation();
  const jumps: { label: string; path: string }[] = [];
  if (event.source_type === "cron") {
    jumps.push({ label: t("inbox.openCron"), path: "/crons" });
  }
  if (event.source_type === "memory") {
    jumps.push({ label: t("inbox.openMemory"), path: "/memory" });
  }
  const sessionId =
    typeof event.payload?.session_id === "string"
      ? event.payload.session_id
      : "";
  const chat = sessionId
    ? chats.find((item) => item.session_id === sessionId)
    : undefined;
  if (chat) {
    jumps.push({ label: t("inbox.openChat"), path: `/chat/${chat.id}` });
  }
  if (jumps.length === 0) return null;
  return (
    <div className="mt-3 flex flex-wrap items-center gap-2">
      {jumps.map((jump) => (
        <Button
          key={jump.path}
          variant="secondary"
          size="sm"
          onClick={() => onNavigate(jump.path)}
        >
          {jump.label}
        </Button>
      ))}
    </div>
  );
}

/** 轨迹步骤的人话行;认不出的步骤保底显示原始摘要。 */
function TraceStepLine({ event }: { event: Record<string, unknown> }) {
  const { t } = useTranslation();
  const step = presentTraceStep(event);
  if (step.kind === "tool") {
    return <>{t("inbox.trace.tool", { name: step.name })}</>;
  }
  if (step.kind === "message") return <>{t("inbox.trace.message")}</>;
  if (step.kind === "failed") {
    return (
      <span className="text-danger" title={step.detail}>
        {t("inbox.trace.failed")}
      </span>
    );
  }
  return <>{step.text}</>;
}

function relativeEventTime(value: number) {
  const date = new Date(value * 1000);
  if (Number.isNaN(date.valueOf())) return null;
  return relativeTime(date.toISOString());
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
