import {
  Fragment,
  memo,
  useEffect,
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
import { useTranslation } from "../../lib/i18n";
import {
  summarizeTrack,
  type TrackEntrySnapshot,
} from "../../lib/executionTrack";
import { buildTimeline } from "../../lib/turnTimeline";
import { splitInlineThinking } from "../../lib/inlineThinking";
import { historyTurnDuration } from "../../lib/historyTurnDuration";
import { formatDuration, getMessageTiming } from "../../lib/messageTiming";
import { useNow } from "../../lib/useNow";
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

/**
 * 流式尾轮的最小高度:问题锚顶模型需要问题下方有足够的可滚动空间,
 * 否则短回答时 scrollTop 到不了「问题在视口顶部」的位置。只挂在
 * 「本会话流式出生的最后一轮」上——历史轮/被接替的轮立即释放,
 * 释放的时机恰好是下一个问题重新锚定的那一帧,不可见。
 */
const TAIL_MIN_HEIGHT = "min-h-[calc(100dvh-16rem)]";

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
    <div
      data-chat-content
      className="mx-auto w-full max-w-[48rem] px-6 pb-12 pt-5 sm:px-8"
    >
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
            tail={index === lastIndex}
            activeMessageId={activeMessageId}
            userTimestamp={previousUserTimestamp(turns, index)}
            assistantTimestamp={messageTimestamp(turn.messages.at(-1))}
          />
        ),
      )}
      {showPendingTurn && (
        <div data-testid="turn-assistant" className={`mb-10 ${TAIL_MIN_HEIGHT}`}>
          <AssistantHeader />
          <TurnFlow pieces={[]} foldEntries={[]} waiting pulsing live />
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
    <div data-testid="turn-user" className="mb-8 flex justify-end">
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

type ProcessEntry =
  | { kind: "reasoning"; key: string; message: StreamMessage }
  | { kind: "progress"; key: string; message: StreamMessage }
  | { kind: "pair"; key: string; pair: ToolPair };

/**
 * 一轮回复按时间序渲染的片段:连续的工具/思考槽位合成一个可折叠工作
 * 段(run);叙述、答案与突出卡是恒可见节点(visible),同时天然充当
 * run 的分隔符。时间线严格 append-only,每个 run 在被后续内容接替时
 * 各自收拢一次(渐进收口),此后不再变。
 */
type FlowPiece =
  | { type: "run"; key: string; entries: ProcessEntry[] }
  | { type: "visible"; key: string; node: ReactNode };

interface AssistantTurnProps {
  messages: StreamMessage[];
  showActions: boolean;
  regeneratePrompt: string;
  /** 本轮是流式进行中的最后一轮（无审批卡挂起）。 */
  streaming: boolean;
  /** 本轮是列表里最后一轮(配合流式出生保持锚定空间)。 */
  tail: boolean;
  activeMessageId?: string;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
  userTimestamp?: unknown;
  assistantTimestamp?: unknown;
}

const AssistantTurn = memo(function AssistantTurn({
  messages,
  showActions,
  regeneratePrompt,
  streaming,
  tail,
  activeMessageId,
  onOpenFile,
  onOpenChange,
  userTimestamp,
  assistantTimestamp,
}: AssistantTurnProps) {
  // 入场动画已整体移除(设计裁决:内容直接出现,不做淡入)。
  // 流式出生的轮在它还是尾轮期间保持锚定空间(见 TAIL_MIN_HEIGHT);
  // 该判定只在挂载时定一次,与原 enterAnimation 同语义。
  const bornStreaming = useRef(streaming).current;
  // 内联 <thinking> 标签的模型:思考段拆成轨道条目,正文只留干净文本。
  const presented = useMemo(
    () => messages.flatMap(presentInlineThinking),
    [messages],
  );
  const copyText = plainText(presented);
  const turnChanges = collectFileChanges(messages);
  const turnArtifacts = collectConversationArtifacts(messages);
  const resolveFilePath = (href: string) =>
    resolveConversationFileLink(href, turnArtifacts);

  /* 呈现承诺层(lib/turnTimeline)产出 append-only 的槽位序列;这里只
   * 负责把槽位物化成节点:fold 槽合并成可折叠段落,交付产物/失败进度
   * 就地升级为恒可见的突出卡,答案恒可见。流式期间没有任何已落地内容
   * 会换容器或换形态——「文字先当正文渲染、下个工具一到又被抽进轨道」
   * 的旧行为从模型上消灭了(叙述与答案同构,边界只在收口时被消费)。 */
  const byId = new Map(presented.map((message) => [message.id, message]));
  const slots = buildTimeline(presented);
  const foldEntries: ProcessEntry[] = [];
  const pieces: FlowPiece[] = [];
  let run: ProcessEntry[] = [];
  const flushRun = () => {
    if (run.length > 0) {
      pieces.push({ type: "run", key: `run-${run[0]!.key}`, entries: run });
    }
    run = [];
  };
  const fold = (entry: ProcessEntry) => {
    foldEntries.push(entry);
    run.push(entry);
  };
  const visible = (key: string, node: ReactNode) => {
    flushRun();
    pieces.push({ type: "visible", key, node });
  };
  for (const slot of slots) {
    const message = byId.get(slot.messageId)!;
    if (slot.kind === "reasoning") {
      fold({ kind: "reasoning", key: slot.key, message });
      continue;
    }
    if (slot.kind === "progress") {
      // 失败恒可见(r10 决定);压缩进行中沿用独立卡,完成后归入轨道。
      const failed =
        message.status === "failed" || message.status === "cancelled";
      const activeCompaction =
        isContextCompactionMessage(message) && message.status !== "completed";
      if (failed || activeCompaction) {
        visible(slot.key, <ProgressCard key={slot.key} message={message} />);
      } else {
        fold({ kind: "progress", key: slot.key, message });
      }
      continue;
    }
    if (slot.kind === "tool") {
      const orphan = slot.outputId === slot.messageId;
      const output = slot.outputId ? byId.get(slot.outputId) ?? null : null;
      const pair = buildToolPair(orphan ? null : message, output);
      if (shouldPresentArtifactPair(pair, turnArtifacts)) {
        visible(
          slot.key,
          <div id={`message-${slot.key}`} key={slot.key}>
            <ToolCard pair={pair} onOpenFile={onOpenFile} prominentArtifact />
          </div>,
        );
      } else {
        fold({ kind: "pair", key: slot.key, pair });
      }
      continue;
    }
    // narration:无论 fold 还是 answer 角色都以同一节点恒可见——模型的
    // 讲述是回复的一部分,永远留在正文流里(Codex 式)。这也让收口零
    // 结构变动:工具/思考段各自渐进收起,文字从不动。叙述同时天然充当
    // run 的分隔符——每当模型开口说话,它前面的工作段就被接替而收拢。
    visible(
      slot.key,
      <div
        id={`message-${slot.key}`}
        key={slot.key}
        className={`rounded-[6px] py-1 ${
          activeMessageId === slot.key
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
      </div>,
    );
  }
  flushRun();

  // 正文流式输出时脉冲让位给文字本身;其余流式阶段(等待/思考/工具间隙)保持活动感。
  const hasStreamingText = presented.some(
    (message) =>
      isOrdinaryAssistantMessage(message) && message.status === "in_progress",
  );
  const waiting = streaming && slots.length === 0;
  const pulsing = streaming && !hasStreamingText;

  return (
    <div
      data-testid="turn-assistant"
      className={`mb-10${tail && bornStreaming ? ` ${TAIL_MIN_HEIGHT}` : ""}`}
    >
      <AssistantHeader />
      <TurnFlow
        pieces={waiting ? [] : pieces}
        foldEntries={foldEntries}
        waiting={waiting}
        pulsing={pulsing}
        live={streaming}
        onOpenFile={onOpenFile}
        userTimestamp={userTimestamp}
        assistantTimestamp={assistantTimestamp}
      />
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
    Object.is(previous.tail, next.tail) &&
    Object.is(previous.onOpenFile, next.onOpenFile) &&
    Object.is(previous.onOpenChange, next.onOpenChange) &&
    Object.is(previous.userTimestamp, next.userTimestamp) &&
    Object.is(previous.assistantTimestamp, next.assistantTimestamp)
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
          <FileDiff size={14} strokeWidth={1.8} />
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
          className="flex w-full items-center gap-1 border-t border-line px-3.5 py-2 text-left text-[12px] text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink"
        >
          {expanded
            ? t("chat.changes.showLess")
            : t("chat.changes.showMore", { count: hiddenCount })}
          <ChevronDown
            size={14}
            strokeWidth={1.8}
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
  const elapsed = stop - start;
  if (elapsed <= 0) return "";
  return formatDuration(elapsed);
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
 * 一轮回复的执行流:摘要头 + append-only 时间线,收口是**渐进式**的。
 *
 * 每个工作段(连续的工具/思考)只在它还是「活跃的最后一段」时展开;
 * 一旦被接替——模型开口说话、交付产物、或开始下一段工作前的叙述——
 * 它就地收拢(一次 280ms 结构过渡)。于是答案开始流式时,它上方的
 * 过程已经让位;流结束的时刻**没有任何结构变动**,用户正读到的答案
 * 纹丝不动。这是问题锚顶滚动模型的前提:结尾若再来一次大折叠,会把
 * 正在读的答案往上拽。
 *
 * 尾段没有后继(纯行动收尾)时,流结束后静置 600ms 再收——读作
 * 「整理」而不是「抽走」。用户手动点过开合,以他的选择为准,自动
 * 行为不再覆盖(展开 = 全部段落展开)。
 * 摘要状态机在 lib/executionTrack.ts,槽位与折叠边界在 lib/turnTimeline.ts。
 */
function TurnFlow({
  pieces,
  foldEntries,
  waiting,
  pulsing,
  live,
  onOpenFile,
  userTimestamp,
  assistantTimestamp,
}: {
  pieces: FlowPiece[];
  /** 全部过程条目(时间序),驱动摘要行的状态与计数。 */
  foldEntries: ProcessEntry[];
  waiting: boolean;
  pulsing: boolean;
  /** 本轮仍在流式中。 */
  live: boolean;
  onOpenFile?: (path: string) => void;
  userTimestamp?: unknown;
  assistantTimestamp?: unknown;
}) {
  const { t } = useTranslation();
  // 用户手动点过开合就以他的选择为准,自动行为不再覆盖。
  const [manualOpen, setManualOpen] = useState<boolean | null>(null);
  // 尾段收口延迟:live 结束后静置 600ms 再收拢仍展开的最后一段。
  const [settling, setSettling] = useState(live);
  useEffect(() => {
    if (live) {
      setSettling(true);
      return;
    }
    if (!settling) return;
    const timer = window.setTimeout(() => setSettling(false), 600);
    return () => window.clearTimeout(timer);
  }, [live, settling]);
  const lastPiece = pieces.length > 0 ? pieces[pieces.length - 1] : undefined;
  // 自动展开只给「活跃的最后一段」;被接替的段落已各自收拢。
  const autoOpenLast = (live || settling) && lastPiece?.type === "run";
  const headerOpen = manualOpen ?? autoOpenLast;
  const snapshots = foldEntries.map(entrySnapshot);
  const state = summarizeTrack(snapshots, { streaming: live, waiting });
  // 执行阶段每秒心跳驱动「· 12s」跳动(工具间隙也不停摆);收口后冻结。
  const now = useNow(live);
  const durationLabel =
    trackDurationLabel(foldEntries, live ? now : null) ||
    historyTurnDuration(userTimestamp, assistantTimestamp);
  const compactionEntry =
    foldEntries.length === 1 &&
    foldEntries[0]?.kind === "progress" &&
    isContextCompactionMessage(foldEntries[0].message)
      ? foldEntries[0]
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
  } else if (durationLabel) {
    summary = state.failed
      ? t("chat.workedForWithFailures", {
          duration: durationLabel,
          failed: state.failed,
        })
      : t("chat.workedFor", { duration: durationLabel });
  } else {
    summary = state.failed
      ? t("chat.toolGroupWithFailures", {
          count: state.steps,
          failed: state.failed,
        })
      : t("chat.toolGroup", { count: state.steps });
  }
  const toggleable = foldEntries.length > 0;
  // 摘要头只在有过程可折(或等待首帧)时出现:纯文本回答不需要一行
  // 「已完成 1 个步骤」的噪音。无过程条目且无失败工具则不画头。
  const showHeader = waiting || toggleable;
  const showDurationSuffix =
    Boolean(durationLabel) &&
    (state.kind !== "done" || Boolean(compactionEntry));
  const summaryContent = (
    <>
      {pulsing && <Spinner size={13} />}
      {/* 摘要文字直接替换,不做交叉淡化——状态变化本身就是信息,
          给它加动画反而把注意力引向动画。 */}
      <span>{summary}</span>
      {showDurationSuffix && (
        <span className="shrink-0 tabular-nums">· {durationLabel}</span>
      )}
      {toggleable && (
        <ChevronRight
          size={14}
          strokeWidth={1.8}
          className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
            headerOpen ? "rotate-90" : ""
          }`}
        />
      )}
    </>
  );
  return (
    <div className="my-1.5">
      {showHeader && (
        <div className="flex items-center gap-2">
          {toggleable ? (
            <button
              type="button"
              onClick={() => setManualOpen(!headerOpen)}
              aria-expanded={headerOpen}
              className="inline-flex items-center gap-1.5 rounded-[var(--radius-sm)] px-1 py-1 text-[13px] text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink"
            >
              {summaryContent}
            </button>
          ) : (
            <div className="inline-flex items-center gap-1.5 px-1 py-1 text-[13px] text-ink-tertiary">
              {summaryContent}
            </div>
          )}
        </div>
      )}
      {pieces.map((piece) =>
        piece.type === "run" ? (
          // 每段 run 一个 Collapse:自动态只有「活跃的最后一段」展开,
          // 被接替的段落各自收拢过一次,不再重开。keepMounted 保住条目
          // 内部展开状态;struct 档 = 280ms 高度过渡 + 内容淡出。
          <Collapse
            key={piece.key}
            open={manualOpen ?? (autoOpenLast && piece === lastPiece)}
            keepMounted
            struct
            className="pl-1"
          >
            {piece.entries.map((entry) => (
              <TrackEntry
                key={entry.key}
                entry={entry}
                onOpenFile={onOpenFile}
              />
            ))}
          </Collapse>
        ) : (
          <Fragment key={piece.key}>{piece.node}</Fragment>
        ),
      )}
    </div>
  );
}

/** 摘要状态机的输入快照:ProcessEntry → 纯逻辑层的形状。 */
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
  return {
    key: entry.key,
    kind: entry.kind,
    active: isActiveEntry(entry),
  };
}

/** memo:useNow 每秒心跳只该刷新摘要行的计时文案,条目行引用不变直接跳过。 */
const TrackEntry = memo(function TrackEntry({
  entry,
  onOpenFile,
}: {
  entry: ProcessEntry;
  onOpenFile?: (path: string) => void;
}) {
  if (entry.kind === "reasoning") {
    return <ReasoningBlock message={entry.message} />;
  }
  if (entry.kind === "progress") {
    return <ProgressCard message={entry.message} />;
  }
  return <ToolCard pair={entry.pair} onOpenFile={onOpenFile} />;
});

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
        {copied ? <Check size={14} strokeWidth={1.8} /> : <Copy size={14} strokeWidth={1.8} />}
      </ActionButton>
      {regeneratePrompt && (
        <ActionButton
          label={t("message.regenerate")}
          disabled={busy}
          onClick={() => void sendMessage(regeneratePrompt, navigate)}
        >
          <RefreshCw size={14} strokeWidth={1.8} />
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
      className="flex h-7 w-7 items-center justify-center rounded-[var(--radius-sm)] text-icon transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-icon-strong disabled:pointer-events-none disabled:opacity-40"
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

function previousUserTimestamp(turns: Turn[], index: number): unknown {
  const previous = turns[index - 1];
  if (previous?.role !== "user") return undefined;
  return messageTimestamp(previous.messages[0]);
}

function messageTimestamp(message: StreamMessage | undefined): unknown {
  return message?.metadata?.timestamp;
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
