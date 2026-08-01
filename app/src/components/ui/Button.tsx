import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export type ButtonVariant =
  | "primary"
  | "secondary"
  | "ghost"
  | "accent"
  | "danger";
export type ButtonSize = "sm" | "md";
/**
 * 形状语言分工：功能按钮（提交/新建/发现/删除…）是矩形（8px），
 * 只有「可切换的选择器」——模式切换、筛选 chip、能力 chip——才用胶囊。
 * 全部药丸化会让 8/10/14/18px 的圆角标尺失去角色。
 */
export type ButtonShape = "rect" | "pill";

const base =
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap font-medium " +
  "transition-[background-color,color,box-shadow,border-color,opacity] duration-[var(--dur-fast)] " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-surface disabled:pointer-events-none disabled:opacity-40";

const variants: Record<ButtonVariant, string> = {
  // 主操作：中性近黑，把 accent 从「主操作」中解放出来（Codex 风格）
  primary:
    "bg-btn-primary text-btn-primary-ink shadow-[var(--shadow-control)] hover:bg-btn-primary-hover active:opacity-90",
  secondary:
    "border border-transparent bg-surface text-ink-secondary shadow-[var(--shadow-control)] hover:bg-fill-hover hover:text-ink active:bg-fill-active",
  ghost: "text-ink-secondary hover:bg-fill-hover hover:text-ink active:bg-fill-active",
  // accent：兼容少量旧入口，但仍遵循当前的中性强调色。
  accent: "bg-accent text-btn-primary-ink shadow-[var(--shadow-control)] hover:bg-accent-hover active:opacity-90",
  danger: "text-danger hover:bg-danger-soft",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-8 px-3.5 text-[13px]",
  md: "h-9 px-4 text-sm",
};

const shapes: Record<ButtonShape, string> = {
  rect: "rounded-[var(--radius-sm)]",
  pill: "rounded-full",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  shape?: ButtonShape;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "secondary",
      size = "md",
      shape = "rect",
      className,
      type,
      ...rest
    },
    ref,
  ) => (
    <button
      ref={ref}
      type={type ?? "button"}
      className={cn(
        base,
        variants[variant],
        sizes[size],
        shapes[shape],
        className,
      )}
      {...rest}
    />
  ),
);
Button.displayName = "Button";
