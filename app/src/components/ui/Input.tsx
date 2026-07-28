import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

/** 输入类控件的统一外观（供 Input / textarea / 自定义 select 共用）。 */
export const inputClasses =
  "w-full rounded-[var(--radius-md)] border border-line bg-surface px-3 text-sm text-ink " +
  "shadow-[var(--shadow-sm)] transition-[border-color,box-shadow] duration-[var(--dur-fast)] " +
  "placeholder:text-ink-muted focus-visible:outline-none focus-visible:border-accent " +
  "focus-visible:ring-2 focus-visible:ring-ring " +
  "disabled:cursor-not-allowed disabled:bg-bubble-tool disabled:text-ink-muted";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, ...rest }, ref) => (
    <input ref={ref} className={cn(inputClasses, "h-9", className)} {...rest} />
  ),
);
Input.displayName = "Input";
