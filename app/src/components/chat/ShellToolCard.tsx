import { Terminal } from "lucide-react";
import { Spinner } from "../ui/Spinner";
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
export function ShellToolCard({ pair }: { pair: ToolPair }) {
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

  // 图标与字号在两态完全一致(12px / 继承行的 text-xs),只换颜色。
  const toggle = (
    <>
      <Terminal
        size={12}
        strokeWidth={1.8}
        className={`shrink-0 ${
          failed
            ? "text-danger"
            : running
            ? "text-ink-secondary"
            : "text-icon"
        }`}
      />
      <code
        className={`min-w-0 flex-1 truncate font-mono ${
          failed ? "text-danger" : running ? "text-ink" : "text-ink-secondary"
        }`}
      >
        {command || t("tool.shell")}
      </code>
    </>
  );
  // 行尾槽只在运行中占 13px 的 Spinner。完成态零落墨——成功是预期,
  // 失败由整行 danger 色承担,不靠行尾图标。
  const after = running ? (
    <Spinner size={13} className="text-ink-tertiary" />
  ) : null;

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
