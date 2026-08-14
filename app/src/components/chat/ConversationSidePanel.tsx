import {
  ArrowUpRight,
  ChevronRight,
  FileArchive,
  FileCode,
  FileDiff,
  FileImage,
  Files,
  FolderOpen,
  FileSpreadsheet,
  FileText,
  GitBranch,
  ListTree,
  LoaderCircle,
  Presentation,
  Undo2,
  type LucideIcon,
} from "lucide-react";
import { Fragment, useEffect, useMemo, useState } from "react";
import type { ThemedToken } from "@shikijs/types";
import { fetchFileText, filePreviewUrl, workspaceGitApi } from "../../lib/api";
import {
  canOpenLocalPathWithSystem,
  handleSystemOpenClick,
  openLocalPathWithSystem,
  revealLocalPathInFileManager,
} from "../../lib/desktop";
import {
  presentRunStatus,
  type ConversationArtifact,
} from "../../lib/conversationArtifacts";
import {
  shortenPath,
  type FileChange,
  type FileEdit,
} from "../../lib/fileChanges";
import { useChatStore } from "../../stores/chat";
import { highlightCode, isSupportedLanguage } from "../../lib/highlight";
import { lineDiff, type DiffLine } from "../../lib/lineDiff";
import { Markdown, tokenClass } from "./Markdown";
import {
  matchRepoRelativePath,
  parseUnifiedDiff,
  type UnifiedFileDiff,
} from "../../lib/unifiedDiff";
import { useTranslation } from "../../lib/i18n";
import type { RunStatus } from "../../lib/protocol/types";
import type { StreamMessage } from "../../lib/stream";
import { ChangeStat } from "./ChangeStat";
import { isSuccessfulToolState, toolData } from "./ToolCard";

interface ConversationSidePanelProps {
  messages: StreamMessage[];
  artifacts: ConversationArtifact[];
  changes: FileChange[];
  responseStatus: RunStatus | "idle";
  selectedFilePath?: string;
  selectedChangePath?: string;
  /** 面板的关闭统一走 ChatHeader 的开关;保留字段仅为兼容旧调用。 */
  onClose?: () => void;
  onFileClose?: () => void;
  onOpenFile?: (path: string) => void;
  onOpenChange?: (path: string) => void;
  onLocate: (messageId: string) => void;
}

