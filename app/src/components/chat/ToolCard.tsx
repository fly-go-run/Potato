import { Check, ChevronRight, CircleEllipsis, X } from "lucide-react";
import type { DataContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { t, useTranslation } from "../../lib/i18n";
import { JsonView } from "./Markdown";
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

export function ToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  if (pair.name === "execute_shell_command") {
    return <ShellToolCard pair={pair} />;
  }
  if (isFileTool(pair.name)) {
    return <FileToolCard pair={pair} />;
  }
  const running = !pair.output && pair.call?.status === "in_progress";
  const failed = pair.state === "failed" || pair.state === "error";
  const summary = argumentSummary(pair.arguments);

  return (
    <details className="group my-2 overflow-hidden rounded-[var(--radius-md)] border border-line bg-bubble-tool">
      <summary className="flex min-h-9 cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
        <ChevronRight
          size={14}
          className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
        />
        <span className="min-w-0 truncate font-medium text-ink">
          {humanToolName(pair.name)}
        </span>
        {summary && (
          <span className="min-w-0 flex-1 truncate text-ink-muted">
            {summary}
          </span>
        )}
        <ToolStatus running={running} failed={failed} />
      </summary>
      <div className="space-y-3 border-t border-line px-4 py-3">
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
    </details>
  );
}

export function ToolStatus({
  running,
  failed,
}: {
  running: boolean;
  failed: boolean;
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
    return (
      <span className="flex shrink-0 items-center gap-1 text-danger">
        <X size={13} />
        {t("tool.statusFailed")}
      </span>
    );
  }
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

function argumentSummary(value: string) {
  if (!value) return "";
  try {
    const parsed = JSON.parse(value) as Record<string, unknown>;
    return Object.entries(parsed)
      .slice(0, 2)
      .map(([key, item]) => `${key}: ${String(item)}`)
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

function humanToolName(name: string) {
  return name
    .replace(/^mcp__/, "")
    .replaceAll("_", " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}
