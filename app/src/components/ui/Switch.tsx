import { cn } from "../../lib/cn";

/**
 * 统一开关。此前 Settings/Skills/Crons 各手写一遍（含 translate-x 魔法值）；
 * 收敛为单一原语，focus/disabled 一次做进去。
 */
export interface SwitchProps {
  checked: boolean;
  onChange: () => void;
  disabled?: boolean;
  title?: string;
  "aria-label"?: string;
  className?: string;
}

export function Switch({
  checked,
  onChange,
  disabled,
  title,
  className,
  ...aria
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={aria["aria-label"]}
      title={title}
      disabled={disabled}
      onClick={onChange}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors duration-[var(--dur-fast)]",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-surface",
        "disabled:opacity-40",
        checked ? "bg-accent" : "bg-line-strong",
        className,
      )}
    >
      <span
        className={cn(
          "absolute left-0 top-0.5 h-4 w-4 rounded-full bg-white shadow-[var(--shadow-sm)]",
          "transition-transform duration-[var(--dur-fast)]",
          checked ? "translate-x-[1.125rem]" : "translate-x-0.5",
        )}
      />
    </button>
  );
}