export function ConversationSidePanel({
  messages,
  artifacts,
  changes,
  responseStatus,
  selectedFilePath,
  selectedChangePath,
  onFileClose,
  onOpenFile,
  onOpenChange,
  onLocate,
}: ConversationSidePanelProps) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"overview" | "changes" | "artifacts">(
    changes.length > 0
      ? "changes"
      : artifacts.length > 0
      ? "artifacts"
      : "overview",
  );
  const completedSteps = useMemo(
    () => messages.filter((message) => isSuccessfulToolOutput(message)).length,
    [messages],
  );
  const runPresentation = presentRunStatus(responseStatus);
  const activeLlm = useChatStore(
    (state) => state.activeModel?.active_llm ?? null,
  );

  if (selectedFilePath) {
    return (
      <FilePreviewPanel
        // 按路径重建,避免快速切换文件时旧内容/旧状态短暂串台
        key={selectedFilePath}
        path={selectedFilePath}
        onBack={onFileClose}
      />
    );
  }

  const selectedChange = selectedChangePath
    ? changes.find((change) => change.path === selectedChangePath)
    : undefined;
  if (selectedChange) {
    return (
      <ChangeDiffPanel
        key={selectedChange.path}
        change={selectedChange}
        onBack={onFileClose}
      />
    );
  }

  return (
    <aside className="flex min-h-0 w-[19rem] shrink-0 flex-col border-l border-line bg-bg/70">
      <div className="flex h-11 shrink-0 items-center border-b border-line px-2">
        <div className="flex min-w-0 flex-1 items-center rounded-[var(--radius-sm)] bg-fill-hover px-0.5 py-0.5">
          <PanelTab
            active={tab === "overview"}
            icon={ListTree}
            label={t("chat.panel.overview")}
            onClick={() => setTab("overview")}
          />
          <PanelTab
            active={tab === "changes"}
            icon={FileDiff}
            label={t("chat.panel.changes", { count: changes.length })}
            onClick={() => setTab("changes")}
          />
          <PanelTab
            active={tab === "artifacts"}
            icon={Files}
            label={t("chat.panel.artifacts", { count: artifacts.length })}
            onClick={() => setTab("artifacts")}
          />
        </div>
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
                  <dt className="text-ink-tertiary">{t("chat.panel.messages")}</dt>
                  <dd className="mt-1 tabular-nums text-ink-secondary">
                    {messages.length}
                  </dd>
                </div>
                <div>
                  <dt className="text-ink-tertiary">{t("chat.panel.steps")}</dt>
                  <dd className="mt-1 tabular-nums text-ink-secondary">
                    {completedSteps}
                  </dd>
                </div>
                {activeLlm && (
                  <div className="col-span-2 min-w-0">
                    <dt className="text-ink-tertiary">{t("chat.panel.model")}</dt>
                    <dd
                      className="mt-1 truncate text-ink-secondary"
                      title={`${activeLlm.provider_id} / ${activeLlm.model}`}
                    >
                      {activeLlm.model}
                    </dd>
                  </div>
                )}
              </dl>
            </div>
          </section>
          <section>
            <div className="flex items-center justify-between">
              <h2 className="text-[12px] font-medium text-ink-secondary">
                {t("chat.panel.changesSummary")}
              </h2>
              {changes.length > 0 && (
                <button
                  type="button"
                  onClick={() => setTab("changes")}
                  className="text-xs text-ink-secondary hover:text-ink"
                >
                  {t("chat.panel.viewAll")}
                </button>
              )}
            </div>
            <div className="mt-2 rounded-[var(--radius-md)] bg-surface px-3 py-3 text-[13px] text-ink-secondary">
              {changes.length > 0
                ? t("chat.panel.changeCount", { count: changes.length })
                : t("chat.panel.noChanges")}
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
                  className="text-xs text-ink-secondary hover:text-ink"
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
      ) : tab === "changes" ? (
        <ChangesList changes={changes} onOpen={onOpenChange} />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {artifacts.length === 0 ? (
            <div className="flex h-full min-h-40 flex-col items-center justify-center px-5 text-center">
              <Files size={22} strokeWidth={1.75} className="text-ink-muted" />
              <div className="mt-2 text-[13px] text-ink-secondary">
                {t("chat.panel.noArtifacts")}
              </div>
              <div className="mt-1 text-xs leading-5 text-ink-tertiary">
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
                      <Icon size={16} strokeWidth={1.75} className="shrink-0 text-ink-secondary" />
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
                        <span className="mt-0.5 block truncate text-[11px] text-ink-tertiary">
                          {directoryOf(artifact.path)}
                        </span>
                      </button>
                      <a
                        href={filePreviewUrl(artifact.path)}
                        target="_blank"
                        rel="noreferrer"
                        title={t("tool.file.open")}
                        aria-label={t("tool.file.open")}
                        onClick={(event) =>
                          handleSystemOpenClick(event, artifact.path)
                        }
                        className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
                      >
                        <ArrowUpRight size={14} strokeWidth={1.8} />
                      </a>
                    </div>
                    <button
                      type="button"
                      onClick={() => onLocate(artifact.sourceMessageId)}
                      className="mt-2 flex items-center gap-1 text-[11px] text-ink-secondary hover:text-ink"
                    >
                      {t("chat.panel.locate")}
                      <ChevronRight size={12} strokeWidth={1.8} />
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
  onBack,
}: {
  path: string;
  onBack?: () => void;
}) {
  const { t } = useTranslation();
  const filename = fileBaseName(path) || path;
  return (
    <aside className="flex min-h-0 w-[min(42rem,48vw)] min-w-[20rem] shrink-0 flex-col border-l border-line bg-bg/70">
      <div className="flex min-h-12 shrink-0 items-center gap-1 border-b border-line px-2 py-1">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            title={t("chat.panel.back")}
            aria-label={t("chat.panel.back")}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-tertiary transition-colors hover:bg-fill-hover hover:text-ink"
          >
            <ChevronRight size={14} strokeWidth={1.8} className="rotate-180" />
          </button>
        )}
        <FileText size={14} strokeWidth={1.8} className="shrink-0 text-ink-secondary" />
        <span className="min-w-0 flex-1 leading-tight" title={path}>
          <span className="block truncate text-[13px] font-medium text-ink">
            {filename}
          </span>
          <span className="mt-0.5 block truncate text-[11px] text-ink-tertiary">
            {directoryOf(path)}
          </span>
        </span>
        {canOpenLocalPathWithSystem(path) ? (
          <>
            <button
              type="button"
              onClick={() => void revealLocalPathInFileManager(path)}
              title={t("chat.preview.reveal")}
              aria-label={t("chat.preview.reveal")}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
            >
              <FolderOpen size={14} strokeWidth={1.8} />
            </button>
            <button
              type="button"
              onClick={() => void openLocalPathWithSystem(path)}
              title={t("chat.preview.openSystem")}
              aria-label={t("chat.preview.openSystem")}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
            >
              <ArrowUpRight size={14} strokeWidth={1.8} />
            </button>
          </>
        ) : (
          <a
            href={filePreviewUrl(path)}
            target="_blank"
            rel="noreferrer"
            title={t("tool.file.open")}
            aria-label={t("tool.file.open")}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
          >
            <ArrowUpRight size={14} strokeWidth={1.8} />
          </a>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-hidden bg-surface">
        <FilePreviewBody path={path} filename={filename} />
      </div>
    </aside>
  );
}

type PreviewSpec =
  | { kind: "image" }
  | { kind: "markdown" }
  | { kind: "code"; language: string }
  | { kind: "text" }
  | { kind: "iframe" };

const IMAGE_EXTENSIONS = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "svg",
  "bmp",
  "ico",
]);
const PLAIN_TEXT_EXTENSIONS = new Set([
  "txt",
  "log",
  "csv",
  "tsv",
  "ini",
  "toml",
  "conf",
  "cfg",
  "env",
  "properties",
  "gitignore",
  "lock",
]);
/** 少数扩展名和 shiki 语言名对不上,手工映射;其余直接用扩展名试。 */
const LANGUAGE_BY_EXTENSION: Record<string, string> = {
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  cc: "cpp",
  hpp: "cpp",
  h: "c",
};

