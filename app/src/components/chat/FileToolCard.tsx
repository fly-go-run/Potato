import { useMemo, useState, type ReactNode } from "react";
import {
  ArrowUpRight,
  FileArchive,
  FileCode,
  FileImage,
  FilePenLine,
  FileSpreadsheet,
  FileText,
  Presentation,
  type LucideIcon,
} from "lucide-react";
import { filePreviewUrl } from "../../lib/api";
import { handleSystemOpenClick } from "../../lib/desktop";
import { type DiffLineKind } from "../../lib/lineDiff";
import { qpCount, recordLegacyParse } from "../../lib/toolMeta";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import { Collapse } from "./Collapse";
import { ToolDisclosure } from "./ToolDisclosure";
import type { ToolPair } from "./ToolCard";
import { richOutputText, toolPairStatus, ToolStatus } from "./ToolCard";
import {
  editDiffLines,
  pairChangeStats,
  pairFileEdit,
  visibleDiffLines,
  type FileEdit,
} from "../../lib/fileChanges";

const FILE_TOOL_TITLES: Record<string, TranslationKey> = {
  read_file: "tool.file.read",
  write_file: "tool.file.write",
  edit_file: "tool.file.edit",
  append_file: "tool.file.append",
  send_file_to_user: "tool.file.deliver",
};

/** 改动类工具:行内图标用笔形,并展示 ±行数;只读/发送保持素文件图标。 */
const MODIFYING_FILE_TOOLS = new Set([
  "write_file",
  "edit_file",
  "append_file",
]);

/** 产物 = 本轮真正落盘生成的文件；读取/改写不进入产物卡，保持安静行。 */
const ARTIFACT_TOOLS = new Set([
  "write_file",
  "append_file",
  "send_file_to_user",
]);

export function isFileTool(name: string): boolean {
  return Object.hasOwn(FILE_TOOL_TITLES, name);
}

export function isArtifactTool(name: string): boolean {
  return ARTIFACT_TOOLS.has(name);
}

/** 只有收到成功的终态输出，文件才算真正生成/交付。 */
export function isSuccessfulArtifactPair(pair: ToolPair): boolean {
  // qp meta 的 ok 是语义成败:执行完成(state=success)但语义失败
  // (如 send_file 文件不存在)时不能当产物。无 meta 走原判定。
  if (pair.meta && !pair.meta.ok) return false;
  return Boolean(isArtifactTool(pair.name) && toolPairStatus(pair).completed);
}

