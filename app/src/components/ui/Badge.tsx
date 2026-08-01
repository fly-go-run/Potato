import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export type BadgeTone = "neutral" | "accent" | "ok" | "warn" | "danger";

const tones: Record<BadgeTone, string> = {
  neutral: "bg-fill-active text-ink-secondary",
  accent: "bg-accent-soft text-accent",
  ok: "bg-[color-mix(in_srgb,var(--ok)_16%,transparent)] text-ok",
  warn: "bg-[color-mix(in_srgb,var(--warn)_16%,transparent)] text-warn",
  danger: "bg-danger-soft text-danger",
};

export interface BadgeProps {
  tone?: BadgeTone;
  children: ReactNode;
  className?: string;
}

export function Badge({ tone = "neutral", children, className }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[11px] font-medium leading-none",
        tones[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

/** 数字徽标（如未读计数），与 Badge 分开以固定圆形尺寸。 */
export function CountBadge({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <span className="inline-flex min-w-[18px] items-center justify-center rounded-full bg-accent px-1.5 py-0.5 text-[10px] font-semibold leading-none text-btn-primary-ink">
      {count > 99 ? "99+" : count}
    </span>
  );
}