function previewSpec(path: string): PreviewSpec {
  const extension = fileBaseName(path).split(".").at(-1)?.toLowerCase() ?? "";
  if (IMAGE_EXTENSIONS.has(extension)) return { kind: "image" };
  if (extension === "md" || extension === "markdown") {
    return { kind: "markdown" };
  }
  const language = LANGUAGE_BY_EXTENSION[extension] ?? extension;
  if (isSupportedLanguage(language)) return { kind: "code", language };
  if (PLAIN_TEXT_EXTENSIONS.has(extension)) return { kind: "text" };
  return { kind: "iframe" };
}

function FilePreviewBody({
  path,
  filename,
}: {
  path: string;
  filename: string;
}) {
  const spec = previewSpec(path);
  if (spec.kind === "image") {
    return (
      <div className="flex h-full items-start justify-center overflow-auto p-4">
        <img
          src={filePreviewUrl(path)}
          alt={filename}
          className="max-w-full rounded-[var(--radius-sm)]"
        />
      </div>
    );
  }
  if (spec.kind === "iframe") {
    return (
      <iframe
        title={filename}
        src={filePreviewUrl(path)}
        className="h-full min-h-[24rem] w-full border-0 bg-surface"
      />
    );
  }
  return (
    <TextFilePreview
      path={path}
      filename={filename}
      mode={spec.kind}
      language={spec.kind === "code" ? spec.language : undefined}
    />
  );
}

type TextPreviewState =
  | { phase: "loading" }
  | { phase: "ready"; text: string }
  | { phase: "error" };

/** 超过该体积直接退回 iframe 原样输出,避免整文件塞进 DOM。 */
const MAX_TEXT_PREVIEW_CHARS = 2_000_000;

