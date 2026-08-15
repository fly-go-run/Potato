import { cn } from "../../lib/cn";
import type { KeyboardEvent } from "react";

/**
 * 统一段控/标签切换：中性选中态（非 accent 蓝底），克制。
 * 取代此前 药丸 tab / 下划线 tab / accent-soft 选中 三套并存。
 */
export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  count?: number;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  className?: string;
  /**
   * tabs(默认):裸 chip,选中灰药丸——用于页签语义(如技能/插件切换)。
   * track:凹槽轨道 + 选中浮起——用于设置类单选,未选中项也有明确控件边界。
   */
  variant?: "tabs" | "track";
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  className,
  variant = "tabs",
}: SegmentedControlProps<T>) {
  const selectedIndex = options.findIndex((option) => option.value === value);
  const handleKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (index + 1) % options.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (index - 1 + options.length) % options.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = options.length - 1;
    }
    if (nextIndex === null || options.length === 0) return;
    event.preventDefault();
    const nextOption = options[nextIndex];
    if (!nextOption) return;
    onChange(nextOption.value);
    event.currentTarget.parentElement
      ?.querySelectorAll<HTMLButtonElement>('[role="radio"]')
      .item(nextIndex)
      .focus();
  };

  const track = variant === "track";
  return (
    <div
      role="radiogroup"
      className={cn(
        "inline-flex items-center",
        track ? "gap-0.5 rounded-[10px] bg-fill-hover p-0.5" : "gap-1",
        className,
      )}
    >
      {options.map((option, index) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected || (selectedIndex < 0 && index === 0) ? 0 : -1}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => handleKeyDown(event, index)}
            className={cn(
              "inline-flex items-center gap-1.5 text-[13px] font-medium",
              "transition-[background-color,color,box-shadow] duration-[var(--dur-fast)]",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              track
                ? cn(
                    "h-7 rounded-lg border px-3",
                    selected
                      ? "border-line bg-surface text-ink shadow-[var(--shadow-sm)] dark:border-line-highlight dark:shadow-none"
                      : "border-transparent text-ink-secondary hover:text-ink",
                  )
                : cn(
                    "h-8 rounded-full px-3.5",
                    selected
                      ? "bg-btn-primary text-btn-primary-ink shadow-[var(--shadow-control)]"
                      : "text-ink-secondary hover:bg-fill-hover hover:text-ink",
                  ),
            )}
          >
            {option.label}
            {option.count !== undefined && (
              <span className="text-xs tabular-nums text-ink-tertiary">
                {option.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
