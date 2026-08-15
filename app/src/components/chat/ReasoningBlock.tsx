import { ChevronRight } from "lucide-react";
import { lazy, Suspense, useState } from "react";
import type { StreamMessage } from "../../lib/stream";
import { textFromContent } from "../../lib/content";
import { useTranslation } from "../../lib/i18n";
import { Collapse } from "./Collapse";

const Markdown = lazy(() =>
  import("./Markdown").then((module) => ({ default: module.Markdown })),
);

/**
 * 执行轨道里的思考条目。「思考中」的活动状态由轨道摘要行统一表达,
 * 这里只保留稳定身份(思考过程)与可展开的正文,开合动效与轨道行
 * 共用同一套 qp-collapse 过渡。
 */
export function ReasoningBlock({
  message,
  open: openProp,
  onToggle,
}: {
  message: StreamMessage;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const [internalOpen, setInternalOpen] = useState(false);
  const open = onToggle ? Boolean(openProp) : internalOpen;
  const text = textFromContent(message.content);
  const toggle = () => {
    if (onToggle) onToggle();
    else setInternalOpen((value) => !value);
  };

  return (
    <div className="my-1 text-ink-secondary">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        className="flex items-center gap-1.5 rounded-[var(--radius-sm)] px-1 py-1 text-xs font-medium text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-ink"
      >
        <ChevronRight
          size={12}
          strokeWidth={1.8}
          className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
            open ? "rotate-90" : ""
          }`}
        />
        <span>{t("reasoning.process")}</span>
      </button>
      <Collapse open={open}>
        <div className="ml-4 border-l border-line pl-3 pt-1 text-ink-secondary">
          {text ? (
            <Suspense
              fallback={
                <div className="whitespace-pre-wrap break-words text-sm leading-6 text-ink-secondary">
                  {text}
                </div>
              }
            >
              <Markdown>{text}</Markdown>
            </Suspense>
          ) : (
            <span className="text-xs text-ink-tertiary">
              {t("reasoning.waiting")}
            </span>
          )}
        </div>
      </Collapse>
    </div>
  );
}
