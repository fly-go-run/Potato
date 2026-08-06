import { Check, Terminal, X } from "lucide-react";
import { Spinner } from "../ui/Spinner";
import { ToolDisclosure } from "./ToolDisclosure";
import type { ToolPair } from "./ToolCard";
import { pairDurationLabel, richOutputText, toolPairStatus } from "./ToolCard";
import { useToolDetail } from "../../stores/uiPrefs";
import { useTranslation } from "../../lib/i18n";

/**
 * 完成态收敛为无填充的"安静行"(正文是版面主角,执行过程退居次要);
 * 运行中保持当前步骤可见，长输出在卡内滚动，失败以 danger 色保持可见。
 */
export function ShellToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const command = shellCommand(pair.arguments);
  const { running, failed } = toolPairStatus(pair);
  const debugStatus = useToolDetail();
  const durationLabel = running ? "" : pairDurationLabel(pair);
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

  const toggle = running ? (
    <>
      <Terminal size={14} className="shrink-0 text-ink-secondary" />
      <code className="min-w-0 flex-1 truncate font-mono text-ink">
        {command || t("tool.shell")}
      </code>
    </>
  ) : (
    <>
      <Terminal
        size={12}
        className={`shrink-0 ${
          debugStatus && failed ? "text-danger" : "text-ink-muted"
        }`}
      />
      <code
        className={`min-w-0 flex-1 truncate font-mono text-[12px] ${
          debugStatus && failed ? "text-danger" : "text-ink-tertiary"
        }`}
      >
        {command || t("tool.shell")}
      </code>
    </>
  );
  const after = running ? (
    <Spinner size={13} className="text-ink-muted" />
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
          ? "px-4 py-3"
          : "mb-2 mt-1 rounded-[var(--radius-md)] border border-line bg-bubble-tool px-4 py-3"
      }
    >
      {detail}
    </ToolDisclosure>
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
