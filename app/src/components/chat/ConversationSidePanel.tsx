import {
  ArrowUpRight,
  ChevronRight,
  FileArchive,
  FileCode,
  FileImage,
  Files,
  FileSpreadsheet,
  FileText,
  ListTree,
  PanelRightClose,
  Presentation,
  type LucideIcon,
} from "lucide-react";
import { useMemo, useState } from "react";
import { filePreviewUrl } from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import type { RunStatus } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { isSuccessfulArtifactPair } from "./FileToolCard";
import {
  buildToolPair,
  isSuccessfulToolState,
  toolData,
} from "./ToolCard";

export interface ConversationArtifact {
  id: string;
  path: string;
  name: string;
  sourceMessageId: string;
}

interface ConversationSidePanelProps {
  messages: StreamMessage[];
  artifacts: ConversationArtifact[];
  responseStatus: RunStatus | "idle";
  selectedFilePath?: string;
  onClose: () => void;
  onFileClose?: () => void;
  onOpenFile?: (path: string) => void;
  onLocate: (messageId: string) => void;
}

export function ConversationSidePanel({
  messages,
  artifacts,
  responseStatus,
  selectedFilePath,
  onClose,
  onFileClose,
  onOpenFile,
  onLocate,
}: ConversationSidePanelProps) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"overview" | "artifacts">(
    artifacts.length > 0 ? "artifacts" : "overview",
  );
  const completedSteps = useMemo(
    () =>
      messages.filter(
        (message) =>
          isSuccessfulToolOutput(message),
      ).length,
    [messages],
  );
  const runPresentation = presentRunStatus(responseStatus);

  if (selectedFilePath) {
    return (
      <FilePreviewPanel
        path={selectedFilePath}
        onClose={onClose}
        onBack={onFileClose}
      />
    );
  }

  return (
    <aside className="flex w-[19rem] shrink-0 flex-col border-l border-line bg-bg/70">
      <div className="flex h-11 shrink-0 items-center border-b border-line px-2">
        <div className="flex min-w-0 flex-1 items-center rounded-[var(--radius-sm)] bg-fill-hover px-0.5 py-0.5">
          <PanelTab
            active={tab === "overview"}
            icon={ListTree}
            label={t("chat.panel.overview")}
            onClick={() => setTab("overview")}
          />
          <PanelTab
            active={tab === "artifacts"}
            icon={Files}
            label={t("chat.panel.artifacts", { count: artifacts.length })}
            onClick={() => setTab("artifacts")}
          />
        </div>
        <button
          type="button"
          onClick={onClose}
          title={t("chat.panel.close")}
          aria-label={t("chat.panel.close")}
          className="ml-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted transition-colors hover:bg-fill-hover hover:text-ink"
        >
          <PanelRightClose size={15} />
        </button>
      </div>

      {tab === "overview" ? (
        <div className="space-y-5 overflow-y-auto px-4 py-4">
          <section>
            <h2 className="text-[12px] font-medium text-ink-secondary">
              {t("chat.panel.run")}
            </h2>
            <div className="mt-2 rounded-[var(--radius-md)] bg-surface px-3 py-3">
              <div className="flex items-center gap-2 text-[13px] text-ink">
                <span
                  className={`h-2 w-2 rounded-full ${runPresentation.dotClass}`}
                />
                {t(runPresentation.label)}
              </div>
              <dl className="mt-3 grid grid-cols-2 gap-3 border-t border-line pt-3 text-xs">
                <div>
                  <dt className="text-ink-muted">{t("chat.panel.messages")}</dt>
                  <dd className="mt-1 tabular-nums text-ink-secondary">
                    {messages.length}
                  </dd>
                </div>
                <div>
                  <dt className="text-ink-muted">{t("chat.panel.steps")}</dt>
                  <dd className="mt-1 tabular-nums text-ink-secondary">
                    {completedSteps}
                  </dd>
                </div>
              </dl>
            </div>
          </section>
          <section>
            <div className="flex items-center justify-between">
              <h2 className="text-[12px] font-medium text-ink-secondary">
                {t("chat.panel.artifactSummary")}
              </h2>
              {artifacts.length > 0 && (
                <button
                  type="button"
                  onClick={() => setTab("artifacts")}
                  className="text-xs text-ink-tertiary hover:text-ink"
                >
                  {t("chat.panel.viewAll")}
                </button>
              )}
            </div>
            <div className="mt-2 rounded-[var(--radius-md)] bg-surface px-3 py-3 text-[13px] text-ink-secondary">
              {artifacts.length > 0
                ? t("chat.panel.artifactCount", { count: artifacts.length })
                : t("chat.panel.noArtifacts")}
            </div>
          </section>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {artifacts.length === 0 ? (
            <div className="flex h-full min-h-40 flex-col items-center justify-center px-5 text-center">
              <Files size={22} className="text-ink-muted" />
              <div className="mt-2 text-[13px] text-ink-secondary">
                {t("chat.panel.noArtifacts")}
              </div>
              <div className="mt-1 text-xs leading-5 text-ink-muted">
                {t("chat.panel.noArtifactsHint")}
              </div>
            </div>
          ) : (
            <div className="space-y-1">
              {artifacts.map((artifact) => {
                const Icon = fileIcon(artifact.path);
                return (
                  <div
                    key={artifact.id}
                    className="group rounded-[var(--radius-md)] bg-surface px-2.5 py-2.5"
                  >
                    <div className="flex items-center gap-2.5">
                      <Icon size={17} className="shrink-0 text-ink-secondary" />
                      <button
                        type="button"
                        onClick={() =>
                          onOpenFile
                            ? onOpenFile(artifact.path)
                            : onLocate(artifact.sourceMessageId)
                        }
                        title={artifact.path}
                        className="min-w-0 flex-1 text-left"
                      >
                        <span className="block truncate text-[13px] font-medium text-ink">
                          {artifact.name}
                        </span>
                        <span className="mt-0.5 block truncate text-[11px] text-ink-muted">
                          {directoryOf(artifact.path)}
                        </span>
                      </button>
                      <a
                        href={filePreviewUrl(artifact.path)}
                        target="_blank"
                        rel="noreferrer"
                        title={t("tool.file.open")}
                        aria-label={t("tool.file.open")}
                        className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted hover:bg-fill-hover hover:text-ink"
                      >
                        <ArrowUpRight size={14} />
                      </a>
                    </div>
                    <button
                      type="button"
                      onClick={() => onLocate(artifact.sourceMessageId)}
                      className="mt-2 flex items-center gap-1 text-[11px] text-ink-tertiary hover:text-ink"
                    >
                      {t("chat.panel.locate")}
                      <ChevronRight size={11} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </aside>
  );
}

function FilePreviewPanel({
  path,
  onClose,
  onBack,
}: {
  path: string;
  onClose: () => void;
  onBack?: () => void;
}) {
  const { t } = useTranslation();
  const filename = fileBaseName(path) || path;
  return (
    <aside className="flex w-[min(42rem,48vw)] min-w-[20rem] shrink-0 flex-col border-l border-line bg-bg/70">
      <div className="flex h-11 shrink-0 items-center gap-1 border-b border-line px-2">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            title={t("chat.panel.back")}
            aria-label={t("chat.panel.back")}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted transition-colors hover:bg-fill-hover hover:text-ink"
          >
            <ChevronRight size={15} className="rotate-180" />
          </button>
        )}
        <FileText size={15} className="shrink-0 text-ink-secondary" />
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-ink" title={path}>
          {filename}
        </span>
        <a
          href={filePreviewUrl(path)}
          target="_blank"
          rel="noreferrer"
          title={t("tool.file.open")}
          aria-label={t("tool.file.open")}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted hover:bg-fill-hover hover:text-ink"
        >
          <ArrowUpRight size={14} />
        </a>
        <button
          type="button"
          onClick={onClose}
          title={t("chat.panel.close")}
          aria-label={t("chat.panel.close")}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-muted transition-colors hover:bg-fill-hover hover:text-ink"
        >
          <PanelRightClose size={15} />
        </button>
      </div>
      <div className="min-h-0 flex-1 bg-surface">
        <iframe
          title={filename}
          src={filePreviewUrl(path)}
          className="h-full min-h-[24rem] w-full border-0 bg-surface"
        />
      </div>
    </aside>
  );
}

function PanelTab({
  active,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[6px] px-2 py-1 text-xs transition-colors ${
        active
          ? "bg-surface text-ink shadow-[var(--shadow-sm)]"
          : "text-ink-tertiary hover:text-ink"
      }`}
    >
      <Icon size={13} />
      <span className="truncate">{label}</span>
    </button>
  );
}

export function collectConversationArtifacts(
  messages: StreamMessage[],
): ConversationArtifact[] {
  const outputsByCallId = new Map<string, StreamMessage>();
  for (const message of messages) {
    if (!isToolOutput(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    if (callId) outputsByCallId.set(callId, message);
  }

  const byPath = new Map<string, ConversationArtifact>();
  for (const message of messages) {
    if (!isToolCall(message.type)) continue;
    const callId = stringValue(toolData(message).call_id);
    const pair = buildToolPair(
      message,
      callId ? outputsByCallId.get(callId) ?? null : null,
    );
    if (!isSuccessfulArtifactPair(pair)) continue;
    const path = filePathFromArguments(pair.arguments);
    if (!path) continue;
    byPath.set(path, {
      id: `${message.id}:${path}`,
      path,
      name: fileBaseName(path) || path,
      sourceMessageId: message.id,
    });
  }
  return Array.from(byPath.values()).reverse();
}

function isSuccessfulToolOutput(message: StreamMessage): boolean {
  if (!isToolOutput(message.type) || message.status !== "completed") {
    return false;
  }
  const state = stringValue(toolData(message).state).toLocaleLowerCase();
  return isSuccessfulToolState(state);
}

export function presentRunStatus(status: RunStatus | "idle"): {
  label:
    | "chat.panel.running"
    | "chat.panel.completed"
    | "chat.panel.failed"
    | "chat.panel.cancelled";
  dotClass: string;
} {
  if (status === "created" || status === "in_progress") {
    return { label: "chat.panel.running", dotClass: "animate-pulse bg-ok" };
  }
  if (status === "failed") {
    return { label: "chat.panel.failed", dotClass: "bg-danger" };
  }
  if (status === "cancelled") {
    return { label: "chat.panel.cancelled", dotClass: "bg-warn" };
  }
  return { label: "chat.panel.completed", dotClass: "bg-ink-muted" };
}

function filePathFromArguments(argumentsValue: string): string {
  try {
    const parsed = JSON.parse(argumentsValue) as Record<string, unknown>;
    return typeof parsed.file_path === "string" ? parsed.file_path : "";
  } catch {
    return "";
  }
}

function isToolCall(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call" ||
    type === "function_call" ||
    type === "mcp_tool_call"
  );
}

function isToolOutput(type: StreamMessage["type"]): boolean {
  return (
    type === "plugin_call_output" ||
    type === "function_call_output" ||
    type === "mcp_tool_call_output"
  );
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

const ICON_BY_EXTENSION: Array<[readonly string[], LucideIcon]> = [
  [["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"], FileImage],
  [["xlsx", "xls", "csv", "tsv", "numbers"], FileSpreadsheet],
  [["ppt", "pptx", "key"], Presentation],
  [["zip", "tar", "gz", "tgz", "rar", "7z"], FileArchive],
  [["ts", "tsx", "js", "jsx", "py", "go", "rs", "json", "yaml", "yml", "sh", "html", "css", "sql"], FileCode],
];

function fileIcon(path: string): LucideIcon {
  const extension = fileBaseName(path).split(".").at(-1)?.toLowerCase() ?? "";
  for (const [extensions, icon] of ICON_BY_EXTENSION) {
    if (extensions.includes(extension)) return icon;
  }
  return FileText;
}

function fileBaseName(path: string): string {
  return path.split(/[/\\\\]/).at(-1) ?? "";
}

function directoryOf(path: string): string {
  const directory = path.slice(0, path.length - fileBaseName(path).length);
  return directory.replace(/[/\\\\]$/, "") || path;
}
