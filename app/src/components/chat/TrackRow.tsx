import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

/**
 * 执行轨道摘要行的共享壳:安静文本行,不是控件。
 * 静息无 chevron、无 hover 灰底;hover 文字加深,行尾淡入 12px chevron。
 */
export function TrackRow({
  open,
  onToggle,
  icon,
  after,
  shimmer,
  failed,
  children,
}: {
  open?: boolean;
  onToggle: () => void;
  icon?: ReactNode;
  after?: ReactNode;
  shimmer?: boolean;
  failed?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={open}
      className={`group flex w-full items-center gap-1.5 py-0.5 text-left text-[12px] transition-colors duration-[var(--dur-fast)] ${
        failed ? "text-danger" : "text-ink-tertiary hover:text-ink-secondary"
      }`}
    >
      {icon}
      <span
        className={`min-w-0 flex-1 truncate ${shimmer ? "qp-shimmer" : ""}`}
      >
        {children}
      </span>
      {after}
      <TrackRowChevron open={open} failed={failed} />
    </button>
  );
}

export function TrackRowChevron({
  open,
  failed,
}: {
  open?: boolean;
  failed?: boolean;
}) {
  return (
    <ChevronRight
      size={12}
      strokeWidth={1.8}
      className={`shrink-0 opacity-0 transition-[opacity,transform] duration-[var(--dur-fast)] group-hover:opacity-100 ${
        failed ? "text-danger" : "text-ink-muted"
      } ${open ? "rotate-90" : ""}`}
    />
  );
}

/** 动词 + 对象同一档灰脚注。无动词时对象单独占行，避免空 span。 */
export function TrackSummary({
  verb,
  object,
  shimmer,
  failed,
}: {
  verb?: ReactNode;
  object?: ReactNode;
  shimmer?: boolean;
  failed?: boolean;
}) {
  const tone = failed && !shimmer ? "text-danger" : undefined;
  const objectClass = `font-mono text-[12px] ${
    shimmer
      ? ""
      : failed
        ? "text-danger"
        : "text-ink-tertiary group-hover:text-ink-secondary"
  }`;
  if (!object) {
    return verb ? <span className={tone}>{verb}</span> : null;
  }
  if (!verb) {
    return <span className={objectClass}>{object}</span>;
  }
  return (
    <>
      <span className={tone}>{verb}</span>
      <span className={`ml-1.5 ${objectClass}`}>{object}</span>
    </>
  );
}
