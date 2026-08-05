import { Check, X } from "lucide-react";
import { formatDuration, getMessageTiming } from "../../lib/messageTiming";
import { useToolDetail } from "../../stores/uiPrefs";
import { Spinner } from "../ui/Spinner";
import { ToolDisclosure } from "./ToolDisclosure";
import type { DataContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { t, useTranslation, type TranslationKey } from "../../lib/i18n";
import { JsonView } from "./JsonView";
import { ShellToolCard } from "./ShellToolCard";
import { FileToolCard, isFileTool } from "./FileToolCard";

export interface ToolPair {
  call: StreamMessage | null;
  output: StreamMessage | null;
  callId: string | null;
  name: string;
  arguments: string;
  result: string;
  state: string | null;
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
  prominentArtifact = false,
}: {
  pair: ToolPair;
  onOpenFile?: (path: string) => void;
  prominentArtifact?: boolean;
}) {
  if (pair.name === "execute_shell_command") {
    return <ShellToolCard pair={pair} />;
  }
  if (isFileTool(pair.name)) {
    return (
      <FileToolCard
        pair={pair}
        onOpenFile={onOpenFile}
        prominentArtifact={prominentArtifact}
      />
    );
  }
  return <GenericToolCard pair={pair} />;
}

function GenericToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const { running, failed } = toolPairStatus(pair);
  const debugStatus = useToolDetail();
  const summary = argumentSummary(pair.arguments, t);
  const durationLabel = running ? "" : pairDurationLabel(pair);
  // 失败时退回中性名词,「读取了」这类完成时态只留给成功。
  const label = failed
    ? humanToolName(pair.name, t)
    : humanToolLabel(pair.name, running, t);

  const detail = (
    <div className="space-y-3">
      <section>
        <div className="mb-1 text-xs font-medium text-ink-muted">
          {t("tool.parameters")}
        </div>
        <JsonView value={pair.arguments || {}} />
      </section>
      <section>
        <div className="mb-1 text-xs font-medium text-ink-muted">
          {t("tool.result")}
        </div>
        {pair.result ? (
          <JsonView value={richOutputText(pair.result)} />
        ) : (
          <div className="text-xs text-ink-muted">
            {running ? t("tool.running") : t("tool.noResult")}
          </div>
        )}
      </section>
    </div>
  );

  const toggle = (
    <>
      <span
        className={`min-w-0 shrink-0 truncate font-medium ${
          running
            ? "text-ink"
            : debugStatus && failed
            ? "text-danger"
            : "text-ink-tertiary"
        }`}
      >
        {label}
      </span>
      {summary && (
        <span className="min-w-0 flex-1 truncate text-ink-muted">{summary}</span>
      )}
    </>
  );
  const after = running ? (
    <ToolStatus running={running} failed={failed} />
  ) : (
    <>
      {durationLabel && (
        <span className="ml-auto shrink-0 pl-2 text-[11px] tabular-nums text-ink-muted">
          {durationLabel}
        </span>
      )}
      {debugStatus && failed ? (
        <X size={13} className="shrink-0 text-danger" />
      ) : debugStatus ? (
        <Check size={13} className="shrink-0 text-ink-muted" />
      ) : null}
    </>
  );

  return (
    <ToolDisclosure
      card={running}
      toggle={toggle}
      after={after}
      detailClassName={
        running
          ? "max-h-[min(20rem,42vh)] overflow-y-auto overscroll-contain px-4 py-3"
          : "mb-2 mt-1 rounded-[var(--radius-md)] border border-line bg-bubble-tool px-4 py-3"
      }
    >
      {detail}
    </ToolDisclosure>
  );
}

export function ToolStatus({
  running,
  failed,
  quiet,
}: {
  running: boolean;
  failed: boolean;
  quiet?: boolean;
}) {
  const { t } = useTranslation();
  const detail = useToolDetail();
  if (running) {
    return (
      <span className="flex shrink-0 items-center gap-1.5 text-ink-muted">
        <Spinner size={13} />
        {t("tool.statusRunning")}
      </span>
    );
  }
  if (failed) {
    if (!detail) return null;
    return (
      <span className="flex shrink-0 items-center gap-1 text-danger">
        <X size={13} />
        {quiet ? null : t("tool.statusFailed")}
      </span>
    );
  }
  if (quiet) {
    return detail ? (
      <Check size={13} className="shrink-0 text-ink-muted" />
    ) : null;
  }
  if (!detail) return null;
  return (
    <span className="flex shrink-0 items-center gap-1 text-ok">
      <Check size={13} />
      {t("tool.statusComplete")}
    </span>
  );
}

/**
 * 单次工具调用耗时:call 首见 → output 收口(无 output 时退回 call
 * 自身收口)。历史加载的会话没有实时计时,返回空串即隐藏。
 */
export function pairDurationLabel(pair: ToolPair): string {
  const start = pair.call ? getMessageTiming(pair.call.id)?.startedAt : null;
  if (start == null) return "";
  const end =
    (pair.output ? getMessageTiming(pair.output.id)?.endedAt : null) ??
    (pair.call ? getMessageTiming(pair.call.id)?.endedAt : null);
  if (end == null) return "";
  return formatDuration(end - start);
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