function TextFilePreview({
  path,
  filename,
  mode,
  language,
}: {
  path: string;
  filename: string;
  mode: "markdown" | "code" | "text";
  language?: string;
}) {
  const { t } = useTranslation();
  const [state, setState] = useState<TextPreviewState>({ phase: "loading" });
  useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    setState({ phase: "loading" });
    fetchFileText(path, controller.signal)
      .then((text) => {
        if (!cancelled) setState({ phase: "ready", text });
      })
      .catch(() => {
        if (!cancelled) setState({ phase: "error" });
      });
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [path]);

  if (state.phase === "loading") {
    return (
      <div className="flex items-center gap-2 px-4 py-3 text-xs text-ink-tertiary">
        <LoaderCircle size={14} strokeWidth={1.8} className="animate-spin" />
        {t("chat.preview.loading")}
      </div>
    );
  }
  if (state.phase === "error" || state.text.length > MAX_TEXT_PREVIEW_CHARS) {
    return (
      <iframe
        title={filename}
        src={filePreviewUrl(path)}
        className="h-full min-h-[24rem] w-full border-0 bg-surface"
      />
    );
  }
  if (mode === "markdown") {
    return (
      <div className="h-full overflow-y-auto px-5 py-4 text-[14px] leading-[1.7] text-ink">
        <Markdown transformUrl={(url) => resolveMarkdownAsset(path, url)}>
          {state.text}
        </Markdown>
      </div>
    );
  }
  return <CodeFileView text={state.text} language={language} />;
}

/** 高亮上限:大文件退化为纯文本行,行号照常。 */
const MAX_HIGHLIGHT_CHARS = 300_000;
/** DOM 行数上限,超出截断提示。 */
const MAX_PREVIEW_LINES = 5000;

function CodeFileView({ text, language }: { text: string; language?: string }) {
  const { t } = useTranslation();
  const [tokens, setTokens] = useState<ThemedToken[][] | null>(null);
  useEffect(() => {
    let active = true;
    setTokens(null);
    if (
      !language ||
      text.length > MAX_HIGHLIGHT_CHARS ||
      !isSupportedLanguage(language)
    ) {
      return () => {
        active = false;
      };
    }
    void highlightCode(text, language)
      .then((result) => {
        if (active) setTokens(result);
      })
      .catch(() => {
        /* 高亮失败时保持纯文本 */
      });
    return () => {
      active = false;
    };
  }, [text, language]);

  const lines = useMemo(() => text.split("\n"), [text]);
  const visible = lines.slice(0, MAX_PREVIEW_LINES);
  const truncated = lines.length - visible.length;
  return (
    <div className="h-full overflow-auto">
      <div className="min-w-max py-2 font-mono text-[12px] leading-[1.7]">
        {visible.map((lineText, index) => (
          <div key={index} className="flex pr-4">
            <span className="w-12 shrink-0 select-none pr-3 text-right text-[11px] leading-[1.85] text-ink-muted">
              {index + 1}
            </span>
            <span className="whitespace-pre text-ink">
              {tokens?.[index]
                ? tokens[index]!.map((token, tokenIndex) => (
                    <span key={tokenIndex} className={tokenClass(token)}>
                      {token.content}
                    </span>
                  ))
                : lineText || " "}
            </span>
          </div>
        ))}
      </div>
      {truncated > 0 && (
        <div className="border-t border-line px-4 py-1.5 text-[11px] text-ink-tertiary">
          {t("chat.diff.truncated", { count: truncated })}
        </div>
      )}
    </div>
  );
}