export function FileToolCard({
  pair,
  onOpenFile,
  onOpenChange,
  prominentArtifact = false,
  shimmer = false,
  open,
  onToggle,
}: {
  pair: ToolPair;
  onOpenFile?: (path: string) => void;
  /** 行内 diff 打开时的「在侧栏打开」;截断行也走这里。 */
  onOpenChange?: (path: string) => void;
  /** 仅在文件被明确交付给用户时展示大号产物卡。 */
  prominentArtifact?: boolean;
  shimmer?: boolean;
  open?: boolean;
  onToggle?: () => void;
}) {
  const { t } = useTranslation();
  const parameters = parseArguments(pair.arguments);
  const path =
    typeof parameters.file_path === "string" ? parameters.file_path : "";
  const { running, failed } = toolPairStatus(pair);
  // 笔/文件图标承担动词,行上不再写「正在写入/读取」。
  const modifies = MODIFYING_FILE_TOOLS.has(pair.name);
  // ±行数与汇总卡同源(meta 真值优先,回落同一套本地估算)。
  const stats = useMemo(
    () => (running || failed ? null : pairChangeStats(pair)),
    [running, failed, pair],
  );
  const inlineEdit = useMemo(
    () => (modifies ? pairFileEdit(pair) : null),
    [modifies, pair],
  );

  const detail = inlineEdit ? (
    <InlineDiffBlock
      edit={inlineEdit}
      path={path}
      onOpenSidebar={onOpenChange}
    />
  ) : (
    <div className="rounded-[var(--radius-md)] bg-surface px-3 py-2">
      <div className="mb-2 flex gap-3 text-xs">
        <span className="shrink-0 text-ink-tertiary">{t("tool.file.path")}</span>
        {path && onOpenFile ? (
          <button
            type="button"
            onClick={() => onOpenFile(path)}
            className="min-w-0 break-all text-left text-ink-secondary underline decoration-dotted underline-offset-2 hover:text-ink"
            title={t("tool.file.open")}
          >
            <code>{path}</code>
          </button>
        ) : (
          <code className="min-w-0 break-all text-ink-secondary">
            {path || t("tool.noResult")}
          </code>
        )}
      </div>
      <div>
        <div className="mb-2 text-xs font-medium text-ink-tertiary">
          {pair.name === "edit_file"
            ? t("tool.file.changes")
            : t("tool.file.content")}
        </div>
        <FileToolContent pair={pair} parameters={parameters} />
      </div>
    </div>
  );

  if (prominentArtifact && !running && isSuccessfulArtifactPair(pair) && path) {
    return (
      <ArtifactCard
        pair={pair}
        path={path}
        detail={detail}
        onOpenFile={onOpenFile}
      />
    );
  }

  const pathTone = failed
    ? "text-danger"
    : shimmer
      ? ""
      : "text-ink-tertiary group-hover:text-ink";
  // 行的整个点击面都归展开(行内 diff/内容)——文件名不再是独立链接,
  // 否则行的主区域被"跳侧栏"占据,展开反而只能点边缘。跳转统一走
  // 展开后的行尾 ArrowUpRight。
  const pathNode = (
    <span
      className={`min-w-0 flex-1 truncate font-mono text-[12px] ${pathTone} ${
        shimmer ? "qp-shimmer" : ""
      }`}
    >
      {path || t("tool.file.path")}
    </span>
  );

  const RowIcon = modifies ? FilePenLine : FileText;
  const toggle = (
    <>
      <RowIcon
        size={14}
        strokeWidth={1.8}
        className={`shrink-0 ${failed ? "text-danger" : "text-ink-muted"}`}
      />
    </>
  );
  const after = (
    <>
      {pathNode}
      {failed ? (
        <span className="shrink-0 pl-2 text-[11px] text-danger">
          {t("tool.file.failed")}
        </span>
      ) : (
        stats && (
          <span className="shrink-0 select-none pl-2 text-[11px] tabular-nums">
            <span className="text-ok">+{stats.additions}</span>
            {stats.deletions > 0 && (
              <span className="pl-1 text-danger">−{stats.deletions}</span>
            )}
          </span>
        )
      )}
      {running && <ToolStatus running failed={false} quiet />}
    </>
  );

  return (
    <ToolDisclosure
      toggle={toggle}
      after={after}
      trailing={(open) => {
        // 改动行去侧栏 diff,只读行去文件预览——同一颗行尾按钮分流
        const jump = modifies ? onOpenChange : onOpenFile;
        return open && path && jump ? (
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              jump(path);
            }}
            title={modifies ? t("chat.panel.open") : t("tool.file.open")}
            aria-label={modifies ? t("chat.panel.open") : t("tool.file.open")}
            className="shrink-0 rounded-[var(--radius-sm)] p-0.5 text-icon opacity-0 transition-opacity duration-[var(--dur-fast)] hover:text-icon-strong group-hover:opacity-100"
          >
            <ArrowUpRight size={12} strokeWidth={1.8} />
          </button>
        ) : null;
      }}
      toggleGrow={false}
      failed={failed}
      open={open}
      onToggle={onToggle}
      detailClassName="mb-1 mt-0.5 max-h-[min(20rem,42vh)] overflow-y-auto overscroll-contain"
    >
      {detail}
    </ToolDisclosure>
  );
}

/**
 * 产物文件卡：满宽、无描边、bg-bubble-tool 底，图标 + 文件名 + 大小/目录，
 * 右侧「打开」按钮走既有的文件预览接口。仍可展开看写入内容，但默认收起，
 * 保持「正文当主角」。
 */
function ArtifactCard({
  pair,
  path,
  detail,
  onOpenFile,
}: {
  pair: ToolPair;
  path: string;
  detail: ReactNode;
  onOpenFile?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const Icon = fileIcon(path);
  const name = fileBaseName(path) || path;
  const meta = fileSizeLabel(pair) || directoryOf(path);

  return (
    <div className="my-2 overflow-hidden rounded-[var(--radius-md)] bg-bubble-tool">
      <div className="flex items-center gap-3 px-3 py-2.5">
        <button
          type="button"
          onClick={() =>
            onOpenFile ? onOpenFile(path) : setExpanded((value) => !value)
          }
          aria-expanded={expanded}
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
          title={path}
        >
          <Icon size={20} strokeWidth={1.75} className="shrink-0 text-ink-secondary" />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px] font-medium text-ink">
              {name}
            </span>
            {meta && (
              <span className="mt-0.5 block truncate text-[12px] text-ink-tertiary">
                {meta}
              </span>
            )}
          </span>
        </button>
        <a
          href={filePreviewUrl(path)}
          target="_blank"
          rel="noreferrer"
          title={t("tool.file.open")}
          aria-label={t("tool.file.open")}
          onClick={(event) => handleSystemOpenClick(event, path)}
          className="shrink-0 rounded-[var(--radius-sm)] p-1.5 text-icon transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:text-icon-strong"
        >
          <ArrowUpRight size={14} strokeWidth={1.8} />
        </a>
      </div>
      <Collapse open={expanded}>
        <div className="px-3 pb-3">{detail}</div>
      </Collapse>
    </div>
  );
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
      "java",
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
  return path.split(/[/\\]/).at(-1) ?? "";
}

function directoryOf(path: string): string {
  // 裸文件名(无目录段)返回空——回落到路径会让卡片把文件名重复两遍。
  const directory = path.slice(0, path.length - fileBaseName(path).length);
  return directory.replace(/[/\\]$/, "");
}

