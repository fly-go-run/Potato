import { forwardRef, type SelectHTMLAttributes } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "../../lib/cn";
import { inputClasses } from "./Input";

/**
 * 原生 select 的外观化封装：appearance-none 去掉系统控件样式，
 * 叠加自定义 chevron，闭合态与其它输入件完全一致（消除 demo tell），
 * 同时保留原生的键盘/可访问性行为，无需新增依赖。
 */
export type SelectProps = SelectHTMLAttributes<HTMLSelectElement>;

export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, children, ...rest }, ref) => (
    <div className="relative">
      <select
        ref={ref}
        className={cn(
          inputClasses,
          "h-9 cursor-pointer appearance-none pr-9",
          className,
        )}
        {...rest}
      >
        {children}
      </select>
      <ChevronDown
        size={15}
        aria-hidden
        className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-ink-muted"
      />
    </div>
  ),
);
Select.displayName = "Select";