/** 「改动」tab:按文件一行,右侧 ± 行数,点击进入 diff 视图。 */
function ChangesList({
  changes,
  onOpen,
}: {
  changes: FileChange[];
  onOpen?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const projectDir = useChatStore((state) => state.project?.path ?? null);
  if (changes.length === 0) {
    return (
      <div className="flex h-full min-h-40 flex-col items-center justify-center px-5 text-center">
        <FileDiff size={22} strokeWidth={1.75} className="text-ink-muted" />
        <div className="mt-2 text-[13px] text-ink-secondary">
          {t("chat.panel.noChanges")}
        </div>
        <div className="mt-1 text-xs leading-5 text-ink-tertiary">
          {t("chat.panel.noChangesHint")}
        </div>
      </div>
    );
  }
  return (
    <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto p-2">
      {changes.map((change) => {
        const shortDir = directoryOf(shortenPath(change.path, projectDir));
        return (
          <button
            key={change.path}
            type="button"
            onClick={() => onOpen?.(change.path)}
            title={change.path}
            className="flex w-full items-center gap-3 rounded-[var(--radius-sm)] px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[13px] font-medium text-ink">
                {change.name}
              </span>
              {shortDir && (
                <span className="mt-0.5 block truncate text-[11px] text-ink-tertiary">
                  {shortDir}
                </span>
              )}
            </span>
            <ChangeStat
              additions={change.additions}
              deletions={change.deletions}
            />
          </button>
        );
      })}
    </div>
  );
}

/**
 * diff 数据源三态:优先向工作区 git 拿真实行号的 unified diff;
 * 文件不在 git 视野(非 coding 项目/已提交/被 ignore)时回落到
 * 会话事件流里的替换片段。
 */
type GitDiffState =
  | { phase: "loading" }
  | {
      phase: "git";
      branch: string;
      relative: string;
      /** 是否存在未暂存(含未跟踪)改动;撤销按钮只对这部分有意义。 */
      hasUnstaged: boolean;
      /** staged/unstaged 各一段,谁有内容渲染谁,避免只报一半。 */
      files: { staged: boolean; file: UnifiedFileDiff }[];
    }
  | { phase: "fallback" };

function useGitDiff(path: string): {
  state: GitDiffState;
  reload: () => void;
} {
  const [state, setState] = useState<GitDiffState>({ phase: "loading" });
  const [version, setVersion] = useState(0);
  useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    setState({ phase: "loading" });
    (async () => {
      const status = await workspaceGitApi.status(controller.signal);
      const relative = matchRepoRelativePath(
        path,
        status.changes.map((item) => item.path),
      );
      if (!relative) throw new Error("path not in git status");
      const entries = status.changes.filter((item) => item.path === relative);
      const unstagedEntries = entries.filter((item) => !item.staged);
      const stagedEntries = entries.filter((item) => item.staged);
      const files: { staged: boolean; file: UnifiedFileDiff }[] = [];
      if (unstagedEntries.length > 0) {
        const untracked = unstagedEntries.some((item) => item.status === "?");
        const text = (
          await workspaceGitApi.diff(relative, { untracked }, controller.signal)
        ).diff;
        const file = parseUnifiedDiff(text).find(
          (item) => item.hunks.length > 0,
        );
        if (file) files.push({ staged: false, file });
      }
      // staged 与 unstaged 同时存在时两段都取,只报一半会误导。
      if (stagedEntries.length > 0) {
        const text = (
          await workspaceGitApi.diff(
            relative,
            { staged: true },
            controller.signal,
          )
        ).diff;
        const file = parseUnifiedDiff(text).find(
          (item) => item.hunks.length > 0,
        );
        if (file) files.push({ staged: true, file });
      }
      if (files.length === 0) throw new Error("empty diff");
      if (!cancelled) {
        setState({
          phase: "git",
          branch: status.branch,
          relative,
          hasUnstaged: unstagedEntries.length > 0,
          files,
        });
      }
    })().catch(() => {
      if (!cancelled) setState({ phase: "fallback" });
    });
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [path, version]);
  return { state, reload: () => setVersion((value) => value + 1) };
}

