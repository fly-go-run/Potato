import type { ReactNode } from "react";

/**
 * 统一空态：无外框，直接浮在画布上（描边盒会让空态看起来像后台 placeholder）。
 * 唯一的视觉锚点是放大后的图标托盘；图标尺寸在此统一归一化，
 * 调用方传什么 size 都按 26px 渲染，保证四个页面的空态重量一致。
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
    <div className="flex min-h-[15rem] flex-col items-center justify-center px-6 py-12 text-center">
      <div className="grid h-14 w-14 place-items-center rounded-full border border-line bg-surface text-ink-tertiary [&>svg]:h-[26px] [&>svg]:w-[26px]">
        {icon}
      </div>
      <p className="mt-4 text-sm font-medium text-ink">{title}</p>
      {description && (
        <p className="mt-1 max-w-sm text-sm text-ink-tertiary">{description}</p>
      )}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
