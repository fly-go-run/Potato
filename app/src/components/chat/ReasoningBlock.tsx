import { Brain, ChevronRight } from "lucide-react";
import { lazy, Suspense } from "react";
import type { StreamMessage } from "../../lib/stream";
import { textFromContent } from "../../lib/content";
import { useTranslation } from "../../lib/i18n";

const Markdown = lazy(() =>
  import("./Markdown").then((module) => ({ default: module.Markdown })),
);

export function ReasoningBlock({
  message,
  compact = false,
}: {
  message: StreamMessage;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const text = textFromContent(message.content);
  const streaming = message.status === "in_progress";

  return (
    <details
      className={`group text-ink-secondary ${compact ? "my-1" : "my-2"}`}
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 py-1 text-xs font-medium">
        <ChevronRight
          size={compact ? 12 : 14}
          className="transition-transform group-open:rotate-90"
        />
        {!compact && <Brain size={14} />}
        <span>
          {compact
            ? t("reasoning.depth")
            : streaming
            ? t("reasoning.thinking")
            : t("reasoning.process")}
        </span>
        {streaming && (
          <span className="flex gap-1" aria-label={t("reasoning.ariaThinking")}>
            <span className="h-1 w-1 animate-pulse rounded-full bg-ink-tertiary" />
            <span className="h-1 w-1 animate-pulse rounded-full bg-ink-tertiary [animation-delay:150ms]" />
            <span className="h-1 w-1 animate-pulse rounded-full bg-ink-tertiary [animation-delay:300ms]" />
          </span>
        )}
      </summary>
      <div
        className={`${
          compact ? "ml-4" : "ml-5 border-l border-line pl-4"
        } pt-1 text-ink-secondary`}
      >
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
          <span className="text-xs text-ink-muted">
            {t("reasoning.waiting")}
          </span>
        )}
      </div>
    </details>
  );
}
