import { Check, ChevronRight, CircleEllipsis, X } from "lucide-react";
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
 * C 端默认只需要知道「正在处理」或「已完成」，逐条成功/失败属于调试
 * 细节。开发构建以及显式打开 debug=tools 时保留状态，方便定位工具链问题。
 */
export function showToolDebugStatus(): boolean {
  if (import.meta.env.DEV) return true;
  if (typeof window === "undefined") return false;
  try {
    const locationText = `${window.location.search}${window.location.hash}`;
    return (
      /(?:[?&])debug=tools(?:&|$)/i.test(locationText) ||
      window.localStorage.getItem("qwenpaw.toolDebug") === "1"
    );
  } catch {
    return false;
  }
}

export function ToolCard({
  pair,
  onOpenFile,
}: {
  pair: ToolPair;
  onOpenFile?: (path: string) => void;
}) {
  const { t } = useTranslation();
  if (pair.name === "execute_shell_command") {
    return <ShellToolCard pair={pair} />;
  }
  if (isFileTool(pair.name)) {
    return <FileToolCard pair={pair} onOpenFile={onOpenFile} />;
  }
  const { running, failed } = toolPairStatus(pair);
  const debugStatus = showToolDebugStatus();
  const summary = argumentSummary(pair.arguments, t);

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

  if (running) {
    return (
      <details className="group my-2 overflow-hidden rounded-[var(--radius-md)] border border-line bg-bubble-tool">
        <summary className="flex min-h-9 cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
          <ChevronRight
            size={14}
            className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
          />
          <span className="min-w-0 truncate font-medium text-ink">
            {humanToolName(pair.name, t)}
          </span>
          {summary && (
            <span className="min-w-0 flex-1 truncate text-ink-muted">
              {summary}
            </span>
          )}
          <ToolStatus running={running} failed={failed} />
        </summary>
        <div className="max-h-[min(20rem,42vh)] overflow-y-auto overscroll-contain border-t border-line px-4 py-3">
          {detail}
        </div>
      </details>
    );
  }

  return (
    <details className="group my-0.5">
      <summary className="flex min-h-7 cursor-pointer list-none items-center gap-1.5 rounded-[var(--radius-sm)] px-1.5 py-1 text-xs transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover">
        <ChevronRight
          size={12}
          className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
        />
        <span
          className={`min-w-0 shrink-0 truncate font-medium ${
            debugStatus && failed ? "text-danger" : "text-ink-tertiary"
          }`}
        >
          {humanToolName(pair.name, t)}
        </span>
        {summary && (
          <span className="min-w-0 flex-1 truncate text-ink-muted">
            {summary}
          </span>
        )}
        {debugStatus && failed ? (
          <X size={13} className="shrink-0 text-danger" />
        ) : debugStatus ? (
          <Check size={13} className="shrink-0 text-ink-muted" />
        ) : null}
      </summary>
      <div className="mb-2 mt-1 rounded-[var(--radius-md)] border border-line bg-bubble-tool px-4 py-3">
        {detail}
      </div>
    </details>
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
  if (running) {
    return (
      <span className="flex shrink-0 items-center gap-1 text-ink-muted">
        <CircleEllipsis size={14} className="animate-pulse" />
        {t("tool.statusRunning")}
      </span>
    );
  }
  if (failed) {
    if (!showToolDebugStatus()) return null;
    return (
      <span className="flex shrink-0 items-center gap-1 text-danger">
        <X size={13} />
        {quiet ? null : t("tool.statusFailed")}
      </span>
    );
  }
  if (quiet) {
    return showToolDebugStatus() ? (
      <Check size={13} className="shrink-0 text-ink-muted" />
    ) : null;
  }
  if (!showToolDebugStatus()) return null;
  return (
    <span className="flex shrink-0 items-center gap-1 text-ok">
      <Check size={13} />
      {t("tool.statusComplete")}
    </span>
  );
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
