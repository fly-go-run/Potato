import { Check, CircleEllipsis, X } from "lucide-react";
import { Spinner } from "../ui/Spinner";
import type { DataContent, TextContent } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { useTranslation } from "../../lib/i18n";
import { useToolDetail } from "../../stores/uiPrefs";

export function ProgressCard({ message }: { message: StreamMessage }) {
  const { t } = useTranslation();
  // hook 必须在压缩卡的提前 return 之前调用。
  const debugStatus = useToolDetail();
  if (isContextCompactionMessage(message)) {
    const phase = String(message.metadata?.phase ?? "in_progress");
    const label =
      phase === "fallback"
        ? t("chat.contextCompaction.fallback")
        : phase === "completed"
        ? t("chat.contextCompaction.completed")
        : t("chat.contextCompaction.running");
    return (
      <div className="my-2 flex items-center gap-2 text-xs text-ink-muted">
        {phase === "in_progress" && <Spinner size={13} />}
        <span className="min-w-0 truncate font-medium text-ink-secondary">
          {label}
        </span>
      </div>
    );
  }

  const rawTitle = progressTitle(message);
  // 后端事件名(英文 slug/工具内部名)不直出;认不出的统一"正在处理"。
  const title = presentTitle(rawTitle) || t("progress.working");
  const status = progressStatus(message);
  const completed = message.status === "completed";
  const failed = message.status === "failed" || message.status === "cancelled";
  const Icon = failed ? X : completed ? Check : CircleEllipsis;

  // 失败终态不分环境,必须始终可见:静默吞掉失败比暴露内部字段更伤。
  if (failed) {
    return (
      <div className="my-2 flex items-center gap-2 text-xs text-danger">
        <Icon size={14} className="shrink-0" />
        <span className="min-w-0 truncate font-medium">
          {t("progress.failedTitle")}
        </span>
        {status && (
          <span className="min-w-0 truncate" title={status}>
            · {status}
          </span>
        )}
      </div>
    );
  }

  if (completed) {
    return (
      <div className="my-2 flex items-center gap-2 text-xs text-ink-muted">
        {debugStatus && <Icon size={14} className="shrink-0" />}
        <span className="min-w-0 truncate font-medium text-ink-secondary">
          {title}
        </span>
        {debugStatus && status && status !== title && (
          <span className="min-w-0 truncate">· {status}</span>
        )}
      </div>
    );
  }

  return (
    <div className="my-2 rounded-md bg-bubble-tool px-3 py-2 text-xs">
      <div className="flex items-center gap-2">
        <Spinner size={13} className="text-ink-tertiary" />
        <span className="min-w-0 truncate font-medium text-ink">{title}</span>
      </div>
      <div className="mt-1 pl-[22px] text-ink-muted">
        {status || t("tool.progressWaiting")}
      </div>
    </div>
  );
}

/**
 * 标题只拦"明显是内部标识符"的文本:含 _ . : 的 slug、全小写单词。
 * 中文、带空格的句子、TitleCase 单词(Planning/Uploading)都放行。
 */
function presentTitle(value: string): string {
  if (!value) return "";
  if (/[_.:]/.test(value) && !value.includes(" ")) return "";
  if (/^[a-z0-9-]+$/.test(value)) return "";
  return value;
}

export function isContextCompactionMessage(message: StreamMessage): boolean {
  return message.metadata?.kind === "context_compaction";
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
