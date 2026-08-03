import { useState, type ReactNode } from "react";
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
  collectFileChanges,
  directoryOf,
  shortenPath,
  totalChangeStats,
  type FileChange,
} from "../../lib/fileChanges";
import { useTranslation } from "../../lib/i18n";
import type { ContentBlock } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { useChatStore } from "../../stores/chat";
import { PotatoMark } from "../brand/PotatoMark";
import { ApprovalCard } from "./ApprovalCard";
import { ChangeStat } from "./ConversationSidePanel";
import { isArtifactTool } from "./FileToolCard";
import { MessageContent } from "./MessageContent";
import { isContextCompactionMessage, ProgressCard } from "./ProgressCard";
import { ReasoningBlock } from "./ReasoningBlock";
import {
  buildToolPair,
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
  const turns = groupIntoTurns(messages);
  const pendingApprovals = useChatStore((state) => state.pendingApprovals);
  const isStreaming = useChatStore((state) => state.isStreaming);
  const lastIndex = turns.length - 1;
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
            activeMessageId={activeMessageId}
          />
        ),
      )}
      {pendingApprovals.map((approval) => (
        <ApprovalCard key={approval.request_id} approval={approval} />
      ))}
    </div>
  );
}

function UserTurn({
  messages,
  activeMessageId,
}: {
  messages: StreamMessage[];
  activeMessageId?: string;
}) {
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
}

type RenderItem =
  | { kind: "node"; key: string; node: ReactNode }
  | { kind: "pair"; key: string; pair: ToolPair }
  | { kind: "process"; key: string; entry: ProcessEntry };

type ProcessEntry =
  | { kind: "reasoning"; key: string; message: StreamMessage }
  | { kind: "progress"; key: string; message: StreamMessage }
  | { kind: "message"; key: string; message: StreamMessage }
  | { kind: "pair"; key: string; pair: ToolPair };

/** 可并入整轮执行组:已结束的成功/失败步骤都收进去；运行中和产物仍需突出。 */
function isGroupable(pair: ToolPair): boolean {
  return !toolPairStatus(pair).running && !isArtifactTool(pair.name);
}

