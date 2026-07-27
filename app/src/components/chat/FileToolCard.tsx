import { ChevronRight, FileText } from "lucide-react";
import { lineDiff, type DiffLineKind } from "../../lib/lineDiff";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import type { ToolPair } from "./ToolCard";
import { richOutputText, ToolStatus } from "./ToolCard";

const FILE_TOOL_TITLES: Record<string, TranslationKey> = {
  read_file: "tool.file.read",
  write_file: "tool.file.write",
  edit_file: "tool.file.edit",
  append_file: "tool.file.append",
};

export function isFileTool(name: string): boolean {
  return Object.hasOwn(FILE_TOOL_TITLES, name);
}

export function FileToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const parameters = parseArguments(pair.arguments);
  const path =
    typeof parameters.file_path === "string" ? parameters.file_path : "";
  const running = !pair.output && pair.call?.status === "in_progress";
  const failed = pair.state === "failed" || pair.state === "error";
  const titleKey = FILE_TOOL_TITLES[pair.name] ?? "tool.genericName";

  return (
    <details className="group my-2 overflow-hidden rounded-md bg-bubble-tool">
      <summary className="flex min-h-9 cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
        <ChevronRight
          size={14}
          className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
        />
        <FileText size={14} className="shrink-0 text-ink-secondary" />
        <span className="shrink-0 font-medium text-ink">{t(titleKey)}</span>
        <span className="min-w-0 flex-1 truncate font-mono text-ink-muted">
          {path || t("tool.file.path")}
        </span>
        <ToolStatus running={running} failed={failed} />
      </summary>
      <div className="border-t border-line bg-surface">
        <div className="flex gap-3 border-b border-line px-4 py-2 text-xs">
          <span className="shrink-0 text-ink-muted">
            {t("tool.file.path")}
          </span>
          <code className="min-w-0 break-all text-ink-secondary">
            {path || t("tool.noResult")}
          </code>
        </div>
        <div className="px-4 py-3">
          <div className="mb-2 text-xs font-medium text-ink-muted">
            {pair.name === "edit_file"
              ? t("tool.file.changes")
              : t("tool.file.content")}
          </div>
          <FileToolContent pair={pair} parameters={parameters} />
        </div>
      </div>
    </details>
  );
}

function FileToolContent({
  pair,
  parameters,
}: {
  pair: ToolPair;
  parameters: Record<string, unknown>;
}) {
  const { t } = useTranslation();
  if (pair.name === "edit_file") {
    const before =
      typeof parameters.old_text === "string" ? parameters.old_text : "";
    const after =
      typeof parameters.new_text === "string" ? parameters.new_text : "";
    if (before || after) return <LineDiff before={before} after={after} />;
  }

  const argumentContent =
    typeof parameters.content === "string" ? parameters.content : "";
  const rawContent =
    pair.name === "read_file"
      ? richOutputText(pair.result)
      : argumentContent || pair.arguments;
  const content =
    typeof rawContent === "string"
      ? rawContent
      : rawContent
        ? JSON.stringify(rawContent, null, 2)
        : "";

  if (!content) {
    return (
      <div className="text-xs text-ink-muted">
        {!pair.output && pair.call?.status === "in_progress"
          ? t("tool.running")
          : t("tool.noResult")}
      </div>
    );
  }
  return (
    <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-md bg-bg px-3 py-2 font-mono text-xs leading-5 text-ink">
      {content}
    </pre>
  );
}

function LineDiff({ before, after }: { before: string; after: string }) {
  const lines = lineDiff(before, after);
  return (
    <div className="max-h-80 overflow-auto rounded-md border border-line font-mono text-xs leading-5">
      {lines.map((line, index) => (
        <div
          key={`${index}-${line.kind}`}
          className={`flex min-w-max px-2 ${diffLineClass(line.kind)}`}
        >
          <span className="w-5 shrink-0 select-none text-center">
            {line.kind === "remove" ? "-" : line.kind === "add" ? "+" : " "}
          </span>
          <span className="whitespace-pre pr-3">{line.text || " "}</span>
        </div>
      ))}
    </div>
  );
}

function diffLineClass(kind: DiffLineKind): string {
  if (kind === "remove") return "bg-danger-soft text-danger";
  if (kind === "add") return "bg-ok/10 text-ok";
  return "bg-line/30 text-ink-muted";
}

function parseArguments(value: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}