/**
 * 大小优先读 qp meta(file_write.bytes_written / file_sent.size_bytes)。
 * 历史会话无 meta 时回落文本解析:write/append 的 "Wrote 1234 bytes to …"
 * 正则与 send_file 的 "File sent successfully." 块匹配(legacy 路径,
 * dev 构建计数以便验收断言新会话不再触发)。
 */
function fileSizeLabel(pair: ToolPair): string {
  const metaBytes =
    qpCount(pair.meta, "bytes_written") ?? qpCount(pair.meta, "size_bytes");
  if (metaBytes !== null) return formatBytes(metaBytes);
  if (pair.meta) return ""; // 有 meta 但无大小字段(ok=false 等):不猜。

  const result = pair.result;
  if (!result) return "";
  const text = richOutputText(result);
  if (typeof text === "string") {
    const match = /(\d+)\s*bytes/i.exec(text);
    if (match) {
      recordLegacyParse("F1:bytes-regex");
      return formatBytes(Number(match[1]));
    }
  }
  const sentFileBytes = sentFileSizeBytes(result);
  return sentFileBytes === null ? "" : formatBytes(sentFileBytes);
}

function sentFileSizeBytes(result: string): number | null {
  try {
    const blocks = JSON.parse(result) as unknown;
    if (!Array.isArray(blocks)) return null;
    const delivered = blocks.some(
      (block) =>
        isRecord(block) &&
        block.type === "text" &&
        block.text === "File sent successfully.",
    );
    if (!delivered) return null;
    recordLegacyParse("F2:sent-file-text");

    for (const block of blocks) {
      if (!isRecord(block) || !isRecord(block.source)) continue;
      if (block.source.type !== "url" || typeof block.source.url !== "string") {
        continue;
      }
      const bytes = byteSize(block) ?? byteSize(block.source);
      if (bytes !== null) return bytes;
    }
  } catch {
    // 非结构化结果继续使用既有目录回退。
  }
  return null;
}

function byteSize(value: Record<string, unknown>): number | null {
  for (const key of ["size_bytes", "byte_size", "size"]) {
    const raw = value[key];
    const bytes =
      typeof raw === "number"
        ? raw
        : typeof raw === "string" && /^\d+$/.test(raw)
        ? Number(raw)
        : Number.NaN;
    if (Number.isFinite(bytes) && bytes >= 0) return bytes;
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  const kilobytes = bytes / 1024;
  if (kilobytes < 1024) return `${kilobytes.toFixed(1)} KB`;
  return `${(kilobytes / 1024).toFixed(1)} MB`;
}

function FileToolContent({
  pair,
  parameters,
}: {
  pair: ToolPair;
  parameters: Record<string, unknown>;
}) {
  const { t } = useTranslation();
  const argumentContent =
    typeof parameters.content === "string" ? parameters.content : "";
  const rawContent =
    pair.name === "read_file"
      ? richOutputText(pair.result)
      : argumentContent || pair.arguments;
  const content =
    typeof rawContent === "string"
      ? rawContent
      : rawContent
      ? JSON.stringify(rawContent, null, 2)
      : "";

  if (!content) {
    return (
      <div className="text-xs text-ink-tertiary">
        {!pair.output && pair.call?.status === "in_progress"
          ? t("tool.running")
          : t("tool.noResult")}
      </div>
    );
  }
  return (
    <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-md bg-bg px-3 py-2 font-mono text-xs leading-5 text-ink">
      {content}
    </pre>
  );
}

const DIFF_SIGN: Record<DiffLineKind, string> = {
  add: "+",
  remove: "-",
  same: "",
};

/** 轨道内联 diff:与侧栏 DiffBlock 同色,无边框圆角 surface。 */
function InlineDiffBlock({
  edit,
  path,
  onOpenSidebar,
}: {
  edit: FileEdit;
  path: string;
  onOpenSidebar?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const { visible, truncated } = useMemo(
    () => visibleDiffLines(editDiffLines(edit)),
    [edit],
  );
  const truncatedLabel = t("chat.diff.inlineTruncated", { count: truncated });

  return (
    <div
      data-inline-diff
      className="overflow-hidden rounded-[var(--radius-md)] bg-surface"
    >
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
                className={`w-7 shrink-0 select-none text-center ${diffSignClass(
                  line.kind,
                )}`}
              >
                {DIFF_SIGN[line.kind]}
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
      {truncated > 0 &&
        (onOpenSidebar && path ? (
          <button
            type="button"
            onClick={() => onOpenSidebar(path)}
            className="flex w-full px-3 py-1.5 text-left text-[11px] text-ink-tertiary transition-colors duration-[var(--dur-fast)] hover:text-ink"
          >
            {truncatedLabel}
          </button>
        ) : (
          <div className="px-3 py-1.5 text-[11px] text-ink-tertiary">
            {truncatedLabel}
          </div>
        ))}
    </div>
  );
}

function diffSignClass(kind: DiffLineKind): string {
  if (kind === "remove") return "text-danger";
  if (kind === "add") return "text-ok";
  return "text-ink-muted";
}

function parseArguments(value: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}
