import type { ReactNode } from "react";

/**
 * 统一空态：实线极浅底容器 + 图标置于圆形托盘，取代此前各页的
 * 虚线边框（placeholder 语汇，偏廉价）。
 */
export interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center rounded-[var(--radius-lg)] border border-line bg-bubble-tool/60 px-6 py-16 text-center">
      <div className="grid h-11 w-11 place-items-center rounded-full border border-line bg-surface text-ink-tertiary shadow-[var(--shadow-sm)]">
        {icon}
      </div>
      <p className="mt-4 text-sm font-medium text-ink">{title}</p>
      {description && (
        <p className="mt-1 max-w-sm text-sm text-ink-muted">{description}</p>
      )}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
