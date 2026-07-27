import { Brain, ChevronRight } from "lucide-react";
import type { StreamMessage } from "../../lib/stream";
import { useTranslation } from "../../lib/i18n";
import { Markdown, textFromContent } from "./Markdown";

export function ReasoningBlock({ message }: { message: StreamMessage }) {
  const { t } = useTranslation();
  const text = textFromContent(message.content);
  const streaming = message.status === "in_progress";

  return (
    <details className="group my-2 text-ink-secondary">
      <summary className="flex cursor-pointer list-none items-center gap-2 py-1 text-xs font-medium">
        <ChevronRight
          size={14}
          className="transition-transform group-open:rotate-90"
        />
        <Brain size={14} />
        <span>
          {streaming ? t("reasoning.thinking") : t("reasoning.process")}
        </span>
        {streaming && (
          <span
            className="flex gap-1"
            aria-label={t("reasoning.ariaThinking")}
          >
            <span className="h-1 w-1 animate-pulse rounded-full bg-accent" />
            <span className="h-1 w-1 animate-pulse rounded-full bg-accent [animation-delay:150ms]" />
            <span className="h-1 w-1 animate-pulse rounded-full bg-accent [animation-delay:300ms]" />
          </span>
        )}
      </summary>
      <div className="ml-5 border-l border-line pl-4 pt-1 text-ink-secondary">
        {text ? (
          <Markdown>{text}</Markdown>
        ) : (
          <span className="text-xs text-ink-muted">
            {t("reasoning.waiting")}
          </span>
        )}
      </div>
    </details>
  );
}
