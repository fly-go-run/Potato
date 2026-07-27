import { Check, ChevronRight, CircleEllipsis, Terminal, X } from "lucide-react";
import type { ToolPair } from "./ToolCard";
import { richOutputText } from "./ToolCard";
import { useTranslation } from "../../lib/i18n";

export function ShellToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const command = shellCommand(pair.arguments);
  const running = !pair.output && pair.call?.status === "in_progress";
  const failed = pair.state === "failed" || pair.state === "error";
  const output = richOutputText(pair.result);

  return (
    <details className="group my-2 overflow-hidden rounded-md bg-bubble-tool">
      <summary className="flex min-h-9 cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
        <ChevronRight
          size={14}
          className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
        />
        <Terminal size={14} className="shrink-0 text-ink-secondary" />
        <code className="min-w-0 flex-1 truncate font-mono text-ink">
          {command || t("tool.shell")}
        </code>
        {running ? (
          <CircleEllipsis size={14} className="animate-pulse text-ink-muted" />
        ) : failed ? (
          <X size={14} className="text-danger" />
        ) : (
          <Check size={14} className="text-ok" />
        )}
      </summary>
      <div className="border-t border-line bg-bg px-4 py-3 font-mono text-xs leading-6">
        <div className="mb-2 flex gap-2 text-ink-secondary">
          <span className="select-none text-ink-muted">$</span>
          <span className="whitespace-pre-wrap break-all">{command}</span>
        </div>
        {output ? (
          <pre className="whitespace-pre-wrap break-words text-ink">
            {typeof output === "string"
              ? output
              : JSON.stringify(output, null, 2)}
          </pre>
        ) : (
          <span className="text-ink-muted">
            {running ? t("tool.waitingOutput") : t("tool.noOutput")}
          </span>
        )}
      </div>
    </details>
  );
}

function shellCommand(argumentsJson: string) {
  try {
    const parsed = JSON.parse(argumentsJson) as { command?: unknown };
    return typeof parsed.command === "string" ? parsed.command : argumentsJson;
  } catch {
    return argumentsJson;
  }
}
