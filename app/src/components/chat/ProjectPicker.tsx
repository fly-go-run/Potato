import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ArrowLeft,
  Check,
  ChevronRight,
  Folder,
  FolderGit2,
  FolderOpen,
  Plus,
  Search,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { projectApi } from "../../lib/api";
import { cn } from "../../lib/cn";
import { hasNativeDialogs, pickDirectoryNative } from "../../lib/desktop";
import { useTranslation } from "../../lib/i18n";
import {
  loadRecentProjects,
  mergeProjects,
  projectNameFromPath,
  rememberRecentProject,
  type CodingProject,
  type CodingProjectInfo,
  type DirectoryListing,
  type ProjectBinding,
} from "../../lib/projects";
import { useChatStore } from "../../stores/chat";
import { Button, IconButton, Input, SkeletonRows, inputClasses } from "../ui";

/** 「浏览目录」「新建项目」两个子流程仍是独立 Dialog：多级导航塞不进轻弹层。 */
type SubFlow = "browse" | "create";

interface PickerEntry {
  key: string;
  name: string;
  path: string;
  isGit?: boolean;
  /** null = 默认工作区（解绑） */
  binding: ProjectBinding | null;
}

/**
 * 项目选择器（对标 WorkBuddy 的工作空间弹层）：
 * 挂靠在 composer 底栏的项目 chip 上方，搜索 + 列表 + 底部动作两行，
 * 无遮罩、不打断输入；子流程才升级成模态 Dialog。
 */
