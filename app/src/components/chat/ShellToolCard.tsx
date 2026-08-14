import { Check, Terminal, X } from "lucide-react";
import { Spinner } from "../ui/Spinner";
import { ToolDisclosure } from "./ToolDisclosure";
import type { ToolPair } from "./ToolCard";
import { pairDurationLabel, richOutputText, toolPairStatus } from "./ToolCard";
import { useToolDetail } from "../../stores/uiPrefs";
import { useTranslation } from "../../lib/i18n";
import { qpBool, qpInt } from "../../lib/toolMeta";

/**
 * 执行轨道里恒定是无填充的"安静行"(正文是版面主角,执行过程退居次要):
 * 运行中与完成态共用同一套行几何,只有图标/文字颜色和行尾槽随状态变化,
 * 收尾时整行一个像素都不动。长输出在展开的详情面板内滚动,失败以 danger
 * 色保持可见。
 */
export function ShellToolCard({ pair }: { pair: ToolPair }) {
  const { t } = useTranslation();
  const command = shellCommand(pair.arguments);
  const { running, failed } = toolPairStatus(pair);
  const debugStatus = useToolDetail();
  const durationLabel = running ? "" : pairDurationLabel(pair);
  const output = richOutputText(pair.result);
  // qp meta(有则展示,历史会话无 meta 时整段静默):exit code 用终端
  // 母语 "exit N" 不翻译,与 $ 提示符同一语域;非零才值得刺眼。
  // 有符号读取:-1=超时、负数=信号终止,恰是最需要展示的终态。
  // 沙箱只在"没进沙箱"时提示——默认开沙箱的前提下,缺席才是信号。
  const exitCode = qpInt(pair.meta, "exit_code");
  const unsandboxed = qpBool(pair.meta, "sandboxed") === false;

  const detail = (
    <div className="font-mono text-xs leading-6">
      <div className="mb-2 flex gap-2 text-ink-secondary">
        <span className="select-none text-ink-muted">$</span>
        <span className="whitespace-pre-wrap break-all">{command}</span>
        {(exitCode !== null || unsandboxed) && (
          <span className="ml-auto flex shrink-0 select-none items-center gap-2 pl-3 text-[11px]">
            {unsandboxed && (
              <span className="text-warn">{t("tool.shell.noSandbox")}</span>
            )}
            {exitCode !== null && (
              <span
                className={`tabular-nums ${
                  exitCode === 0 ? "text-ink-muted" : "text-danger"
                }`}
              >
                exit {exitCode}
              </span>
            )}
          </span>
        )}
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

  // 图标与字号在两态完全一致(12px / 继承行的 text-xs),只换颜色。
  const toggle = (
    <>
      <Terminal
        size={12}
        className={`shrink-0 ${
          running
            ? "text-ink-secondary"
            : debugStatus && failed
            ? "text-danger"
            : "text-ink-muted"
        }`}
      />
      <code
        className={`min-w-0 flex-1 truncate font-mono ${
          running
            ? "text-ink"
            : debugStatus && failed
            ? "text-danger"
            : "text-ink-tertiary"
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
      toggle={toggle}
      after={after}
      // 详情面板两态同形;输出本身已在 <pre> 里限高滚动,面板不再另加上限。
      detailClassName="mb-2 mt-1 rounded-[var(--radius-md)] border border-line bg-bubble-tool px-4 py-3"
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