function AssistantTurn({
  messages,
  showActions,
  regeneratePrompt,
  activeMessageId,
  onOpenFile,
  onOpenChange,
}: {
  messages: StreamMessage[];
  showActions: boolean;
  regeneratePrompt: string;
  activeMessageId?: string;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
}) {
  const pairedOutputs = new Set<string>();
  const copyText = plainText(messages);
  const turnChanges = collectFileChanges(messages);

  // WorkBuddy keeps the conversational narration that happens before/among
  // tool calls inside the execution disclosure. Only the final assistant
  // message remains in the collapsed view. A plain answer without any
  // execution steps is left untouched.
  const hasProcess = messages.some(isProcessMessage);
  const ordinaryMessages = messages.filter(isOrdinaryAssistantMessage);
  const finalOrdinaryMessageId = ordinaryMessages.at(-1)?.id;
  const collapseIntermediateMessages =
    hasProcess && ordinaryMessages.length > 1 && finalOrdinaryMessageId;

  const items: RenderItem[] = [];
  for (const message of messages) {
    if (message.type === "reasoning") {
      const entry: ProcessEntry = {
        kind: "reasoning",
        key: message.id,
        message,
      };
      if (isCompletedProcessMessage(message)) {
        items.push({ kind: "process", key: message.id, entry });
      } else {
        items.push({
          kind: "node",
          key: message.id,
          node: <ReasoningBlock key={message.id} message={message} />,
        });
      }
      continue;
    }
    if (message.type === "progress") {
      const entry: ProcessEntry = {
        kind: "progress",
        key: message.id,
        message,
      };
      if (isCompletedProcessMessage(message)) {
        items.push({ kind: "process", key: message.id, entry });
      } else {
        items.push({
          kind: "node",
          key: message.id,
          node: <ProgressCard key={message.id} message={message} />,
        });
      }
      continue;
    }
    if (isToolCall(message.type)) {
      const callId = stringValue(toolData(message).call_id);
      const output = messages.find(
        (candidate) =>
          isToolOutput(candidate.type) &&
          stringValue(toolData(candidate).call_id) === callId,
      );
      if (output) pairedOutputs.add(output.id);
      const pair = buildToolPair(message, output ?? null);
      if (isGroupable(pair)) {
        items.push({
          kind: "process",
          key: message.id,
          entry: { kind: "pair", key: message.id, pair },
        });
      } else {
        items.push({ kind: "pair", key: message.id, pair });
      }
      continue;
    }
    if (isToolOutput(message.type)) {
      if (pairedOutputs.has(message.id)) continue;
      const pair = buildToolPair(null, message);
      if (isGroupable(pair)) {
        items.push({
          kind: "process",
          key: message.id,
          entry: { kind: "pair", key: message.id, pair },
        });
      } else {
        items.push({ kind: "pair", key: message.id, pair });
      }
      continue;
    }
    if (message.content.length === 0) continue;
    if (
      collapseIntermediateMessages &&
      message.status === "completed" &&
      isOrdinaryAssistantMessage(message) &&
      message.id !== finalOrdinaryMessageId
    ) {
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
          <MessageContent content={message.content} markdown />
        </div>
      ),
    });
  }

  /* WorkBuddy 把整轮执行管线收成一行「已完成 … ›」，而不是只折叠
   * 恰好连续的工具卡。先收集所有已完成的思考/进度/工具步骤，再把
   * 正文和运行中、失败、产物等需要突出的内容按原顺序渲染。 */
  const rendered: ReactNode[] = [];
  const processEntries = items.flatMap((item) =>
    item.kind === "process" ? [item.entry] : [],
  );
  let executionRendered = false;
  for (const item of items) {
    if (item.kind === "process") {
      if (!executionRendered) {
        rendered.push(
          <ExecutionGroup
            key={`execution-${processEntries[0]!.key}`}
            entries={processEntries}
            onOpenFile={onOpenFile}
          />,
        );
        executionRendered = true;
      }
      continue;
    }
    if (item.kind === "pair") {
      rendered.push(
        <div id={`message-${item.key}`} key={item.key}>
          <ToolCard pair={item.pair} onOpenFile={onOpenFile} />
        </div>,
      );
    } else {
      rendered.push(item.node);
    }
  }

  return (
    <div data-testid="turn-assistant" className="qp-msg-in mb-10">
      <div className="mb-2 flex items-center gap-2 text-[14px] font-semibold text-ink-secondary">
        <span className="flex h-6 w-6 items-center justify-center rounded-[7px] bg-btn-primary text-btn-primary-ink">
          <PotatoMark size={16} />
        </span>
        <span>{APP_NAME}</span>
      </div>
      {rendered}
      {turnChanges.length > 0 && (
        <FileChangesCard changes={turnChanges} onOpenChange={onOpenChange} />
      )}
      {showActions && copyText && (
        <MessageActions text={copyText} regeneratePrompt={regeneratePrompt} />
      )}
    </div>
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

/** 折叠的执行组:一行摘要,点开才展开逐条过程行。 */
function ExecutionGroup({
  entries,
  onOpenFile,
}: {
  entries: ProcessEntry[];
  onOpenFile?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const stepCount = Math.max(
    1,
    entries.filter((entry) => entry.kind !== "message").length,
  );
  const failedCount = entries.filter(
    (entry) => entry.kind === "pair" && toolPairStatus(entry.pair).failed,
  ).length;
  const compactionEntry =
    entries.length === 1 &&
    entries[0]?.kind === "progress" &&
    isContextCompactionMessage(entries[0].message)
      ? entries[0]
      : null;
  const summary = compactionEntry
    ? compactionEntry.message.metadata?.phase === "fallback"
      ? t("chat.contextCompaction.fallback")
      : t("chat.contextCompaction.completed")
    : failedCount
    ? t("chat.toolGroupWithFailures", {
        count: stepCount,
        failed: failedCount,
      })
    : t("chat.toolGroup", { count: stepCount });
  return (
    <div className="my-1.5">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        className="inline-flex items-center gap-1 rounded-[var(--radius-sm)] px-1 py-1 text-[13px] text-ink-tertiary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink-secondary"
      >
        <span>{summary}</span>
        <ChevronRight
          size={13}
          className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
            open ? "rotate-90" : ""
          }`}
        />
      </button>
      {open && (
        <div className="mt-0.5 space-y-1 pl-1">
          {entries.map((entry) => {
            if (entry.kind === "reasoning") {
              return (
                <ReasoningBlock
                  key={entry.key}
                  message={entry.message}
                  compact
                />
              );
            }
            if (entry.kind === "progress") {
              return <ProgressCard key={entry.key} message={entry.message} />;
            }
            if (entry.kind === "message") {
              return (
                <div
                  key={entry.key}
                  id={`message-${entry.key}`}
                  className="py-1 text-sm text-ink-secondary"
                >
                  <MessageContent content={entry.message.content} markdown />
                </div>
              );
            }
            return (
              <ToolCard
                key={entry.key}
                pair={entry.pair}
                onOpenFile={onOpenFile}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

function isCompletedProcessMessage(message: StreamMessage): boolean {
  return message.status === "completed";
}

function isProcessMessage(message: StreamMessage): boolean {
  return (
    message.type === "reasoning" ||
    message.type === "progress" ||
    isToolCall(message.type) ||
    isToolOutput(message.type)
  );
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
    <div className="-ml-1.5 mt-1.5 flex items-center gap-0.5">
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
