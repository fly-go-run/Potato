import { Check, CircleEllipsis, X } from "lucide-react";
import type { DataContent, TextContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { useTranslation } from "../../lib/i18n";

export function ProgressCard({ message }: { message: StreamMessage }) {
  const { t } = useTranslation();
  const title = progressTitle(message) || t("tool.progress");
  const status = progressStatus(message);
  const completed = message.status === "completed";
  const failed = message.status === "failed" || message.status === "cancelled";
  const Icon = failed ? X : completed ? Check : CircleEllipsis;

  if (completed || failed) {
    return (
      <div
        className={`my-2 flex items-center gap-2 text-xs ${
          failed ? "text-danger" : "text-ink-muted"
        }`}
      >
        <Icon size={14} className="shrink-0" />
        <span className="min-w-0 truncate font-medium text-ink-secondary">
          {title}
        </span>
        {status && status !== title && (
          <span className="min-w-0 truncate">· {status}</span>
        )}
      </div>
    );
  }

  return (
    <div className="my-2 rounded-md bg-bubble-tool px-3 py-2 text-xs">
      <div className="flex items-center gap-2">
        <CircleEllipsis
          size={14}
          className="shrink-0 animate-pulse text-accent"
        />
        <span className="min-w-0 truncate font-medium text-ink">{title}</span>
      </div>
      <div className="mt-1 pl-[22px] text-ink-muted">
        {status || t("tool.progressWaiting")}
      </div>
    </div>
  );
}

function progressTitle(message: StreamMessage): string {
  if (message.name) return message.name;
  const metadata = message.metadata ?? {};
  return firstString(metadata.title, metadata.tool_name, metadata.name);
}

function progressStatus(message: StreamMessage): string {
  const text = message.content.find(
    (part): part is TextContent => part.type === "text",
  )?.text;
  if (text) return text;
  const data = message.content.find(
    (part): part is DataContent => part.type === "data",
  )?.data as Record<string, unknown> | undefined;
  return firstString(data?.status, data?.message, data?.detail);
}

function firstString(...values: unknown[]): string {
  return (
    values.find(
      (value): value is string =>
        typeof value === "string" && value.trim().length > 0,
    ) ?? ""
  );
}