/** 单文件 diff 视图:宽面板,文件头 + git 行号 diff 或逐次编辑的红绿行块。 */
function ChangeDiffPanel({
  change,
  onBack,
}: {
  change: FileChange;
  onBack?: () => void;
}) {
  const { t } = useTranslation();
  const projectDir = useChatStore((state) => state.project?.path ?? null);
  const { state: gitState, reload } = useGitDiff(change.path);
  const [undoState, setUndoState] = useState<
    "idle" | "confirm" | "busy" | "done" | "error"
  >("idle");
  const gitTotals =
    gitState.phase === "git"
      ? gitState.files.reduce(
          (sum, item) => ({
            additions: sum.additions + item.file.additions,
            deletions: sum.deletions + item.file.deletions,
          }),
          { additions: 0, deletions: 0 },
        )
      : null;
  const headerAdditions = gitTotals?.additions ?? change.additions;
  const headerDeletions = gitTotals?.deletions ?? change.deletions;
  const shortDir = directoryOf(shortenPath(change.path, projectDir));
  // 撤销只覆盖未暂存改动(后端 discard = restore + clean),
  // 纯 staged 改动撤不动,不给按钮以免"点了没反应"。
  const canUndo = gitState.phase === "git" && gitState.hasUnstaged;

  const performUndo = async () => {
    if (gitState.phase !== "git") return;
    setUndoState("busy");
    try {
      await workspaceGitApi.discard([gitState.relative]);
      setUndoState("done");
      reload();
    } catch {
      setUndoState("error");
    }
  };
  return (
    <aside className="flex min-h-0 w-[min(42rem,48vw)] min-w-[20rem] shrink-0 flex-col border-l border-line bg-bg/70">
      <div className="flex h-11 shrink-0 items-center gap-1 border-b border-line px-2">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            title={t("chat.panel.back")}
            aria-label={t("chat.panel.back")}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-ink-tertiary transition-colors hover:bg-fill-hover hover:text-ink"
          >
            <ChevronRight size={14} strokeWidth={1.8} className="rotate-180" />
          </button>
        )}
        <FileDiff size={14} strokeWidth={1.8} className="shrink-0 text-ink-secondary" />
        <span
          className="min-w-0 truncate text-[13px] font-medium text-ink"
          title={change.path}
        >
          {change.name}
        </span>
        <span className="min-w-0 flex-1 truncate text-[11px] text-ink-tertiary">
          {shortDir}
        </span>
        <ChangeStat additions={headerAdditions} deletions={headerDeletions} />
        {canUndo &&
          (undoState === "confirm" ? (
            <span className="ml-1 flex shrink-0 items-center gap-1">
              <button
                type="button"
                onClick={() => void performUndo()}
                className="rounded-[var(--radius-sm)] px-2 py-1 text-[12px] text-danger transition-colors hover:bg-danger-soft"
              >
                {t("common.confirm")}
              </button>
              <button
                type="button"
                onClick={() => setUndoState("idle")}
                className="rounded-[var(--radius-sm)] px-2 py-1 text-[12px] text-ink-secondary transition-colors hover:bg-fill-hover"
              >
                {t("common.cancel")}
              </button>
            </span>
          ) : (
            <button
              type="button"
              onClick={() => setUndoState("confirm")}
              disabled={undoState === "busy"}
              title={t("chat.diff.undo")}
              aria-label={t("chat.diff.undo")}
              className="ml-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon transition-colors hover:bg-fill-hover hover:text-icon-strong disabled:opacity-50"
            >
              {undoState === "busy" ? (
                <LoaderCircle size={14} strokeWidth={1.8} className="animate-spin" />
              ) : (
                <Undo2 size={14} strokeWidth={1.8} />
              )}
            </button>
          ))}
        <a
          href={filePreviewUrl(change.path)}
          target="_blank"
          rel="noreferrer"
          title={t("tool.file.open")}
          aria-label={t("tool.file.open")}
          className="ml-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
        >
          <ArrowUpRight size={14} strokeWidth={1.8} />
        </a>
      </div>
      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-3 py-3">
        {undoState === "done" || undoState === "error" ? (
          <div
            className={`rounded-[var(--radius-md)] px-3 py-2 text-xs ${
              undoState === "done"
                ? "bg-fill-active text-ok"
                : "bg-danger-soft text-danger"
            }`}
          >
            {undoState === "done"
              ? t("chat.diff.undone")
              : t("chat.diff.undoFailed")}
          </div>
        ) : null}
        {gitState.phase === "loading" ? (
          <div className="flex items-center gap-2 px-1 py-1.5 text-xs text-ink-tertiary">
            <LoaderCircle size={14} strokeWidth={1.8} className="animate-spin" />
            {t("chat.diff.loading")}
          </div>
        ) : gitState.phase === "git" ? (
          gitState.files.map((item) => (
            <GitDiffView
              key={item.staged ? "staged" : "unstaged"}
              branch={gitState.branch}
              staged={item.staged}
              file={item.file}
            />
          ))
        ) : (
          change.edits.map((edit, index) => (
            <DiffBlock
              key={`${edit.messageId}-${index}`}
              edit={edit}
              ordinal={change.edits.length > 1 ? index + 1 : 0}
            />
          ))
        )}
      </div>
    </aside>
  );
}