export function ProjectPicker() {
  const { t } = useTranslation();
  const project = useChatStore((state) => state.project);
  const setProject = useChatStore((state) => state.setProject);
  const [open, setOpen] = useState(false);
  const [flow, setFlow] = useState<SubFlow | null>(null);
  const [managed, setManaged] = useState<CodingProject[]>([]);
  const [current, setCurrent] = useState<CodingProjectInfo | null>(null);
  const [listing, setListing] = useState<DirectoryListing | null>(null);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [projectName, setProjectName] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  // 从弹层动作行唤起子流程时，别把焦点弹回 chip（会和 Dialog 抢焦点）
  const openingFlowRef = useRef(false);
  const projects = useMemo(
    () => mergeProjects(managed, loadRecentProjects()),
    [managed, open],
  );

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActiveIndex(0);
    setError(null);
    setLoading(true);
    // DropdownMenu 默认把焦点收在 content 上，下一帧抢回搜索框
    const focusTimer = window.setTimeout(() => searchRef.current?.focus(), 0);
    const clearFocusTimer = () => window.clearTimeout(focusTimer);
    void Promise.all([projectApi.current(), projectApi.list()])
      .then(([projectInfo, projectList]) => {
        setCurrent(projectInfo);
        setManaged(projectList);
      })
      .catch((reason: unknown) => setError(readableError(reason)))
      .finally(() => setLoading(false));
    return clearFocusTimer;
  }, [open]);

  const entries = useMemo<PickerEntry[]>(() => {
    const all: PickerEntry[] = [
      {
        key: "__workspace__",
        name: t("projects.defaultWorkspace"),
        path: current?.workspace_dir ?? "",
        binding: null,
      },
      ...projects.map((item) => ({
        key: item.path,
        name: item.name,
        path: item.path,
        isGit: item.isGit,
        binding: item,
      })),
    ];
    const keyword = query.trim().toLowerCase();
    if (!keyword) return all;
    return all.filter(
      (item) =>
        item.name.toLowerCase().includes(keyword) ||
        item.path.toLowerCase().includes(keyword),
    );
  }, [projects, current, query, t]);

  // 过滤后高亮项可能越界；键盘移动时保持高亮行可见
  useEffect(() => {
    setActiveIndex((index) =>
      entries.length === 0 ? 0 : Math.min(index, entries.length - 1),
    );
  }, [entries.length]);

  useEffect(() => {
    const row = listRef.current?.children[activeIndex] as
      | HTMLElement
      | undefined;
    row?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const selectProject = (selection: ProjectBinding | null) => {
    setProject(selection);
    setOpen(false);
  };

  const startFlow = (next: SubFlow) => {
    openingFlowRef.current = true;
    setError(null);
    setOpen(false);
    setFlow(next);
    if (next === "browse") void browse();
  };

  /* 桌面壳下「浏览目录」直接唤起系统原生目录选择器(对标 WB);
   * 自制的逐级浏览器只作为纯网页环境的兜底。 */
  const startBrowse = async () => {
    if (!hasNativeDialogs()) {
      startFlow("browse");
      return;
    }
    setOpen(false);
    const path = await pickDirectoryNative();
    if (!path) return; // 用户取消
    const selection = { path, name: projectNameFromPath(path) };
    rememberRecentProject(selection);
    selectProject(selection);
  };

  const closeFlow = () => {
    setFlow(null);
    setError(null);
    setProjectName("");
  };

  const browse = async (path = "~") => {
    setLoading(true);
    setError(null);
    try {
      setListing(await projectApi.browse(path));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setLoading(false);
    }
  };

  const selectBrowsedDirectory = () => {
    if (!listing || listing.selectable === false) return;
    const selection = {
      path: listing.current,
      name: projectNameFromPath(listing.current),
    };
    rememberRecentProject(selection);
    setFlow(null);
    selectProject(selection);
  };

  const createProject = async () => {
    const name = projectName.trim();
    if (!name) return;
    setSaving(true);
    setError(null);
    try {
      const created = await projectApi.create(name);
      const selection = { ...created, isGit: true };
      setManaged((items) => [
        ...items.filter((item) => item.path !== created.path),
        {
          path: created.path,
          name: created.name,
          is_git: true,
          is_active: true,
        },
      ]);
      setProjectName("");
      setFlow(null);
      selectProject(selection);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSaving(false);
    }
  };

  const onSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (entries.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => Math.min(index + 1, entries.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) => Math.max(index - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const entry = entries[activeIndex];
      if (entry) selectProject(entry.binding);
    }
    // Esc 交给 Radix 的 DismissableLayer 关闭弹层
  };

  return (
    <>
      {/* 用 DropdownMenu 做锚定壳（仓库未装 react-popover）：
       * 内部不放 Menu.Item，因此 typeahead / roving focus 不会和搜索框抢键盘。 */}
      <DropdownMenu.Root open={open} onOpenChange={setOpen} modal={false}>
        <DropdownMenu.Trigger asChild>
          <Button
            variant="ghost"
            size="sm"
            title={project?.path ?? t("projects.defaultWorkspace")}
            className="max-w-40 px-2 data-[state=open]:bg-fill-hover data-[state=open]:text-ink"
          >
            <Folder size={14} className="shrink-0" />
            <span className="truncate">
              {project?.name ?? t("projects.defaultWorkspace")}
            </span>
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            side="top"
            align="start"
            sideOffset={8}
            aria-label={t("projects.title")}
            onCloseAutoFocus={(event) => {
              if (openingFlowRef.current) {
                openingFlowRef.current = false;
                event.preventDefault();
              }
            }}
            className="qp-pop z-50 w-80 overflow-hidden rounded-[var(--radius-md)] border border-line bg-raised shadow-[var(--shadow-md)]"
          >
            <div className="relative border-b border-line p-1.5">
              <Search
                size={14}
                aria-hidden
                className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-ink-tertiary"
              />
              <input
                ref={searchRef}
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setActiveIndex(0);
                }}
                onKeyDown={onSearchKeyDown}
                placeholder={t("projects.search")}
                aria-label={t("projects.search")}
                className={cn(inputClasses, "h-8 pl-8 pr-2.5 text-[13px]")}
              />
            </div>

            {error && (
              <div className="border-b border-line px-3 py-2 text-xs text-danger">
                {error}
              </div>
            )}

            {loading ? (
              <div className="p-2" aria-label={t("projects.loading")}>
                <SkeletonRows rows={3} />
              </div>
            ) : entries.length === 0 ? (
              <div className="px-3 py-6 text-center text-[13px] text-ink-muted">
                {t("projects.noMatches")}
              </div>
            ) : (
              <div ref={listRef} className="max-h-64 overflow-y-auto p-1">
                {entries.map((entry, index) => {
                  const selected = entry.binding
                    ? project?.path === entry.binding.path
                    : !project;
                  const Icon = entry.isGit ? FolderGit2 : Folder;
                  return (
                    <button
                      key={entry.key}
                      type="button"
                      onClick={() => selectProject(entry.binding)}
                      onMouseEnter={() => setActiveIndex(index)}
                      title={entry.path || entry.name}
                      className={cn(
                        "flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-2 py-1.5 text-left",
                        index === activeIndex && "bg-fill-hover",
                      )}
                    >
                      <Icon
                        size={15}
                        className={cn(
                          "shrink-0",
                          entry.isGit ? "text-accent" : "text-ink-tertiary",
                        )}
                      />
                      <span className="min-w-0 flex-1">
                        <span
                          className={cn(
                            "block truncate text-[13px]",
                            selected ? "font-medium text-accent" : "text-ink",
                          )}
                        >
                          {entry.name}
                        </span>
                        {entry.path && (
                          <span className="mt-0.5 block truncate text-xs text-ink-tertiary">
                            {entry.path}
                          </span>
                        )}
                      </span>
                      {selected && (
                        <Check size={14} className="shrink-0 text-accent" />
                      )}
                    </button>
                  );
                })}
              </div>
            )}

            <div className="border-t border-line p-1">
              <ActionRow
                icon={<FolderOpen size={14} className="text-ink-tertiary" />}
                label={t("projects.browse")}
                onClick={() => void startBrowse()}
              />
              <ActionRow
                icon={<Plus size={14} className="text-ink-tertiary" />}
                label={t("projects.create")}
                onClick={() => startFlow("create")}
              />
            </div>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      {/* 子流程 1：多级目录浏览 */}
      <FlowDialog
        open={flow === "browse"}
        onClose={closeFlow}
        title={t("projects.browseTitle")}
        description={t("projects.browseDescription")}
        closeLabel={t("projects.close")}
        error={flow === "browse" ? error : null}
      >
        <div className="flex items-center gap-2 border-b border-line px-4 py-2">
          <IconButton
            size="sm"
            disabled={!listing?.parent || loading}
            onClick={() => {
              if (listing?.parent) void browse(listing.parent);
            }}
            title={t("projects.parent")}
          >
            <ArrowLeft size={15} />
          </IconButton>
          <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink-secondary">
            {listing?.current ?? t("projects.loading")}
          </span>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {loading ? (
            <div
              className="px-3 py-2"
              aria-label={t("projects.loadingDirectories")}
            >
              <SkeletonRows rows={5} />
            </div>
          ) : listing?.dirs.length ? (
            <div className="space-y-0.5">
              {listing.dirs.map((directory) => (
                <button
                  key={directory.path}
                  type="button"
                  onClick={() => void browse(directory.path)}
                  className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-ink-secondary hover:bg-fill-hover hover:text-ink"
                >
                  <Folder size={15} className="shrink-0 text-ink-muted" />
                  <span className="min-w-0 flex-1 truncate">
                    {directory.name}
                  </span>
                  <ChevronRight size={14} className="text-ink-muted" />
                </button>
              ))}
            </div>
          ) : (
            <div className="px-3 py-8 text-center text-sm text-ink-muted">
              {t("projects.noSubdirectories")}
            </div>
          )}
        </div>
        <footer className="flex items-center justify-end gap-2 border-t border-line px-4 py-3">
          <Button variant="ghost" size="sm" onClick={closeFlow}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={!listing || listing.selectable === false || loading}
            onClick={selectBrowsedDirectory}
          >
            {t("projects.selectCurrent")}
          </Button>
        </footer>
      </FlowDialog>

      {/* 子流程 2：新建项目 */}
      <FlowDialog
        open={flow === "create"}
        onClose={closeFlow}
        title={t("projects.createTitle")}
        description={t("projects.createDescription")}
        closeLabel={t("projects.close")}
        error={flow === "create" ? error : null}
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void createProject();
          }}
          className="p-5"
        >
          <label className="block text-xs font-medium text-ink-secondary">
            {t("projects.name")}
            <Input
              autoFocus
              value={projectName}
              onChange={(event) => setProjectName(event.target.value)}
              placeholder={t("projects.namePlaceholder")}
              className="mt-1.5"
            />
          </label>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={closeFlow}>
              {t("common.cancel")}
            </Button>
            <Button
              type="submit"
              variant="primary"
              size="sm"
              disabled={!projectName.trim() || saving}
            >
              {saving ? t("projects.creating") : t("projects.create")}
            </Button>
          </div>
        </form>
      </FlowDialog>
    </>
  );
}

function ActionRow({
  icon,
  label,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 text-left text-[13px] text-ink-secondary hover:bg-fill-hover hover:text-ink"
    >
      <span className="flex shrink-0 items-center">{icon}</span>
      <span className="truncate">{label}</span>
    </button>
  );
}

/** 子流程共用的模态外壳，沿用改造前的居中 Dialog 形态。 */
function FlowDialog({
  open,
  onClose,
  title,
  description,
  closeLabel,
  error,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description: string;
  closeLabel: string;
  error: string | null;
  children: ReactNode;
}) {
  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 flex max-h-[min(42rem,calc(100%-2rem))] w-[calc(100%-2rem)] max-w-2xl -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[var(--radius-lg)] border border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {title}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-muted">
                {description}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton size="sm" title={closeLabel}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          {error && (
            <div className="mx-5 mt-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
              {error}
            </div>
          )}
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
