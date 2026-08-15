import { Spinner } from "../ui/Spinner";
import { ToolDisclosure } from "./ToolDisclosure";
import type { DataContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { t, useTranslation, type TranslationKey } from "../../lib/i18n";
import { parseQpMeta, qpString, type QpMeta } from "../../lib/toolMeta";
import { JsonView } from "./JsonView";
import { ShellToolCard } from "./ShellToolCard";
import { FileToolCard, isFileTool } from "./FileToolCard";
import { TrackSummary } from "./TrackRow";

export interface ToolPair {
  call: StreamMessage | null;
  output: StreamMessage | null;
  callId: string | null;
  name: string;
  arguments: string;
  result: string;
  state: string | null;
  /** 结构化结果契约(qp meta);历史/取消/未迁移工具为 null。 */
  meta: QpMeta | null;
  /** 后端 ToolUISpec 声明的图标(TOOL_CALL_START 的 ui.icon);可缺。 */
  uiIcon: string;
}

/**
 * One status source for every tool card and the execution disclosure.
 * Backend history can carry the lifecycle on either the message envelope or
 * the tool-result `state`, so checking only one of them can show a running or
 * failed tool as completed.
 */
export function toolPairStatus(pair: ToolPair) {
  const outputStatus = pair.output?.status;
  // A call envelope can reach completed before its output envelope is
  // appended. Keep that gap visible as an active step instead of hiding it
  // inside the completed execution summary.
  const waitingForOutput = Boolean(pair.call && !pair.output);
  const running =
    waitingForOutput ||
    isRunningToolState(pair.state) ||
    outputStatus === "created" ||
    outputStatus === "in_progress";
  const failed =
    outputStatus === "failed" ||
    outputStatus === "cancelled" ||
    isFailedToolState(pair.state);
  const completed =
    Boolean(pair.output) &&
    !running &&
    !failed &&
    isSuccessfulToolState(pair.state);
  return { running, failed, completed };
}

/**
 * 无 hook 的类型分发器:流式首帧可能拿不到真实工具名,同一条目会在
 * 通用卡与 Shell/File 卡之间切换。hook 全部下沉到具体卡组件,分发器
 * 自身不持 hook,避免切换时 hook 数量变化(Rendered fewer hooks)。
 */
export function ToolCard({
  pair,
  onOpenFile,
  onOpenChange,
  prominentArtifact = false,
  embedded = false,
  shimmer = false,
  tail = false,
  open,
  onToggle,
}: {
  pair: ToolPair;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
  prominentArtifact?: boolean;
  /** 组内原始层:shell 直接出纯文本块,文件保留 ± 行。 */
  embedded?: boolean;
  shimmer?: boolean;
  tail?: boolean;
  open?: boolean;
  onToggle?: () => void;
}) {
  if (toolPairStatus(pair).failed && !prominentArtifact) {
    return <FailedToolRow pair={pair} open={open} onToggle={onToggle} />;
  }
  if (pair.name === "execute_shell_command") {
    return (
      <ShellToolCard
        pair={pair}
        embedded={embedded}
        shimmer={shimmer}
        tail={tail}
        open={open}
        onToggle={onToggle}
      />
    );
  }
  if (isFileTool(pair.name)) {
    return (
      <FileToolCard
        pair={pair}
        onOpenFile={onOpenFile}
        onOpenChange={onOpenChange}
        prominentArtifact={prominentArtifact}
        shimmer={shimmer}
        open={open}
        onToggle={onToggle}
      />
    );
  }
  return (
    <GenericToolCard
      pair={pair}
      embedded={embedded}
      shimmer={shimmer}
      open={open}
      onToggle={onToggle}
    />
  );
}

function GenericToolCard({
  pair,
  embedded = false,
  shimmer = false,
  open,
  onToggle,
}: {
  pair: ToolPair;
  embedded?: boolean;
  shimmer?: boolean;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const { running, failed } = toolPairStatus(pair);
  const summary = argumentSummary(pair.arguments, t);
  const label = failed
    ? humanToolName(pair.name, t)
    : humanToolLabel(pair.name, running, t);

  const detail = (
    <div className="space-y-3">
      <section>
        <div className="mb-1 text-xs font-medium text-ink-tertiary">
          {t("tool.parameters")}
        </div>
        <JsonView value={pair.arguments || {}} />
      </section>
      <section>
        <div className="mb-1 text-xs font-medium text-ink-tertiary">
          {t("tool.result")}
        </div>
        {pair.result ? (
          <JsonView value={richOutputText(pair.result)} />
        ) : (
          <div className="text-xs text-ink-tertiary">
            {running ? t("tool.running") : t("tool.noResult")}
          </div>
        )}
      </section>
    </div>
  );

  if (embedded) {
    return (
      <div className="mb-1 mt-0.5 max-h-[min(20rem,42vh)] overflow-y-auto overscroll-contain rounded-[var(--radius-md)] bg-surface px-3 py-2">
        {detail}
      </div>
    );
  }

  const toggle = (
    <>
      {pair.uiIcon && (
        <span aria-hidden className="shrink-0 text-[14px] leading-none text-ink-muted">
          {pair.uiIcon}
        </span>
      )}
      <span className={shimmer ? "qp-shimmer min-w-0 truncate" : "min-w-0 truncate"}>
        <TrackSummary
          verb={label}
          object={summary}
          shimmer={shimmer}
          failed={failed}
        />
      </span>
    </>
  );
  const after = running ? (
    <ToolStatus running={running} failed={failed} />
  ) : null;

  return (
    <ToolDisclosure
      toggle={toggle}
      after={after}
      failed={failed}
      open={open}
      onToggle={onToggle}
      detailClassName="mb-1 mt-0.5 max-h-[min(20rem,42vh)] overflow-y-auto overscroll-contain rounded-[var(--radius-md)] bg-surface px-3 py-2"
    >
      {detail}
    </ToolDisclosure>
  );
}

/** 失败工具:红字安静行,点开才见完整错误。 */
function FailedToolRow({
  pair,
  open,
  onToggle,
}: {
  pair: ToolPair;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const reason = toolFailureSummary(pair);
  const output = pair.result ? richOutputText(pair.result) : "";
  const detail = output ? (
    <pre className="max-h-[min(18rem,34vh)] overflow-y-auto overscroll-contain whitespace-pre-wrap break-words font-mono text-xs leading-6 text-ink">
      {typeof output === "string" ? output : JSON.stringify(output, null, 2)}
    </pre>
  ) : (
    <div className="text-xs text-ink-tertiary">{t("tool.noResult")}</div>
  );

  return (
    <ToolDisclosure
      toggle={
        <TrackSummary
          verb={humanToolName(pair.name, t)}
          object={reason}
          failed
        />
      }
      failed
      open={open}
      onToggle={onToggle}
      detailClassName="mb-1 mt-0.5 rounded-[var(--radius-md)] bg-surface px-3 py-2"
    >
      {detail}
    </ToolDisclosure>
  );
}

function toolFailureSummary(pair: ToolPair): string {
  const fromMeta =
    qpString(pair.meta, "error") ||
    qpString(pair.meta, "message") ||
    qpString(pair.meta, "detail");
  if (fromMeta) return firstLine(fromMeta);
  const text = richOutputText(pair.result);
  if (typeof text === "string" && text.trim()) return firstLine(text);
  return "";
}

function firstLine(value: string, max = 80): string {
  const line = value.trim().split(/\r?\n/, 1)[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/**
 * 运行中的行尾指示:13px Spinner,不带文字。完成/失败态零落墨——
 * 成功是预期不配图标,失败由整行 danger 色承担(对号/叉号已整体移除)。
 */
export function ToolStatus({
  running,
}: {
  running: boolean;
  failed?: boolean;
  quiet?: boolean;
}) {
  if (!running) return null;
  return <Spinner size={13} className="shrink-0 text-ink-tertiary" />;
}

export function toolData(message: StreamMessage | null) {
  const block = message?.content.find(
    (part): part is DataContent => part.type === "data",
  );
  return (block?.data ?? {}) as Record<string, unknown>;
}

export function buildToolPair(
  call: StreamMessage | null,
  output: StreamMessage | null,
): ToolPair {
  const callData = toolData(call);
  const outputData = toolData(output);
  const callUi = callData.ui as Record<string, unknown> | undefined;
  return {
    call,
    output,
    callId: stringValue(callData.call_id) || stringValue(outputData.call_id),
    name:
      stringValue(callData.name) ||
      stringValue(outputData.name) ||
      t("tool.genericName"),
    arguments: stringValue(callData.arguments),
    result: stringValue(outputData.output),
    state: stringValue(outputData.state) || null,
    meta: parseQpMeta(outputData.meta),
    uiIcon:
      callUi && typeof callUi === "object" && !Array.isArray(callUi)
        ? stringValue(callUi.icon)
        : "",
  };
}

function normalizedToolState(state: string | null): string {
  return state?.trim().toLocaleLowerCase() ?? "";
}

/** 旧历史可能没有 state；有 state 时只认后端的明确成功态。 */
export function isSuccessfulToolState(state: string | null): boolean {
  const normalized = normalizedToolState(state);
  return (
    normalized === "" || normalized === "success" || normalized === "completed"
  );
}

export function isRunningToolState(state: string | null): boolean {
  const normalized = normalizedToolState(state);
  return normalized === "created" || normalized === "in_progress";
}

export function isFailedToolState(state: string | null): boolean {
  const normalized = normalizedToolState(state);
  return Boolean(
    normalized &&
      normalized !== "success" &&
      normalized !== "completed" &&
      normalized !== "created" &&
      normalized !== "in_progress",
  );
}

function argumentSummary(
  value: string,
  translate: (key: TranslationKey) => string,
) {
  if (!value) return "";
  try {
    const parsed = JSON.parse(value) as Record<string, unknown>;
    return Object.entries(parsed)
      .slice(0, 2)
      .map(([key, item]) => `${argumentLabel(key, translate)}: ${String(item)}`)
      .join(" · ");
  } catch {
    return value;
  }
}

export function richOutputText(output: string) {
  try {
    const parsed = JSON.parse(output) as unknown;
    if (!Array.isArray(parsed)) return parsed;
    const text = parsed
      .filter(
        (item): item is { type: "text"; text: string } =>
          Boolean(item) &&
          typeof item === "object" &&
          (item as { type?: unknown }).type === "text" &&
          typeof (item as { text?: unknown }).text === "string",
      )
      .map((item) => item.text)
      .join("\n");
    return text || parsed;
  } catch {
    return output;
  }
}

/** 每个已知工具的运行中/完成时态文案,未知工具回落到通用格式。 */
const TOOL_TENSE_KEYS: Record<
  string,
  { running: TranslationKey; done: TranslationKey }
> = {
  execute_shell_command: {
    running: "tool.tense.shell.running",
    done: "tool.tense.shell.done",
  },
  skill: { running: "tool.tense.skill.running", done: "tool.tense.skill.done" },
  web_search: {
    running: "tool.tense.webSearch.running",
    done: "tool.tense.webSearch.done",
  },
  web_fetch: {
    running: "tool.tense.webFetch.running",
    done: "tool.tense.webFetch.done",
  },
  grep_search: {
    running: "tool.tense.searchFiles.running",
    done: "tool.tense.searchFiles.done",
  },
  glob_search: {
    running: "tool.tense.matchFiles.running",
    done: "tool.tense.matchFiles.done",
  },
  read_file: {
    running: "tool.tense.fileRead.running",
    done: "tool.tense.fileRead.done",
  },
  write_file: {
    running: "tool.tense.fileWrite.running",
    done: "tool.tense.fileWrite.done",
  },
  edit_file: {
    running: "tool.tense.fileEdit.running",
    done: "tool.tense.fileEdit.done",
  },
  append_file: {
    running: "tool.tense.fileAppend.running",
    done: "tool.tense.fileAppend.done",
  },
  send_file_to_user: {
    running: "tool.tense.fileDeliver.running",
    done: "tool.tense.fileDeliver.done",
  },
};

/**
 * 时态化工具标签:运行中「正在读取」、完成后「读取了」。轨道叙事由
 * 它统一供给(卡片头部、摘要行、折叠态近况列表)。失败态请调用方
 * 自行退回 humanToolName 的中性名词。
 */
export function humanToolLabel(
  name: string,
  running: boolean,
  translate: (
    key: TranslationKey,
    params?: Record<string, string | number>,
  ) => string,
): string {
  const normalized = name.replace(/^mcp__/i, "").toLocaleLowerCase();
  const tense = TOOL_TENSE_KEYS[normalized];
  if (tense) return translate(running ? tense.running : tense.done);
  const pretty = humanToolName(name, translate);
  return running ? translate("tool.genericRunning", { name: pretty }) : pretty;
}

export function humanToolName(
  name: string,
  translate: (key: TranslationKey) => string,
) {
  const normalized = name.replace(/^mcp__/i, "").toLocaleLowerCase();
  const labelKey: Record<string, TranslationKey> = {
    skill: "tool.skill",
    web_search: "tool.webSearch",
    web_fetch: "tool.webFetch",
    grep_search: "tool.searchFiles",
    glob_search: "tool.matchFiles",
    execute_shell_command: "tool.shell",
    send_file_to_user: "tool.file.deliver",
  };
  const translated = labelKey[normalized];
  if (translated) return translate(translated);
  return name
    .replace(/^mcp__/, "")
    .replaceAll("_", " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function argumentLabel(
  key: string,
  translate: (key: TranslationKey) => string,
): string {
  const labels: Record<string, TranslationKey> = {
    search_term: "tool.searchTerm",
    query: "tool.searchTerm",
    url: "tool.url",
    command: "tool.command",
    path: "tool.file.path",
    file_path: "tool.file.path",
  };
  return labels[key] ? translate(labels[key]) : key;
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}
