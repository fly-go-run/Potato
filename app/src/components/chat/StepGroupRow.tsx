import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { Collapse } from "./Collapse";

/**
 * 多条同族工具的 fold-row:摘要在 Collapse 外,原始卡进 struct 档。
 * 思考行不用这个组件。
 */
export function StepGroupRow({
  summary,
  open,
  keepMounted,
  onToggle,
  children,
}: {
  summary: ReactNode;
  open: boolean;
  keepMounted: boolean;
  onToggle: () => void;
  children: ReactNode;
}) {
  return (
    <div className="my-0.5">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className="flex min-h-7 w-full items-center gap-1.5 rounded-[var(--radius-sm)] px-1.5 py-1 text-left text-xs text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink"
      >
        <ChevronRight
          size={12}
          strokeWidth={1.8}
          className={`shrink-0 text-ink-tertiary transition-transform duration-[var(--dur-fast)] ${
            open ? "rotate-90" : ""
          }`}
        />
        <span className="min-w-0 truncate">{summary}</span>
      </button>
      <Collapse open={open} keepMounted={keepMounted} struct>
        {children}
      </Collapse>
    </div>
  );
}
