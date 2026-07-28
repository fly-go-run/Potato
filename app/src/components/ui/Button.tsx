import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export type ButtonVariant =
  | "primary"
  | "secondary"
  | "ghost"
  | "accent"
  | "danger";
export type ButtonSize = "sm" | "md";

// 药丸造型（rounded-full）是 ChatGPT/Codex 桌面版按钮的签名语言
const base =
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-full font-medium " +
  "transition-[background-color,color,box-shadow,transform,border-color] duration-[var(--dur-fast)] " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-surface active:scale-[0.98] disabled:pointer-events-none disabled:opacity-40";

const variants: Record<ButtonVariant, string> = {
  // 主操作：中性近黑，把 accent 从「主操作」中解放出来（Codex 风格）
  primary: "bg-btn-primary text-btn-primary-ink hover:bg-btn-primary-hover",
  secondary:
    "border border-line bg-surface text-ink-secondary hover:border-line-strong hover:bg-fill-hover hover:text-ink",
  ghost: "text-ink-secondary hover:bg-fill-hover hover:text-ink",
  // accent：仅用于真正需要强调蓝的少数入口
  accent: "bg-accent text-white hover:bg-accent-hover",
  danger: "text-danger hover:bg-danger-soft",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-8 px-3.5 text-[13px]",
  md: "h-10 px-5 text-sm",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "secondary", size = "md", className, type, ...rest }, ref) => (
    <button
      ref={ref}
      type={type ?? "button"}
      className={cn(base, variants[variant], sizes[size], className)}
      {...rest}
    />
  ),
);
Button.displayName = "Button";