/** git diff 总行数上限,超出折叠;按 hunk 顺序消耗预算。 */
const MAX_GIT_DIFF_LINES = 800;

function GitDiffView({
  branch,
  staged,
  file,
}: {
  branch: string;
  staged?: boolean;
  file: UnifiedFileDiff;
}) {
  const { t } = useTranslation();
  const sections: {
    hunk: UnifiedFileDiff["hunks"][number];
    lines: UnifiedFileDiff["hunks"][number]["lines"];
  }[] = [];
  let remaining = MAX_GIT_DIFF_LINES;
  let total = 0;
  for (const hunk of file.hunks) {
    total += hunk.lines.length;
    if (remaining <= 0) continue;
    const lines = hunk.lines.slice(0, remaining);
    remaining -= lines.length;
    sections.push({ hunk, lines });
  }
  const truncated = total - (MAX_GIT_DIFF_LINES - remaining);

  return (
    <section className="overflow-hidden rounded-[var(--radius-md)] border border-line bg-surface">
      <div className="flex items-center justify-between border-b border-line px-3 py-1.5 text-[11px] text-ink-tertiary">
        <span className="flex items-center gap-1.5">
          <GitBranch size={12} strokeWidth={1.8} />
          {staged ? t("chat.diff.staged") : t("chat.diff.workingTree")} ·{" "}
          {branch}
        </span>
        <ChangeStat additions={file.additions} deletions={file.deletions} />
      </div>
      <div className="overflow-x-auto">
        <div className="min-w-max font-mono text-[12px] leading-[1.7]">
          {sections.map(({ hunk, lines }, sectionIndex) => (
            <Fragment key={sectionIndex}>
              <div className="bg-fill-hover/60 px-3 py-1 text-[11px] text-ink-muted">
                {`@@ -${hunk.oldStart},${hunk.oldCount} +${hunk.newStart},${hunk.newCount} @@`}
                {hunk.section ? ` ${hunk.section}` : ""}
              </div>
              {lines.map((line, lineIndex) => (
                <div
                  key={`${sectionIndex}-${lineIndex}`}
                  className={`flex pr-4 ${
                    line.kind === "add"
                      ? "bg-ok/10"
                      : line.kind === "remove"
                      ? "bg-danger-soft"
                      : ""
                  }`}
                >
                  <span className="w-10 shrink-0 select-none pr-2 text-right text-[11px] leading-[1.85] text-ink-muted">
                    {line.oldLine ?? ""}
                  </span>
                  <span className="w-10 shrink-0 select-none pr-2 text-right text-[11px] leading-[1.85] text-ink-muted">
                    {line.newLine ?? ""}
                  </span>
                  <span
                    className={`w-5 shrink-0 select-none text-center ${
                      line.kind === "add"
                        ? "text-ok"
                        : line.kind === "remove"
                        ? "text-danger"
                        : "text-ink-muted"
                    }`}
                  >
                    {line.kind === "add"
                      ? "+"
                      : line.kind === "remove"
                      ? "-"
                      : ""}
                  </span>
                  <span
                    className={`whitespace-pre ${
                      line.kind === "context" ? "text-ink-tertiary" : "text-ink"
                    }`}
                  >
                    {line.text || " "}
                  </span>
                </div>
              ))}
            </Fragment>
          ))}
        </div>
      </div>
      {truncated > 0 && (
        <div className="border-t border-line px-3 py-1.5 text-[11px] text-ink-tertiary">
          {t("chat.diff.truncated", { count: truncated })}
        </div>
      )}
    </section>
  );
}

const DIFF_TOOL_LABELS = {
  write_file: "chat.diff.write",
  edit_file: "chat.diff.edit",
  append_file: "chat.diff.append",
} as const;

/** 每块最多渲染这么多行,超出折叠成提示,防止整文件写入撑爆面板。 */
const MAX_DIFF_LINES = 600;

