import {
  memo,
  useCallback,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useNavigate } from "react-router-dom";
import {
  Check,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  Copy,
  FileDiff,
  RefreshCw,
} from "lucide-react";
import { APP_NAME } from "../../lib/appInfo";
import {
  collectConversationArtifacts,
  resolveConversationFileLink,
  shouldPresentArtifactPair,
} from "../../lib/conversationArtifacts";
import {
  collectFileChanges,
  directoryOf,
  shortenPath,
  totalChangeStats,
  type FileChange,
} from "../../lib/fileChanges";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import { textFromContent } from "../../lib/content";
import {
  selectCollapsedWindow,
  summarizeTrack,
  type CollapsedRow,
  type CollapsedRowRole,
  type TrackEntrySnapshot,
} from "../../lib/executionTrack";
import { splitInlineThinking } from "../../lib/inlineThinking";
import { formatDuration, getMessageTiming } from "../../lib/messageTiming";
import { useNow } from "../../lib/useNow";
import { useUiPrefs } from "../../stores/uiPrefs";
import type { ContentBlock, TextContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { useChatStore } from "../../stores/chat";
import { PotatoMark } from "../brand/PotatoMark";
import { Spinner } from "../ui/Spinner";
import { ApprovalCard } from "./ApprovalCard";
import { ChangeStat } from "./ChangeStat";
import { Collapse } from "./Collapse";
import { MessageContent } from "./MessageContent";
import { isContextCompactionMessage, ProgressCard } from "./ProgressCard";
import { ReasoningBlock } from "./ReasoningBlock";
import {
  buildToolPair,
  humanToolLabel,
  toolData,
  toolPairStatus,
  ToolCard,
  type ToolPair,
} from "./ToolCard";

interface MessageListProps {
  messages: StreamMessage[];
  activeMessageId?: string;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
}

interface Turn {
  id: string;
  role: "user" | "assistant";
  messages: StreamMessage[];
}

export function MessageList({
  messages,
  activeMessageId,
  onOpenFile,
  onOpenChange,
}: MessageListProps) {
  const turns = useMemo(() => groupIntoTurns(messages), [messages]);
  const pendingApprovals = useChatStore((state) => state.pendingApprovals);
  const isStreaming = useChatStore((state) => state.isStreaming);
  const lastIndex = turns.length - 1;
  // 发送后助手首帧到达前,提前挂载头像行 + 等待轨道,消灭"死空气"。
  // 审批卡在场时模型本来就暂停,不再叠加等待占位。
  const showPendingTurn =
    isStreaming &&
    pendingApprovals.length === 0 &&
    (turns.length === 0 || turns[lastIndex]!.role === "user");
  return (
    <div className="mx-auto w-full max-w-[48rem] px-6 pb-12 pt-8 sm:px-8">
      {turns.map((turn, index) =>
        turn.role === "user" ? (
          <UserTurn
            key={turn.id}
            messages={turn.messages}
            activeMessageId={activeMessageId}
          />
        ) : (
          <AssistantTurn
            key={turn.id}
            messages={turn.messages}
            onOpenFile={onOpenFile}
            onOpenChange={onOpenChange}
            // 流式进行中的最后一轮不显示动作行。
            showActions={!(isStreaming && index === lastIndex)}
            // 只有最后一轮能「重新生成」：它等价于重发上一条用户消息。
            regeneratePrompt={
              index === lastIndex ? previousUserText(turns, index) : ""
            }
            streaming={
              isStreaming &&
              index === lastIndex &&
              pendingApprovals.length === 0
            }
            activeMessageId={activeMessageId}
          />
        ),
      )}
      {showPendingTurn && (
        <div data-testid="turn-assistant" className="qp-msg-in mb-10">
          <AssistantHeader />
          <ExecutionTrack entries={[]} waiting pulsing live />
        </div>
      )}
      {pendingApprovals.map((approval) => (
        <ApprovalCard key={approval.request_id} approval={approval} />
      ))}
    </div>
  );
}

interface UserTurnProps {
  messages: StreamMessage[];
  activeMessageId?: string;
}

const UserTurn = memo(function UserTurn({
  messages,
  activeMessageId,
}: UserTurnProps) {
  return (
    <div data-testid="turn-user" className="qp-msg-in mb-8 flex justify-end">
      {/* 70% 上限 + 中档圆角:对表 WB(682px 宽的 18px 圆角灰板太"网页") */}
      <div className="max-w-[70%] rounded-[var(--radius-md)] bg-bubble-user px-4 py-2.5 text-[15px] leading-[1.7]">
        {messages.map((message) => (
          <div
            id={`message-${message.id}`}
            key={message.id}
            className={
              activeMessageId === message.id
                ? "rounded-[6px] ring-2 ring-accent/35 ring-offset-2 ring-offset-bubble-user"
                : undefined
            }
          >
            <MessageContent content={message.content} markdown={false} />
          </div>
        ))}
      </div>
    </div>
  );
}, areUserTurnPropsEqual);

type RenderItem =
  | { kind: "node"; key: string; node: ReactNode }
  | { kind: "pair"; key: string; pair: ToolPair }
  | { kind: "process"; key: string; entry: ProcessEntry };

type ProcessEntry =
  | { kind: "reasoning"; key: string; message: StreamMessage }
  | { kind: "progress"; key: string; message: StreamMessage }
  | { kind: "message"; key: string; message: StreamMessage }
  | { kind: "pair"; key: string; pair: ToolPair };

interface AssistantTurnProps {
  messages: StreamMessage[];
  showActions: boolean;
  regeneratePrompt: string;
  /** 本轮是流式进行中的最后一轮（无审批卡挂起）。 */
  streaming: boolean;
  activeMessageId?: string;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
}

const AssistantTurn = memo(function AssistantTurn({
  messages,
  showActions,
  regeneratePrompt,
  streaming,
  activeMessageId,
  onOpenFile,
  onOpenChange,
}: AssistantTurnProps) {
  // 流式中挂载的轮是从等待占位原位接管的,重播入场动画会闪一下;
  // 该决定只在挂载时定一次,避免流结束时 class 变化重新触发动画。
  const enterAnimation = useRef(!streaming).current;
  // 内联 <thinking> 标签的模型:思考段拆成轨道条目,正文只留干净文本。
  const presented = useMemo(
    () => messages.flatMap(presentInlineThinking),
    [messages],
  );
  const pairedOutputs = new Set<string>();
  const outputsByCallId = new Map<string, StreamMessage>();
  for (const message of presented) {
    if (!isToolOutput(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    if (!outputsByCallId.has(callId)) outputsByCallId.set(callId, message);
  }
  const copyText = plainText(presented);
  const turnChanges = collectFileChanges(messages);
  const turnArtifacts = collectConversationArtifacts(messages);
  const resolveFilePath = (href: string) =>
    resolveConversationFileLink(href, turnArtifacts);

  // WorkBuddy keeps the conversational narration that happens before/among
  // tool calls inside the execution disclosure. A text message folds into
  // the track as soon as any later reasoning/tool call follows it — even
  // mid-stream — so hoisted tool entries never render above newer text.
  // Text with no execution after it (the final answer) stays outside.
  const foldIntoTrack: boolean[] = new Array(presented.length).fill(false);
  {
    let movedOn = false;
    for (let index = presented.length - 1; index >= 0; index -= 1) {
      foldIntoTrack[index] = movedOn;
      if (startsTrackWork(presented[index]!, outputsByCallId)) movedOn = true;
    }
  }
  const items: RenderItem[] = [];
  for (let index = 0; index < presented.length; index += 1) {
    const message = presented[index]!;
    if (message.type === "reasoning") {
      // 运行中与完成态共用轨道条目,身份不变,完成时原位收口。
      items.push({
        kind: "process",
        key: message.id,
        entry: { kind: "reasoning", key: message.id, message },
      });
      continue;
    }
    if (message.type === "progress") {
      // 失败恒可见(r10 决定);压缩进行中沿用独立卡,轨道摘要只认完成态。
      const failed =
        message.status === "failed" || message.status === "cancelled";
      const activeCompaction =
        isContextCompactionMessage(message) && message.status !== "completed";
      if (failed || activeCompaction) {
        items.push({
          kind: "node",
          key: message.id,
          node: <ProgressCard key={message.id} message={message} />,
        });
      } else {
        items.push({
          kind: "process",
          key: message.id,
          entry: { kind: "progress", key: message.id, message },
        });
      }
      continue;
    }
    if (isToolCall(message.type)) {
      const callId = stringValue(toolData(message).call_id);
      const output = outputsByCallId.get(callId);
      if (output) pairedOutputs.add(output.id);
      const pair = buildToolPair(message, output ?? null);
      if (shouldPresentArtifactPair(pair, turnArtifacts)) {
        items.push({ kind: "pair", key: message.id, pair });
      } else {
        items.push({
          kind: "process",
          key: message.id,
          entry: { kind: "pair", key: message.id, pair },
        });
      }
      continue;
    }
    if (isToolOutput(message.type)) {
      if (pairedOutputs.has(message.id)) continue;
      const pair = buildToolPair(null, message);
      if (shouldPresentArtifactPair(pair, turnArtifacts)) {
        items.push({ kind: "pair", key: message.id, pair });
      } else {
        items.push({
          kind: "process",
          key: message.id,
          entry: { kind: "pair", key: message.id, pair },
        });
      }
      continue;
    }
    if (message.content.length === 0) continue;
    if (isOrdinaryAssistantMessage(message) && foldIntoTrack[index]) {
      items.push({
        kind: "process",
        key: message.id,
        entry: { kind: "message", key: message.id, message },
      });
      continue;
    }
    items.push({
      kind: "node",
      key: message.id,
      node: (
        <div
          id={`message-${message.id}`}
          key={message.id}
          className={`rounded-[6px] py-1 ${
            activeMessageId === message.id
              ? "ring-2 ring-accent/35 ring-offset-2 ring-offset-canvas"
              : ""
          }`}
        >
          <MessageContent
            content={message.content}
            markdown
            onOpenFile={onOpenFile}
            resolveFilePath={resolveFilePath}
          />
        </div>
      ),
    });
  }

  /* WorkBuddy 把整轮执行管线收成一条稳定「执行轨道」：运行中步骤也在
   * 轨道容器里原位演化，而不是独立卡完成后被摘要行替换。正文、失败
   * 进度、产物等需要突出的内容按原顺序渲染在轨道之外。 */
  const rendered: ReactNode[] = [];
  const processEntries = items.flatMap((item) =>
    item.kind === "process" ? [item.entry] : [],
  );
  // 正文流式输出时脉冲让位给文字本身;其余流式阶段(等待/思考/工具间隙)保持活动感。
  const hasStreamingText = presented.some(
    (message) =>
      isOrdinaryAssistantMessage(message) && message.status === "in_progress",
  );
  const waiting = streaming && items.length === 0;
  const pulsing = streaming && !hasStreamingText;
  let executionRendered = false;
  for (const item of items) {
    if (item.kind === "process") {
      if (!executionRendered) {
        rendered.push(
          <ExecutionTrack
            key="execution-track"
            entries={processEntries}
            waiting={false}
            pulsing={pulsing}
            live={streaming}
            onOpenFile={onOpenFile}
            resolveFilePath={resolveFilePath}
          />,
        );
        executionRendered = true;
      }
      continue;
    }
    if (item.kind === "pair") {
      rendered.push(
        <div id={`message-${item.key}`} key={item.key}>
          <ToolCard
            pair={item.pair}
            onOpenFile={onOpenFile}
            prominentArtifact
          />
        </div>,
      );
    } else {
      rendered.push(item.node);
    }
  }

  return (
    <div
      data-testid="turn-assistant"
      className={`${enterAnimation ? "qp-msg-in " : ""}mb-10`}
    >
      <AssistantHeader />
      {waiting ? (
        <ExecutionTrack
          entries={[]}
          waiting
          pulsing
          live
          onOpenFile={onOpenFile}
          resolveFilePath={resolveFilePath}
        />
      ) : (
        rendered
      )}
      {turnChanges.length > 0 && (
        <FileChangesCard changes={turnChanges} onOpenChange={onOpenChange} />
      )}
      {showActions && copyText && (
        <MessageActions text={copyText} regeneratePrompt={regeneratePrompt} />
      )}
    </div>
  );
}, areAssistantTurnPropsEqual);

function areUserTurnPropsEqual(
  previous: UserTurnProps,
  next: UserTurnProps,
): boolean {
  return (
    messageReferencesEqual(previous.messages, next.messages) &&
    Object.is(previous.activeMessageId, next.activeMessageId)
  );
}

function areAssistantTurnPropsEqual(
  previous: AssistantTurnProps,
  next: AssistantTurnProps,
): boolean {
  return (
    messageReferencesEqual(previous.messages, next.messages) &&
    Object.is(previous.activeMessageId, next.activeMessageId) &&
    Object.is(previous.showActions, next.showActions) &&
    Object.is(previous.regeneratePrompt, next.regeneratePrompt) &&
    Object.is(previous.streaming, next.streaming) &&
    Object.is(previous.onOpenFile, next.onOpenFile) &&
    Object.is(previous.onOpenChange, next.onOpenChange)
  );
}

function AssistantHeader() {
  return (
    <div className="mb-2 flex items-center gap-2 text-[14px] font-semibold text-ink-secondary">
      <span className="flex h-6 w-6 items-center justify-center rounded-[7px] bg-btn-primary text-btn-primary-ink">
        <PotatoMark size={16} />
      </span>
      <span>{APP_NAME}</span>
    </div>
  );
}

function messageReferencesEqual(
  previous: StreamMessage[],
  next: StreamMessage[],
): boolean {
  return (
    previous.length === next.length &&
    previous.every((message, index) => Object.is(message, next[index]))
  );
}

/** 对标 Codex「Edited N files」:一轮结束后的改动收口卡,行点击进侧栏 diff。 */
const CHANGES_COLLAPSED_COUNT = 3;

function FileChangesCard({
  changes,
  onOpenChange,
}: {
  changes: FileChange[];
  onOpenChange?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const projectDir = useChatStore((state) => state.project?.path ?? null);
  const [expanded, setExpanded] = useState(false);
  const stats = totalChangeStats(changes);
  const visible = expanded
    ? changes
    : changes.slice(0, CHANGES_COLLAPSED_COUNT);
  const hiddenCount = changes.length - visible.length;

  return (
    <div className="my-3 overflow-hidden rounded-[var(--radius-md)] border border-line bg-surface">
      <div className="flex items-center gap-2.5 px-3.5 py-2.5">
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[8px] bg-fill-hover text-ink-secondary">
          <FileDiff size={15} />
        </span>
        <span className="text-[13px] font-medium text-ink">
          {t("chat.changes.title", { count: stats.files })}
        </span>
        <ChangeStat additions={stats.additions} deletions={stats.deletions} />
      </div>
      <div className="border-t border-line">
        {visible.map((change) => {
          const shortDir = directoryOf(shortenPath(change.path, projectDir));
          return (
            <button
              key={change.path}
              type="button"
              onClick={() => onOpenChange?.(change.path)}
              title={change.path}
              className="flex w-full items-center gap-4 px-3.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
            >
              <span className="min-w-0 flex-1 truncate text-[13px]">
                {shortDir && (
                  <span className="text-ink-tertiary">{shortDir}/</span>
                )}
                <span className="font-medium text-ink">{change.name}</span>
              </span>
              <ChangeStat
                additions={change.additions}
                deletions={change.deletions}
              />
            </button>
          );
        })}
      </div>
      {(hiddenCount > 0 || expanded) && (
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          aria-expanded={expanded}
          className="flex w-full items-center gap-1 border-t border-line px-3.5 py-2 text-left text-[12px] text-ink-tertiary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink-secondary"
        >
          {expanded
            ? t("chat.changes.showLess")
            : t("chat.changes.showMore", { count: hiddenCount })}
          <ChevronDown
            size={13}
            className={`transition-transform duration-[var(--dur-fast)] ${
              expanded ? "rotate-180" : ""
            }`}
          />
        </button>
      )}
    </div>
  );
}

/**
 * 轨道总耗时:首个有计时的条目开始 → 最后一个收口;now 非空(仍有
 * 条目运行)时用它实时计。历史加载的会话没有计时,返回空串即隐藏。
 */
function trackDurationLabel(
  entries: ProcessEntry[],
  now: number | null,
): string {
  let start = Number.POSITIVE_INFINITY;
  let end: number | null = null;
  for (const entry of entries) {
    const ids =
      entry.kind === "pair"
        ? [entry.pair.call?.id, entry.pair.output?.id]
        : [entry.message.id];
    for (const id of ids) {
      if (!id) continue;
      // 内联 <thinking> 拆出的合成条目(`:thinking` 后缀)沿用原消息计时。
      const timing = getMessageTiming(id.replace(/:thinking$/, ""));
      if (!timing) continue;
      start = Math.min(start, timing.startedAt);
      if (timing.endedAt !== null) end = Math.max(end ?? 0, timing.endedAt);
    }
  }
  if (!Number.isFinite(start)) return "";
  const stop = now ?? end;
  if (stop === null) return "";
  return formatDuration(stop - start);
}

/** 近况列表的简短详情:文件名 / 截断命令 / 查询词,拿不到就不显示。 */
function recentPairDetail(pair: ToolPair): string {
  try {
    const args = JSON.parse(pair.arguments) as Record<string, unknown>;
    const path = typeof args.file_path === "string" ? args.file_path : "";
    if (path) return path.split(/[/\\]/).at(-1) ?? "";
    const command = typeof args.command === "string" ? args.command : "";
    if (command) {
      return command.length > 40 ? `${command.slice(0, 40)}…` : command;
    }
    for (const key of ["query", "search_term", "url", "skill"]) {
      const value = args[key];
      if (typeof value === "string" && value) return value;
    }
  } catch {
    // 参数不是 JSON 时不展示详情。
  }
  return "";
}

/** 运行中(含尚未收到 output 的间隙)的条目。 */
function isActiveEntry(entry: ProcessEntry): boolean {
  if (entry.kind === "reasoning" || entry.kind === "progress") {
    return (
      entry.message.status === "created" ||
      entry.message.status === "in_progress"
    );
  }
  if (entry.kind === "pair") return toolPairStatus(entry.pair).running;
  return false;
}

/**
 * 稳定执行轨道:等待响应 → 思考中 → 正在使用工具 → 已完成 N 步,
 * 全程共用同一容器。折叠态是 append-only 的有界窗口(摘要行 + 至多
 * COLLAPSED_WINDOW_CAPACITY 行):行按消息 id 常驻,同 key 原位从
 * 运行态换到完成态,被淘汰的行播退出动画;整轮只在流式结束时收口
 * 一次。选行与摘要状态机在 lib/executionTrack.ts(纯逻辑)。
 */
function ExecutionTrack({
  entries,
  waiting,
  pulsing,
  live,
  onOpenFile,
  resolveFilePath,
}: {
  entries: ProcessEntry[];
  waiting: boolean;
  pulsing: boolean;
  /**
   * 本轮仍在流式中。窗口只在此期间可见,isStreaming=false 才收口:
   * 协议没有前瞻的 final 标记,「最终回复已开始」的回溯判定会在
   * 叙述刚到、后续 tool 未到的间隙误收口再重开(review r2 决定)。
   */
  live: boolean;
  onOpenFile?: (path: string) => void;
  resolveFilePath?: (url: string) => string | null;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const detailedTools = useUiPrefs((state) => state.detailedTools);
  const setDetailedTools = useUiPrefs((state) => state.setDetailedTools);
  const snapshots = entries.map(entrySnapshot);
  const state = summarizeTrack(snapshots, { streaming: live, waiting });
  // 展开时窗口让位给完整条目列表,淘汰不播动画直接清空。
  const rows = open
    ? NO_ROWS
    : selectCollapsedWindow(snapshots, { streaming: live });
  const { evicted, settle } = useEvictedRows(rows, !open);
  // 执行阶段每秒心跳驱动「· 12s」跳动(工具间隙也不停摆);收口后冻结。
  const now = useNow(live);
  const durationLabel = trackDurationLabel(entries, live ? now : null);
  const compactionEntry =
    entries.length === 1 &&
    entries[0]?.kind === "progress" &&
    isContextCompactionMessage(entries[0].message)
      ? entries[0]
      : null;
  let summary: string;
  if (state.kind === "waiting") {
    summary = t("chat.waitingModel");
  } else if (state.kind === "runningTool") {
    summary = humanToolLabel(state.toolName, true, t);
  } else if (state.kind === "progress") {
    summary = t("progress.working");
  } else if (state.kind === "thinking") {
    summary = t("reasoning.thinking");
  } else if (compactionEntry) {
    summary =
      compactionEntry.message.metadata?.phase === "fallback"
        ? t("chat.contextCompaction.fallback")
        : t("chat.contextCompaction.completed");
  } else {
    summary = state.failed
      ? t("chat.toolGroupWithFailures", {
          count: state.steps,
          failed: state.failed,
        })
      : t("chat.toolGroup", { count: state.steps });
  }
  const toggleable = entries.length > 0;
  const entryByKey = new Map(entries.map((entry) => [entry.key, entry]));
  const orderOf = new Map(entries.map((entry, index) => [entry.key, index]));
  // 在场行 + 退出中的行按 entries 原始时间序合并渲染。
  const display = [
    ...rows.map((row) => ({ ...row, exiting: false })),
    ...evicted.map((row) => ({ ...row, exiting: true })),
  ].sort((a, b) => (orderOf.get(a.key) ?? 0) - (orderOf.get(b.key) ?? 0));
  const summaryContent = (
    <>
      {pulsing && <Spinner size={13} />}
      <span key={summary} className="qp-swap-in">
        {summary}
      </span>
      {durationLabel && (
        <span className="shrink-0 tabular-nums">· {durationLabel}</span>
      )}
      {toggleable && (
        <ChevronRight
          size={13}
          className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
            open ? "rotate-90" : ""
          }`}
        />
      )}
    </>
  );
  return (
    <div className="my-1.5">
      <div className="flex items-center gap-2">
        {toggleable ? (
          <button
            type="button"
            onClick={() => setOpen((value) => !value)}
            aria-expanded={open}
            className="inline-flex items-center gap-1.5 rounded-[var(--radius-sm)] px-1 py-1 text-[13px] text-ink-tertiary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink-secondary"
          >
            {summaryContent}
          </button>
        ) : (
          <div className="inline-flex items-center gap-1.5 px-1 py-1 text-[13px] text-ink-tertiary">
            {summaryContent}
          </div>
        )}
        {toggleable && open && (
          <div className="qp-fade-in inline-flex items-center overflow-hidden rounded-[var(--radius-sm)] border border-line text-[11px]">
            <button
              type="button"
              onClick={() => setDetailedTools(false)}
              aria-pressed={!detailedTools}
              className={`px-1.5 py-0.5 transition-colors duration-[var(--dur-fast)] ${
                detailedTools
                  ? "text-ink-muted hover:text-ink-secondary"
                  : "bg-fill-hover text-ink-secondary"
              }`}
            >
              {t("chat.density.summary")}
            </button>
            <button
              type="button"
              onClick={() => setDetailedTools(true)}
              aria-pressed={detailedTools}
              className={`px-1.5 py-0.5 transition-colors duration-[var(--dur-fast)] ${
                detailedTools
                  ? "bg-fill-hover text-ink-secondary"
                  : "text-ink-muted hover:text-ink-secondary"
              }`}
            >
              {t("chat.density.detailed")}
            </button>
          </div>
        )}
      </div>
      {display.length > 0 && (
        <div className="pl-1">
          {display.map((row) => {
            const entry = entryByKey.get(row.key);
            if (!entry) return null;
            return (
              <WindowRow
                key={row.key}
                role={row.role}
                entry={entry}
                exiting={row.exiting}
                onExited={settle}
              />
            );
          })}
        </div>
      )}
      {toggleable && (
        <div className="pl-1">
          {entries.map((entry) => (
            <Collapse key={entry.key} open={open} keepMounted>
              <TrackEntry
                entry={entry}
                onOpenFile={onOpenFile}
                resolveFilePath={resolveFilePath}
              />
            </Collapse>
          ))}
        </div>
      )}
    </div>
  );
}

const NO_ROWS: CollapsedRow[] = [];

/** 折叠窗口选行的输入快照:ProcessEntry → 纯逻辑层的形状。 */
function entrySnapshot(entry: ProcessEntry): TrackEntrySnapshot {
  if (entry.kind === "pair") {
    const status = toolPairStatus(entry.pair);
    return {
      key: entry.key,
      kind: "tool",
      active: status.running,
      completed: status.completed,
      failed: status.failed,
      toolName: entry.pair.name,
    };
  }
  if (entry.kind === "message") {
    return { key: entry.key, kind: "message", active: false };
  }
  return { key: entry.key, kind: entry.kind, active: isActiveEntry(entry) };
}

/**
 * 折叠窗口退出簿记:被淘汰的行保留在渲染树里播 qp-row-out,
 * animationend 后移除。对比在 render 阶段同步进行(与 Collapse 的
 * prevOpen 手法一致),避免行先卸载一帧再回插补动画造成闪跳;
 * reduced-motion 或展开清窗时直接移除。
 */
function useEvictedRows(rows: CollapsedRow[], animate: boolean) {
  const [evicted, setEvicted] = useState<CollapsedRow[]>([]);
  const [prev, setPrev] = useState(rows);
  const changed = rowsDiffer(prev, rows);
  if (changed) setPrev(rows);
  if (!animate) {
    // 展开清窗对退出中的行同样生效:rows 可能早已为空(rowsDiffer
    // 不触发),仅 animate 翻转也要立即清掉,不能等 animationend。
    if (evicted.length > 0) setEvicted([]);
  } else if (changed) {
    if (prefersReducedMotion()) {
      if (evicted.length > 0) setEvicted([]);
    } else {
      const keys = new Set(rows.map((row) => row.key));
      const removed = prev.filter((row) => !keys.has(row.key));
      const kept = evicted.filter(
        (row) =>
          !keys.has(row.key) && !removed.some((gone) => gone.key === row.key),
      );
      setEvicted([...kept, ...removed]);
    }
  }
  const settle = useCallback((key: string) => {
    setEvicted((old) => old.filter((row) => row.key !== key));
  }, []);
  return { evicted, settle };
}

function rowsDiffer(a: CollapsedRow[], b: CollapsedRow[]): boolean {
  if (a.length !== b.length) return true;
  return a.some(
    (row, index) => row.key !== b[index]!.key || row.role !== b[index]!.role,
  );
}

function prefersReducedMotion(): boolean {
  return (
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false
  );
}

/**
 * 折叠窗口单行。同一 key 的行在 current → done 时原位交叉淡化内容
 * (qp-swap-in),不重放入场动画;新行入场 qp-msg-in,淘汰行 qp-row-out。
 * 当前步只展示详情(命令/文件/查询词)——「正在…」已由摘要行表达,
 * 无详情时整行省略,避免复读。
 */
function WindowRow({
  role,
  entry,
  exiting,
  onExited,
}: {
  role: CollapsedRowRole;
  entry: ProcessEntry;
  exiting: boolean;
  onExited: (key: string) => void;
}) {
  const { t } = useTranslation();
  const content = windowRowContent(role, entry, t);
  if (!content) return null;
  return (
    <div
      className={`qp-msg-in flex min-h-6 items-start gap-1.5 px-1 py-0.5 text-[12px] leading-5 text-ink-muted ${
        exiting ? "qp-row-out" : ""
      }`}
      onAnimationEnd={(event) => {
        if (event.animationName === "qp-row-out") onExited(entry.key);
      }}
    >
      <span
        key={role}
        className="qp-swap-in flex min-w-0 items-start gap-1.5"
      >
        {content.icon}
        <span
          className={
            content.clamp ? "line-clamp-2 min-w-0" : "min-w-0 truncate"
          }
        >
          {content.text}
        </span>
      </span>
    </div>
  );
}

/** 各角色的行内容;返回 null 表示该行没有可展示的信息,整行省略。 */
function windowRowContent(
  role: CollapsedRowRole,
  entry: ProcessEntry,
  translate: (
    key: TranslationKey,
    params?: Record<string, string | number>,
  ) => string,
): { icon: ReactNode; text: string; clamp?: boolean } | null {
  if (role === "narration") {
    if (entry.kind !== "message") return null;
    const text = textFromContent(entry.message.content).trim();
    if (!text) return null;
    // 与带图标的行左缘对齐:12px 图标位 + 6px 间距。
    return {
      icon: <span aria-hidden className="w-3 shrink-0" />,
      text,
      clamp: true,
    };
  }
  if (entry.kind === "pair") {
    const detail = recentPairDetail(entry.pair);
    if (role === "current") {
      if (!detail) return null;
      return {
        icon: <CircleDashed size={12} className="mt-1 shrink-0" />,
        text: detail,
      };
    }
    const label = humanToolLabel(entry.pair.name, false, translate);
    return {
      icon: <Check size={12} className="mt-1 shrink-0" />,
      text: detail ? `${label} · ${detail}` : label,
    };
  }
  if (entry.kind === "progress") {
    const text = textFromContent(entry.message.content).trim();
    if (!text) return null;
    return {
      icon: <CircleDashed size={12} className="mt-1 shrink-0" />,
      text,
    };
  }
  return null;
}

/** memo:useNow 每秒心跳只该刷新摘要行的计时文案,条目行引用不变直接跳过。 */
const TrackEntry = memo(function TrackEntry({
  entry,
  onOpenFile,
  resolveFilePath,
}: {
  entry: ProcessEntry;
  onOpenFile?: (path: string) => void;
  resolveFilePath?: (url: string) => string | null;
}) {
  if (entry.kind === "reasoning") {
    return <ReasoningBlock message={entry.message} />;
  }
  if (entry.kind === "progress") {
    return <ProgressCard message={entry.message} />;
  }
  if (entry.kind === "message") {
    return (
      <div
        id={`message-${entry.key}`}
        className="py-1 text-sm text-ink-secondary"
      >
        <MessageContent
          content={entry.message.content}
          markdown
          onOpenFile={onOpenFile}
          resolveFilePath={resolveFilePath}
        />
      </div>
    );
  }
  return <ToolCard pair={entry.pair} onOpenFile={onOpenFile} />;
});

/**
 * 该消息是否代表「助手转去继续执行」。文本之后一旦出现这类消息,说明
 * 它只是中间叙述,应折叠进执行轨道的时间线位置。progress 不算触发:
 * 压缩等进度可能出现在最终回复之后,不能因此把答案折叠掉;显式发送
 * 文件渲染在轨道外,同样不触发。普通写文件属于执行步骤,会触发折叠。
 * 工具名可能只出现在 output 上,与
 * buildToolPair 一致地按 call → output 顺序解析,保证分类不劈叉。
 */
function startsTrackWork(
  message: StreamMessage,
  outputsByCallId: Map<string, StreamMessage>,
): boolean {
  if (message.type === "reasoning") return true;
  if (!isToolCall(message.type)) return false;
  const data = toolData(message);
  const name =
    stringValue(data.name) ||
    stringValue(
      toolData(outputsByCallId.get(stringValue(data.call_id)) ?? null).name,
    );
  // 名称未知(仍在流式)时不触发折叠,避免交付调用先折叠正文再弹回。
  if (!name) return false;
  return name !== "send_file_to_user";
}

/**
 * 内联 `<thinking>` 标签的模型不走结构化 reasoning 通道。展示层把
 * 思考段拆成合成 reasoning 消息(并入执行轨道),正文只留干净文本;
 * 未闭合块按流式思考中处理。无标签消息原样返回,保持引用不变。
 */
function presentInlineThinking(message: StreamMessage): StreamMessage[] {
  if (!isOrdinaryAssistantMessage(message)) return [message];
  let thinking = "";
  let open = false;
  let changed = false;
  let templatePart: TextContent | null = null;
  const content: ContentBlock[] = [];
  for (const part of message.content) {
    if (part.type !== "text" || typeof part.text !== "string") {
      content.push(part);
      continue;
    }
    const split = splitInlineThinking(part.text);
    if (!split.changed) {
      content.push(part);
      continue;
    }
    changed = true;
    templatePart ??= part;
    open = open || split.open;
    if (split.thinking) {
      thinking = thinking ? `${thinking}\n\n${split.thinking}` : split.thinking;
    }
    if (split.text) content.push({ ...part, text: split.text });
  }
  if (!changed) return [message];
  const result: StreamMessage[] = [];
  if (thinking || open) {
    result.push({
      ...message,
      id: `${message.id}:thinking`,
      type: "reasoning",
      status:
        open && message.status === "in_progress" ? "in_progress" : "completed",
      content: [
        { ...(templatePart as TextContent), type: "text", text: thinking },
      ],
    });
  }
  if (content.length > 0) result.push({ ...message, content });
  return result;
}

function isOrdinaryAssistantMessage(message: StreamMessage): boolean {
  return (
    message.role === "assistant" &&
    message.type !== "reasoning" &&
    message.type !== "progress" &&
    !isToolCall(message.type) &&
    !isToolOutput(message.type) &&
    message.content.length > 0
  );
}

/**
 * 消息动作行：默认可见但极轻（触屏也能用），hover 才升一档对比。
 */
function MessageActions({
  text,
  regeneratePrompt,
}: {
  text: string;
  regeneratePrompt: string;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const sendMessage = useChatStore((state) => state.sendMessage);
  const busy = useChatStore((state) => state.isStreaming || state.isSubmitting);
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用（非安全上下文 / 权限拒绝）时静默降级。
    }
  };

  return (
    <div className="qp-fade-in -ml-1.5 mt-1.5 flex items-center gap-0.5">
      <ActionButton
        label={copied ? t("message.copied") : t("message.copy")}
        onClick={() => void copy()}
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </ActionButton>
      {regeneratePrompt && (
        <ActionButton
          label={t("message.regenerate")}
          disabled={busy}
          onClick={() => void sendMessage(regeneratePrompt, navigate)}
        >
          <RefreshCw size={14} />
        </ActionButton>
      )}
    </div>
  );
}

function ActionButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className="flex h-7 w-7 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink-secondary disabled:pointer-events-none disabled:opacity-40"
    >
      {children}
    </button>
  );
}

export function groupIntoTurns(messages: StreamMessage[]): Turn[] {
  const turns: Turn[] = [];
  for (const message of messages) {
    if (message.role === "user") {
      turns.push({ id: message.id, role: "user", messages: [message] });
      continue;
    }
    const last = turns.at(-1);
    if (!last || last.role === "user") {
      turns.push({
        id: `assistant-${message.id}`,
        role: "assistant",
        messages: [message],
      });
    } else {
      last.messages.push(message);
    }
  }
  return turns;
}

function previousUserText(turns: Turn[], index: number): string {
  const previous = turns[index - 1];
  return previous?.role === "user" ? plainText(previous.messages) : "";
}

/** 取一轮里正文消息的纯文本（跳过推理、进度与工具卡）。 */
function plainText(messages: StreamMessage[] | undefined): string {
  if (!messages) return "";
  return messages
    .filter(
      (message) =>
        message.type !== "reasoning" &&
        message.type !== "progress" &&
        !isToolCall(message.type) &&
        !isToolOutput(message.type),
    )
    .flatMap((message) => message.content)
    .filter(
      (part): part is Extract<ContentBlock, { type: "text" }> =>
        part.type === "text" && typeof part.text === "string",
    )
    .map((part) => part.text)
    .join("\n\n")
    .trim();
}

function isToolCall(type: StreamMessage["type"]) {
  return (
    type === "plugin_call" ||
    type === "function_call" ||
    type === "mcp_tool_call"
  );
}

function isToolOutput(type: StreamMessage["type"]) {
  return (
    type === "plugin_call_output" ||
    type === "function_call_output" ||
    type === "mcp_tool_call_output"
  );
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}
