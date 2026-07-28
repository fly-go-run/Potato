import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

/**
 * 统一卡片/列表容器：surface 底 + line 描边 + 圆角。深色下叠一条
 * 顶部高光内边框（line-highlight）营造抬升感。用于列表容器与分区卡。
 */
export type CardProps = HTMLAttributes<HTMLDivElement>;

export function Card({ className, ...rest }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-[var(--radius-lg)] border border-line bg-surface",
        "shadow-[inset_0_1px_0_0_var(--line-highlight)]",
        className,
      )}
      {...rest}
    />
  );
}
