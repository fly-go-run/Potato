import { cn } from "../../lib/cn";

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
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  className,
}: SegmentedControlProps<T>) {
  // ChatGPT 式药丸 chip：无外框容器，选中项 = 灰底药丸
  return (
    <div role="tablist" className={cn("inline-flex items-center gap-1", className)}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => onChange(option.value)}
            className={cn(
              "inline-flex h-8 items-center gap-1.5 rounded-full px-3.5 text-[13px] font-medium",
              "transition-[background-color,color] duration-[var(--dur-fast)]",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              selected
                ? "bg-fill-active text-ink"
                : "text-ink-tertiary hover:bg-fill-hover hover:text-ink-secondary",
            )}
          >
            {option.label}
            {option.count !== undefined && (
              <span className="text-xs tabular-nums text-ink-muted">
                {option.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
