import { Terminal } from "lucide-react";
import { ToolDisclosure } from "./ToolDisclosure";
import type { ToolPair } from "./ToolCard";
import { richOutputText, toolPairStatus } from "./ToolCard";
import { useTranslation } from "../../lib/i18n";
import { qpBool, qpInt } from "../../lib/toolMeta";

/**
 * 执行轨道里恒定是无填充的"安静行"(正文是版面主角,执行过程退居次要):
 * 运行中与完成态共用同一套行几何,只有图标/文字颜色和行尾槽随状态变化,
 * 收尾时整行一个像素都不动。长输出在展开的详情面板内滚动,失败以 danger
 * 色保持可见。
 */
export function ShellToolCard({
  pair,
  embedded = false,
  shimmer = false,
  tail = false,
  open,
  onToggle,
}: {
  pair: ToolPair;
  /** 组内原始层:只出命令+输出纯文本块,不再套一层摘要行。 */
  embedded?: boolean;
  shimmer?: boolean;
  /** Live auto-expand: last 5 lines, no frame. Manual expand is full. */
  tail?: boolean;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const command = shellCommand(pair.arguments);
  const { running, failed } = toolPairStatus(pair);
  const output = richOutputText(pair.result);
  // qp meta(有则展示,历史会话无 meta 时整段静默)。异常才落墨:
  // 干净退出(exit 0)不渲染任何东西——零是预期,只有非零/信号值得占
  // 一块注意力。有符号读取:-1=超时、负数=信号终止,恰是最需要展示的。
  // 沙箱只在"没进沙箱"时提示——默认开沙箱的前提下,缺席才是信号。
  const exitCode = qpInt(pair.meta, "exit_code");
  const abnormalExit = exitCode !== null && exitCode !== 0;
  const unsandboxed = qpBool(pair.meta, "sandboxed") === false;

  const detail = (
    <div className="font-mono text-xs leading-6">
      <div className="mb-2 flex gap-2 text-ink-secondary">
        <span className="select-none text-ink-muted">$</span>
        <span className="whitespace-pre-wrap break-all">{command}</span>
        {(abnormalExit || unsandboxed) && (
          <span className="ml-auto flex shrink-0 select-none items-center gap-2 pl-3 text-[11px]">
            {unsandboxed && (
              <span className="text-warn">{t("tool.shell.noSandbox")}</span>
            )}
            {abnormalExit && (
              <span className="tabular-nums text-danger">exit {exitCode}</span>
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
        <span className="text-ink-tertiary">
          {running ? t("tool.waitingOutput") : t("tool.noOutput")}
        </span>
      )}
    </div>
  );

  if (tail) {
    const preview = lastOutputLines(output, 5);
    return (
      <div>
        <button
          type="button"
          onClick={onToggle}
          className="group flex w-full items-center gap-1.5 py-1 text-left text-[13px] text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:text-ink"
        >
          <Terminal
            size={14}
            strokeWidth={1.8}
            className="shrink-0 text-ink-muted"
          />
          <span className={`min-w-0 truncate ${shimmer ? "qp-shimmer" : ""}`}>
            <code className="font-mono text-[12px]">
              {command || t("tool.shell")}
            </code>
          </span>
        </button>
        {preview ? (
          <pre className="font-mono text-xs leading-6 whitespace-pre-wrap break-words text-ink-secondary">
            {preview}
          </pre>
        ) : null}
      </div>
    );
  }

  if (embedded) {
    return (
      <div className="mb-1 mt-0.5 rounded-[var(--radius-md)] bg-surface px-3 py-2">
        {detail}
      </div>
    );
  }

  const toggle = (
    <>
      <Terminal
        size={14}
        strokeWidth={1.8}
        className={`shrink-0 ${failed ? "text-danger" : "text-ink-muted"}`}
      />
      <span className={`min-w-0 truncate ${shimmer ? "qp-shimmer" : ""}`}>
        <code
          className={`font-mono text-[12px] ${
            failed && !shimmer
              ? "text-danger"
              : shimmer
                ? ""
                : "text-ink-tertiary group-hover:text-ink"
          }`}
        >
          {command || t("tool.shell")}
        </code>
      </span>
    </>
  );

  return (
    <ToolDisclosure
      toggle={toggle}
      failed={failed}
      open={open}
      onToggle={onToggle}
      detailClassName="mb-1 mt-0.5 rounded-[var(--radius-md)] bg-surface px-3 py-2"
    >
      {detail}
    </ToolDisclosure>
  );
}

function lastOutputLines(output: unknown, count: number): string {
  const text =
    typeof output === "string"
      ? output
      : output == null
        ? ""
        : JSON.stringify(output, null, 2);
  if (!text) return "";
  const lines = text.replace(/\s+$/u, "").split("\n");
  return lines.slice(-count).join("\n");
}

function shellCommand(argumentsJson: string) {
  try {
    const parsed = JSON.parse(argumentsJson) as { command?: unknown };
    return typeof parsed.command === "string" ? parsed.command : argumentsJson;
  } catch {
    return argumentsJson;
  }
}
