import { Check, ChevronRight, CircleEllipsis, Terminal, X } from "lucide-react";
import type { ToolPair } from "./ToolCard";
import { richOutputText, showToolDebugStatus, toolPairStatus } from "./ToolCard";
import { useTranslation } from "../../lib/i18n";

/**
 * 完成态收敛为无填充的"安静行"(正文是版面主角,执行过程退居次要);
 * 运行中保持当前步骤可见，长输出在卡内滚动，失败以 danger 色保持可见。
 */
export function ShellToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const command = shellCommand(pair.arguments);
  const { running, failed } = toolPairStatus(pair);
  const debugStatus = showToolDebugStatus();
  const output = richOutputText(pair.result);

  const detail = (
    <div className="font-mono text-xs leading-6">
      <div className="mb-2 flex gap-2 text-ink-secondary">
        <span className="select-none text-ink-muted">$</span>
        <span className="whitespace-pre-wrap break-all">{command}</span>
      </div>
      {output ? (
        <pre className="max-h-[min(18rem,34vh)] overflow-y-auto overscroll-contain whitespace-pre-wrap break-words text-ink">
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
  );

  if (running) {
    return (
      <details className="group my-2 overflow-hidden rounded-[var(--radius-md)] border border-line bg-bubble-tool">
        <summary className="flex min-h-9 cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
          <ChevronRight
            size={14}
            className="shrink-0 text-ink-muted transition-transform group-open:rotate-90"
          />
          <Terminal size={14} className="shrink-0 text-ink-secondary" />
          <code className="min-w-0 flex-1 truncate font-mono text-ink">
            {command || t("tool.shell")}
          </code>
          <CircleEllipsis size={14} className="animate-pulse text-ink-muted" />
        </summary>
        <div className="border-t border-line px-4 py-3">{detail}</div>
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
        <Terminal
          size={12}
          className={`shrink-0 ${debugStatus && failed ? "text-danger" : "text-ink-muted"}`}
        />
        <code
          className={`min-w-0 flex-1 truncate font-mono text-[12px] ${
          debugStatus && failed ? "text-danger" : "text-ink-tertiary"
          }`}
        >
          {command || t("tool.shell")}
        </code>
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

function shellCommand(argumentsJson: string) {
  try {
    const parsed = JSON.parse(argumentsJson) as { command?: unknown };
    return typeof parsed.command === "string" ? parsed.command : argumentsJson;
  } catch {
    return argumentsJson;
  }
}
