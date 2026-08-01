import { FileText, Image as ImageIcon, Zap } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "../../lib/i18n";
import type { TriggerKind } from "../../lib/composerTrigger";

export interface TriggerItem {
  /** 选中后写回输入框的值(技能名 / 文件名) */
  value: string;
  /** 次要说明(技能描述 / 文件来源),可空 */
  description?: string;
  icon?: "file" | "image" | "skill";
  /** 技能自带 emoji 时优先于 icon 展示 */
  emoji?: string;
}

interface TriggerPopoverProps {
  kind: TriggerKind;
  items: TriggerItem[];
  activeIndex: number;
  loading: boolean;
  onSelect: (item: TriggerItem) => void;
  onHover: (index: number) => void;
}

/* 挂靠在 composer 上方的候选浮层(对标 WB 的 `/` 技能列表:
 * 与输入区连成整体,极简两栏行)。键盘交互由 Composer 统一驱动。 */
export function TriggerPopover({
  kind,
  items,
  activeIndex,
  loading,
  onSelect,
  onHover,
}: TriggerPopoverProps) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);

  // 键盘移动选中项时保持其可见
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const active = list.children[activeIndex] as HTMLElement | undefined;
    active?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const title =
    kind === "slash"
      ? t("composer.trigger.skills", { count: items.length })
      : t("composer.trigger.files", { count: items.length });

  return (
    <div className="qp-pop absolute inset-x-0 bottom-full z-30 mb-2 overflow-hidden rounded-[var(--radius-lg)] border border-line bg-raised shadow-[var(--shadow-lg)]">
      <div className="px-3.5 pb-1 pt-2.5 text-[11px] font-medium text-ink-tertiary">
        {loading ? t("composer.trigger.loading") : title}
      </div>
      {!loading && items.length === 0 && (
        <div className="px-3.5 pb-3 pt-1 text-[13px] text-ink-muted">
          {kind === "slash"
            ? t("composer.trigger.noSkills")
            : t("composer.trigger.noFiles")}
        </div>
      )}
      <div ref={listRef} className="max-h-64 overflow-y-auto pb-1.5">
        {items.map((item, index) => {
          const Icon =
            item.icon === "image"
              ? ImageIcon
              : item.icon === "file"
                ? FileText
                : Zap;
          return (
            <button
              key={`${item.value}-${index}`}
              type="button"
              // mousedown 先于 textarea blur,避免焦点抖动
              onMouseDown={(event) => {
                event.preventDefault();
                onSelect(item);
              }}
              onMouseEnter={() => onHover(index)}
              className={`flex w-full items-baseline gap-2 px-3.5 py-1.5 text-left ${
                index === activeIndex ? "bg-fill-hover" : ""
              }`}
            >
              {item.emoji ? (
                <span className="shrink-0 self-center text-[13px] leading-none">
                  {item.emoji}
                </span>
              ) : (
                <Icon
                  size={13}
                  className="relative top-px shrink-0 self-center text-ink-tertiary"
                />
              )}
              <span className="shrink-0 text-[13px] font-medium text-ink">
                {item.value}
              </span>
              {item.description && (
                <span className="min-w-0 truncate text-xs text-ink-tertiary">
                  {item.description}
                </span>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
