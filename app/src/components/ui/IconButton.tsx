import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export type IconButtonTone = "default" | "danger";
export type IconButtonSize = "sm" | "md";

const base =
  "inline-flex shrink-0 items-center justify-center rounded-[8px] text-ink-muted " +
  "transition-[background-color,color,opacity] duration-[var(--dur-fast)] " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-surface active:bg-fill-active disabled:pointer-events-none disabled:opacity-40";

const tones: Record<IconButtonTone, string> = {
  default: "hover:bg-fill-hover hover:text-ink",
  danger: "hover:bg-danger-soft hover:text-danger",
};

// 尺寸满足 ≥32px 可点区域（sm 用于密集行内，md 为常规）
const sizes: Record<IconButtonSize, string> = {
  sm: "h-[30px] w-[30px]",
  md: "h-8 w-8",
};

export interface IconButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: IconButtonTone;
  size?: IconButtonSize;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ tone = "default", size = "md", className, type, ...rest }, ref) => (
    <button
      ref={ref}
      type={type ?? "button"}
      className={cn(base, tones[tone], sizes[size], className)}
      {...rest}
    />
  ),
);
IconButton.displayName = "IconButton";