function DiffBlock({ edit, ordinal }: { edit: FileEdit; ordinal: number }) {
  const { t } = useTranslation();
  const lines = useMemo<DiffLine[]>(() => {
    if (edit.tool !== "edit_file") {
      return splitContentLines(edit.after).map((text) => ({
        kind: "add" as const,
        text,
      }));
    }
    // 超预算的大编辑不做行级对齐:整块删 + 整块加(统计口径一致)。
    if (edit.oversized) {
      return [
        ...splitContentLines(edit.before).map((text) => ({
          kind: "remove" as const,
          text,
        })),
        ...splitContentLines(edit.after).map((text) => ({
          kind: "add" as const,
          text,
        })),
      ];
    }
    return lineDiff(edit.before, edit.after);
  }, [edit]);
  const visible = lines.slice(0, MAX_DIFF_LINES);
  const truncated = lines.length - visible.length;
  return (
    <section className="overflow-hidden rounded-[var(--radius-md)] border border-line bg-surface">
      <div className="flex items-center justify-between border-b border-line px-3 py-1.5 text-[11px] text-ink-tertiary">
        <span>
          {t(DIFF_TOOL_LABELS[edit.tool])}
          {ordinal > 0 ? ` · ${ordinal}` : ""}
        </span>
        <ChangeStat additions={edit.additions} deletions={edit.deletions} />
      </div>
      <div className="overflow-x-auto">
        <div className="min-w-max font-mono text-[12px] leading-[1.7]">
          {visible.map((line, index) => (
            <div
              key={`${index}-${line.kind}`}
              className={`flex pr-4 ${
                line.kind === "add"
                  ? "bg-ok/10"
                  : line.kind === "remove"
                  ? "bg-danger-soft"
                  : ""
              }`}
            >
              <span
                className={`w-7 shrink-0 select-none text-center ${
                  line.kind === "add"
                    ? "text-ok"
                    : line.kind === "remove"
                    ? "text-danger"
                    : "text-ink-muted"
                }`}
              >
                {line.kind === "add" ? "+" : line.kind === "remove" ? "-" : ""}
              </span>
              <span
                className={`whitespace-pre ${
                  line.kind === "same" ? "text-ink-tertiary" : "text-ink"
                }`}
              >
                {line.text || " "}
              </span>
            </div>
          ))}
        </div>
      </div>
      {truncated > 0 && (
        <div className="border-t border-line px-3 py-1.5 text-[11px] text-ink-tertiary">
          {t("chat.diff.truncated", { count: truncated })}
        </div>
      )}
    </section>
  );
}

function splitContentLines(value: string): string[] {
  return value === "" ? [] : value.split("\n");
}

/**
 * markdown 预览里的相对资源(./img.png、docs/a.png)按当前文件目录
 * 解析成预览端点地址;协议地址、锚点、绝对 http 路径原样保留。
 */
function resolveMarkdownAsset(filePath: string, url: string): string {
  if (!url || /^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith("//")) {
    return url;
  }
  if (url.startsWith("#")) return url;
  const base = filePath.replaceAll("\\", "/").split("/").slice(0, -1);
  const segments = url.startsWith("/") ? [] : [...base];
  if (url.startsWith("/")) {
    // 以 / 开头视为工作区绝对路径,直接走预览端点
    return filePreviewUrl(url);
  }
  for (const part of url.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") segments.pop();
    else segments.push(part);
  }
  return filePreviewUrl(segments.join("/"));
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
      className={`flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[6px] border px-2 py-1 text-xs transition-[background-color,color,box-shadow,border-color] duration-[150ms] ease-out ${
        active
          ? "border-line bg-surface text-ink shadow-[var(--shadow-sm)] dark:border-line-highlight dark:shadow-none"
          : "border-transparent text-ink-secondary hover:text-ink"
      }`}
    >
      <Icon size={14} strokeWidth={1.8} />
      <span className="truncate">{label}</span>
    </button>
  );
}

function isSuccessfulToolOutput(message: StreamMessage): boolean {
  if (!isToolOutput(message.type) || message.status !== "completed") {
    return false;
  }
  const state = stringValue(toolData(message).state).toLocaleLowerCase();
  return isSuccessfulToolState(state);
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
  [
    [
      "ts",
      "tsx",
      "js",
      "jsx",
      "py",
      "go",
      "rs",
      "json",
      "yaml",
      "yml",
      "sh",
      "html",
      "css",
      "sql",
    ],
    FileCode,
  ],
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
