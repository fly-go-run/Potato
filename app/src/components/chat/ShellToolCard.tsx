import { useEffect, useRef, useState } from "react";
import { Check, Copy, LoaderCircle, Terminal, TriangleAlert } from "lucide-react";
import { ToolDisclosure } from "./ToolDisclosure";
import type { ToolPair } from "./ToolCard";
import { richOutputText, toolPairStatus } from "./ToolCard";
import { useTranslation } from "../../lib/i18n";
import { shellPresentation } from "../../lib/toolPresentation";
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
  tail: _tail = false,
  open,
  onToggle,
}: {
  pair: ToolPair;
  /** 组内原始层:只出命令+输出纯文本块,不再套一层摘要行。 */
  embedded?: boolean;
  shimmer?: boolean;
  /** Legacy live mode; the collapsed preview now displays the latest output. */
  tail?: boolean;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const { running, failed, completed } = toolPairStatus(pair);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  useEffect(() => {
    if (copyState === "idle") return;
    const timer = window.setTimeout(() => setCopyState("idle"), 1800);
    return () => window.clearTimeout(timer);
  }, [copyState]);
  const output = richOutputText(pair.result);
  const outputRef = useRef<HTMLPreElement>(null);
  const followOutput = useRef(true);
  useEffect(() => {
    const node = outputRef.current;
    if (running && node && followOutput.current) node.scrollTop = node.scrollHeight;
  }, [output, running, open]);
  const presentation = shellPresentation(pair.arguments, output, running);
  const { command } = presentation;
  const footnote = t(presentation.label);
  // qp meta(有则展示,历史会话无 meta 时整段静默)。异常才落墨:
  // 干净退出(exit 0)不渲染任何东西——零是预期,只有非零/信号值得占
  // 一块注意力。有符号读取:-1=超时、负数=信号终止,恰是最需要展示的。
  // 沙箱只在"没进沙箱"时提示——默认开沙箱的前提下,缺席才是信号。
  const exitCode = qpInt(pair.meta, "exit_code");
  const abnormalExit = exitCode !== null && exitCode !== 0;
  const unsandboxed = qpBool(pair.meta, "sandboxed") === false;

  const outputText = typeof output === "string" ? output : output ? JSON.stringify(output, null, 2) : "";
  const copyOutput = async () => {
    try {
      await navigator.clipboard.writeText(`$ ${command}\n\n${outputText}`);
      setCopyState("copied");
    } catch { setCopyState("failed"); }
  };
  const uncertain = presentation.hiddenErrors && !running && !failed;
  const statusLabel = t(running ? "tool.result.running" : failed || abnormalExit ? "tool.result.failed" : uncertain ? "tool.result.uncertain" : completed ? "tool.result.completed" : "tool.result.waiting");
  const panelClass = "my-2 overflow-hidden rounded-[10px] border border-line-strong bg-bubble-tool";
  const detail = (
    <>
      <div className="flex min-h-9 items-center justify-between gap-3 px-3 text-[13px] text-ink-secondary">
        <span className="font-medium">Shell</span>
        <button type="button" onClick={() => void copyOutput()}
          aria-label={t("tool.result.copy")} title={t("tool.result.copy")}
          className="flex h-7 w-7 items-center justify-center rounded-md hover:bg-fill-active focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink">
          {copyState === "copied" ? <Check size={14} /> : <Copy size={14} />}
        </button>
      </div>
      <div className="max-h-24 overflow-auto px-3 pb-3 font-mono text-[13px] leading-6 text-ink-secondary">
        <span className="select-none">$ </span><span className="whitespace-pre-wrap break-words">{command}</span>
      </div>
      {outputText ? (
        <pre ref={outputRef} tabIndex={0} aria-label={t("tool.result.output")}
          onScroll={(event) => {
            const node = event.currentTarget;
            followOutput.current = node.scrollHeight - node.scrollTop - node.clientHeight < 24;
          }}
          className="max-h-[min(20rem,36vh)] min-h-10 overflow-auto overscroll-contain border-t border-line px-3 py-3 font-mono text-[13px] leading-6 text-ink focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink">
          {outputText}
        </pre>
      ) : <div className="px-3 pb-3 text-[13px] text-ink-secondary">{t(running ? "tool.waitingOutput" : "tool.output.empty")}</div>}
      <div className="flex min-h-9 items-center gap-3 border-t border-line px-3 text-[12px] text-ink-secondary">
        <span role="status">{copyState !== "idle" && t(copyState === "copied" ? "tool.result.copied" : "tool.result.copyFailed")}</span>
        {unsandboxed && <span>{t("tool.shell.noSandbox")}</span>}
        <span role="status" className={`ml-auto flex items-center gap-1.5 ${failed || abnormalExit ? "text-danger" : uncertain ? "text-warn" : ""}`}>
          {running ? <LoaderCircle size={13} className="animate-spin motion-reduce:animate-none" /> : failed || abnormalExit || uncertain ? <TriangleAlert size={13} /> : completed ? <Check size={13} /> : null}
          {statusLabel}{abnormalExit ? ` · exit ${exitCode}` : ""}
        </span>
      </div>
    </>
  );

  if (embedded) return <div className={panelClass}>{detail}</div>;

  const toggle = (
    <>
      <Terminal
        size={13}
        strokeWidth={1.8}
        className={`shrink-0 ${failed ? "text-danger" : "text-ink-tertiary"}`}
      />
      <span className={`min-w-0 truncate ${shimmer ? "qp-shimmer" : ""}`}>
        <code
          className={`font-sans text-[14px] font-medium ${
            failed && !shimmer
              ? "text-danger"
              : shimmer
                ? ""
                : "text-ink-secondary group-hover:text-ink"
          }`}
        >
          {footnote || t("tool.shell")}
          {presentation.label === "tool.action.command" && <span className="ml-2 font-mono font-normal">{command.split("\n")[0]?.slice(0, 70)}</span>}
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
      preview={!open && (running || failed || presentation.hiddenErrors) && (
        <div className="ml-5 pb-2 text-[13px] leading-6 text-ink-secondary">
          {presentation.preview ? <pre className="line-clamp-3 max-h-[4.5rem] overflow-hidden whitespace-pre-wrap break-words font-mono text-[13px]">{presentation.preview}</pre> : <span>{t(running ? "tool.waitingOutput" : "tool.output.empty")}</span>}
          {presentation.hiddenErrors && !running && <div className="text-warn">{t("tool.output.hiddenErrors")}</div>}
        </div>
      )}
      detailClassName={panelClass}
    >
      {detail}
    </ToolDisclosure>
  );
}
