import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { isMacDesktopShell, startDesktopWindowDrag } from "../../lib/desktop";
import {
  Blocks,
  ChevronDown,
  Clock3,
  FolderClosed,
  MessageCirclePlus,
  Moon,
  MoreHorizontal,
  NotebookPen,
  PanelLeft,
  PenLine,
  PenSquare,
  Pin,
  PinOff,
  Search,
  Settings,
  Sun,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState, type MouseEvent } from "react";
import { PotatoMark } from "../brand/PotatoMark";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import type { ChatSpec } from "../../lib/api";
import { APP_NAME } from "../../lib/appInfo";
import { presentError } from "../../lib/errorPresentation";
import { useTranslation } from "../../lib/i18n";
import { relativeTime } from "../../lib/relativeTime";
import { loadSessionProject } from "../../lib/projects";
import { shortcutLabel } from "../../lib/shortcuts";
import { setThemePreference } from "../../lib/theme";
import { useChatStore } from "../../stores/chat";
import { useUiStore } from "../../stores/ui";
import { Button, ConfirmDialog, IconButton, Input, SkeletonRows } from "../ui";

export function Sidebar({ onSearch }: { onSearch: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const chats = useChatStore((state) => state.chats);
  const chatsLoading = useChatStore((state) => state.chatsLoading);
  const activeChatId = useChatStore((state) => state.activeChatId);
  const newChat = useChatStore((state) => state.newChat);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);
  // 分组展开态只活在内存里：刷新后回到默认展开，不值得占一个持久化键。
  const [chatsExpanded, setChatsExpanded] = useState(true);
  const [projectsExpanded, setProjectsExpanded] = useState(true);
  const [expandedProjectPaths, setExpandedProjectPaths] = useState<
    Record<string, boolean>
  >({});
  const groupedChats = useMemo(() => {
    const unbound: ChatSpec[] = [];
    const byPath = new Map<
      string,
      { name: string; path: string; chats: ChatSpec[] }
    >();
    for (const chat of chats) {
      const project = loadSessionProject(chat.session_id);
      if (!project) {
        unbound.push(chat);
        continue;
      }
      const group = byPath.get(project.path) ?? {
        name: project.name,
        path: project.path,
        chats: [],
      };
      group.chats.push(chat);
      byPath.set(project.path, group);
    }
    return { unbound, workspaces: Array.from(byPath.values()) };
  }, [chats]);

  const startNewChat = () => {
    newChat();
    navigate("/");
  };

  const onTitlebarMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (!isMacDesktopShell() || event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (
      target.closest(
        "button, a, input, textarea, select, [role=button], [data-radix-popper-content-wrapper]",
      )
    ) {
      return;
    }
    startDesktopWindowDrag();
  };

  return (
    // 深色下侧栏（bg）比画布（canvas）更亮，抬升本身已经分层，
    // 再加一条比两者都亮的描边会变成刺眼的接缝 → 深色去掉右边框。
    <aside className="flex h-full min-h-0 w-[16.5rem] shrink-0 flex-col border-r border-line bg-bg dark:border-r-transparent">
      {/* macOS overlay 标题栏：两个常用入口与红绿灯同排；按钮之外的
          空白仍是原生拖拽区。Web 端保持普通页面内工具栏的位置。 */}
      <div
        data-tauri-drag-region
        onMouseDown={onTitlebarMouseDown}
        className={`flex h-11 shrink-0 items-center justify-end gap-3 pt-1 ${
          isMacDesktopShell() ? "pl-[4.75rem] pr-3" : "px-3"
        }`}
      >
        <IconButton
          size="sm"
          title={`${t("sidebar.collapse")} · ${shortcutLabel("B")}`}
          aria-label={t("sidebar.collapse")}
          onClick={toggleSidebar}
        >
          <PanelLeft size={16} />
        </IconButton>
        <IconButton
          size="sm"
          title={`${t("sidebar.newChat")} · ${shortcutLabel("N")}`}
          aria-label={t("sidebar.newChat")}
          onClick={startNewChat}
        >
          <MessageCirclePlus size={17} />
        </IconButton>
      </div>

      <div className="flex items-center px-6 pb-4 pt-3">
        <div className="min-w-0 leading-none">
          <div className="truncate text-[15px] font-semibold tracking-[-0.01em] text-ink">
            {APP_NAME}
          </div>
        </div>
      </div>

      <div className="px-3 pb-4">
        <button
          type="button"
          onClick={startNewChat}
          className={`flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-left text-sm leading-5 text-ink transition-colors duration-[var(--dur-fast)] ${
            location.pathname === "/" ? "bg-fill-active" : "hover:bg-fill-hover"
          }`}
        >
          <PenSquare size={16} className="text-ink-muted" />
          <span className="flex-1">{t("sidebar.newChat")}</span>
        </button>
        <NavLink
          to="/crons"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm leading-5 transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Clock3
                size={16}
                className={isActive ? "text-ink-secondary" : "text-ink-muted"}
              />
              <span className="flex-1">{t("sidebar.crons")}</span>
            </>
          )}
        </NavLink>
        <NavLink
          to="/skills"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm leading-5 transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Blocks
                size={16}
                className={isActive ? "text-ink-secondary" : "text-ink-muted"}
              />
              <span className="flex-1">{t("sidebar.skills")}</span>
            </>
          )}
        </NavLink>
        <NavLink
          to="/memory"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm leading-5 transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <NotebookPen
                size={16}
                className={isActive ? "text-ink-secondary" : "text-ink-muted"}
              />
              <span className="flex-1">{t("sidebar.memory")}</span>
            </>
          )}
        </NavLink>
        <button
          type="button"
          onClick={onSearch}
          className="mt-0.5 flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-left text-sm leading-5 text-ink transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
        >
          <Search size={16} className="text-ink-muted" />
          <span className="flex-1">{t("sidebar.searchChats")}</span>
        </button>
      </div>

      <nav className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        {/* Codex 的层级更符合工作台心智：先找项目，再找不属于项目的普通会话。 */}
        {groupedChats.workspaces.length > 0 && (
          <div>
            <button
              type="button"
              onClick={() => setProjectsExpanded((value) => !value)}
              aria-expanded={projectsExpanded}
              title={
                projectsExpanded
                  ? t("sidebar.collapseGroup")
                  : t("sidebar.expandGroup")
              }
              className="flex w-full items-center gap-1 rounded-[var(--radius-sm)] px-3 py-2 text-left text-xs text-ink-tertiary transition-colors hover:text-ink-secondary"
            >
              <span className="truncate">
                {t("sidebar.projectsGroup", {
                  count: groupedChats.workspaces.length,
                })}
              </span>
              <ChevronDown
                size={13}
                className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
                  projectsExpanded ? "" : "-rotate-90"
                }`}
              />
            </button>
            {projectsExpanded && (
              <div className="space-y-1.5">
                {groupedChats.workspaces.map((project) => {
                  const open = expandedProjectPaths[project.path] ?? true;
                  return (
                    <div key={project.path}>
                      <button
                        type="button"
                        onClick={() =>
                          setExpandedProjectPaths((value) => ({
                            ...value,
                            [project.path]: !open,
                          }))
                        }
                        aria-expanded={open}
                        title={project.path}
                        className="flex w-full items-center gap-2 rounded-[var(--radius-sm)] px-3 py-2 text-left text-[12px] text-ink-secondary hover:bg-fill-hover"
                      >
                        <FolderClosed
                          size={14}
                          className="shrink-0 text-ink-muted"
                        />
                        <span className="min-w-0 flex-1 truncate">
                          {project.name}
                        </span>
                        <span className="text-[10px] tabular-nums text-ink-muted">
                          {project.chats.length}
                        </span>
                        <ChevronDown
                          size={12}
                          className={`shrink-0 text-ink-muted transition-transform ${
                            open ? "" : "-rotate-90"
                          }`}
                        />
                      </button>
                      {open && (
                        <div className="ml-3 border-l border-line pl-1">
                          {project.chats.map((chat) => (
                            <ChatRow
                              key={chat.id}
                              chat={chat}
                              active={chat.id === activeChatId}
                              nested
                            />
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        <div className={groupedChats.workspaces.length > 0 ? "mt-3" : ""}>
          <button
            type="button"
            onClick={() => setChatsExpanded((value) => !value)}
            aria-expanded={chatsExpanded}
            title={
              chatsExpanded
                ? t("sidebar.collapseGroup")
                : t("sidebar.expandGroup")
            }
            className="flex w-full items-center gap-1 rounded-[var(--radius-sm)] px-3 py-2 text-left text-xs text-ink-tertiary transition-colors duration-[var(--dur-fast)] hover:text-ink-secondary"
          >
            <span className="truncate">
              {t("sidebar.chatsGroup", { count: groupedChats.unbound.length })}
            </span>
            <ChevronDown
              size={13}
              className={`shrink-0 transition-transform duration-[var(--dur-fast)] ${
                chatsExpanded ? "" : "-rotate-90"
              }`}
            />
          </button>
          {chatsExpanded &&
            (chatsLoading && chats.length === 0 ? (
              <div className="px-2 py-1">
                <SkeletonRows rows={4} />
              </div>
            ) : groupedChats.unbound.length === 0 ? (
              <div className="px-3 py-2 text-[13px] leading-5 text-ink-muted">
                {groupedChats.workspaces.length === 0
                  ? t("sidebar.empty")
                  : t("sidebar.unboundEmpty")}
              </div>
            ) : (
              <div className="space-y-1">
                {groupedChats.unbound.map((chat) => (
                  <ChatRow
                    key={chat.id}
                    chat={chat}
                    active={chat.id === activeChatId}
                  />
                ))}
              </div>
            ))}
        </div>
      </nav>

      <div className="flex min-h-[56px] items-center gap-2 border-t border-line px-3 py-2.5">
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-line bg-surface text-ink-secondary shadow-[var(--shadow-control)]">
            <PotatoMark size={18} />
          </span>
          <div className="min-w-0">
            <div className="truncate text-[14px] font-semibold text-ink">
              {APP_NAME}
            </div>
          </div>
        </div>
        <ThemeToggle />
        <NavLink
          to="/settings"
          state={{ background: location }}
          title={t("sidebar.settings")}
          aria-label={t("sidebar.settings")}
          className={({ isActive }) =>
            `inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink-muted hover:bg-fill-hover hover:text-ink"
            }`
          }
        >
          <Settings size={16} />
        </NavLink>
      </div>
    </aside>
  );
}

function isDarkTheme() {
  return (
    typeof document !== "undefined" &&
    document.documentElement.classList.contains("dark")
  );
}

/**
 * 浅/深一键切换：复用 lib/theme 的偏好写入（与设置页同一套机制），
 * 通过监听 <html> 的 class 变化保持图标与设置页的三档选择同步。
 */
function ThemeToggle() {
  const { t } = useTranslation();
  const [dark, setDark] = useState(isDarkTheme);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    setDark(root.classList.contains("dark"));
    const observer = new MutationObserver(() =>
      setDark(root.classList.contains("dark")),
    );
    observer.observe(root, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  const label = dark ? t("sidebar.theme.toLight") : t("sidebar.theme.toDark");

  return (
    <IconButton
      size="sm"
      title={label}
      aria-label={label}
      onClick={() => setThemePreference(dark ? "light" : "dark")}
    >
      {dark ? <Sun size={16} /> : <Moon size={16} />}
    </IconButton>
  );
}

function ChatRow({
  chat,
  active,
  nested = false,
}: {
  chat: ChatSpec;
  active: boolean;
  nested?: boolean;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameValue, setRenameValue] = useState(chat.name);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [pinError, setPinError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const renameChat = useChatStore((state) => state.renameChat);
  const togglePinned = useChatStore((state) => state.togglePinned);
  const deleteChat = useChatStore((state) => state.deleteChat);
  const clearError = useChatStore((state) => state.clearError);
  // 每次渲染现算：列表本身随会话更新重渲染，不额外挂定时器。
  const updatedAt = relativeTime(chat.updated_at);

  useEffect(() => {
    if (!pinError) return;
    const timer = window.setTimeout(() => setPinError(null), 4000);
    return () => window.clearTimeout(timer);
  }, [pinError]);

  const openContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    setMenuOpen(true);
  };

  const submitRename = async () => {
    if (!renameValue.trim()) return;
    setRenameError(null);
    setBusy(true);
    clearError();
    try {
      await renameChat(chat.id, renameValue);
    } catch (reason) {
      setRenameError(actionFailureMessage(reason));
      setBusy(false);
      return;
    }
    const failure = actionFailureMessage();
    if (failure) {
      setRenameError(failure);
      setBusy(false);
      return;
    }
    setBusy(false);
    setRenameOpen(false);
  };

  const confirmDelete = async () => {
    setDeleteError(null);
    setBusy(true);
    clearError();
    try {
      await deleteChat(chat.id);
    } catch (reason) {
      setDeleteError(actionFailureMessage(reason));
      setBusy(false);
      return;
    }
    const failure = actionFailureMessage();
    if (failure) {
      setDeleteError(failure);
      setBusy(false);
      return;
    }
    setBusy(false);
    setDeleteOpen(false);
  };

  const togglePin = async () => {
    setPinError(null);
    setBusy(true);
    clearError();
    try {
      await togglePinned(chat.id);
    } catch (reason) {
      setPinError(actionFailureMessage(reason));
      setBusy(false);
      return;
    }
    const failure = actionFailureMessage();
    if (failure) setPinError(failure);
    setBusy(false);
  };

  const actionFailureMessage = (reason?: unknown) => {
    const source = reason ?? useChatStore.getState().error;
    if (source === null || source === undefined || source === "") return null;
    const presentation = presentError(source);
    return t("sidebar.actionFailed", {
      message: t(presentation.summaryKey),
    });
  };

  return (
    <>
      <DropdownMenu.Root open={menuOpen} onOpenChange={setMenuOpen}>
        <div
          onContextMenu={openContextMenu}
          className={`group relative flex items-center rounded-md transition-colors duration-[var(--dur-fast)] ${
            active ? "bg-fill-active" : "hover:bg-fill-hover"
          }`}
        >
          <button
            type="button"
            onClick={() => navigate(`/chat/${chat.id}`)}
            className={`flex min-w-0 flex-1 items-center gap-2 overflow-hidden py-2 pr-1 text-left text-sm leading-5 ${
              nested ? "pl-2" : "pl-3"
            } ${active ? "text-ink" : "text-ink-secondary"}`}
          >
            {chat.pinned && (
              <Pin size={12} className={`shrink-0 text-ink-muted`} />
            )}
            <span className="min-w-0 flex-1 truncate">
              {chat.name || t("sidebar.untitled")}
            </span>
          </button>
          {/*
          时间与操作按钮共用行尾这块空间：时间占流内宽度（给按钮预留位置），
          hover / 菜单打开时淡出让位，绝对定位的「…」按钮淡入覆盖上来。
        */}
          <span
            aria-hidden={menuOpen ? true : undefined}
            className={`pointer-events-none min-w-9 shrink-0 pr-2.5 text-right text-[11px] tabular-nums text-ink-tertiary transition-opacity duration-[var(--dur-fast)] ${
              menuOpen ? "opacity-0" : "group-hover:opacity-0"
            }`}
          >
            {updatedAt ? t(updatedAt.key, updatedAt.params) : ""}
          </span>
          <DropdownMenu.Trigger asChild>
            <IconButton
              size="sm"
              title={t("sidebar.chatActions")}
              className="absolute right-1 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100 data-[state=open]:opacity-100"
            >
              <MoreHorizontal size={15} />
            </IconButton>
          </DropdownMenu.Trigger>
        </div>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="end"
            sideOffset={4}
            className="qp-pop z-50 min-w-32 rounded-[var(--radius-md)] border border-line bg-raised p-1 shadow-[var(--shadow-md)]"
          >
            <MenuItem
              icon={<PenLine size={14} />}
              label={t("sidebar.rename")}
              onSelect={() => {
                setRenameValue(chat.name);
                setRenameError(null);
                setRenameOpen(true);
              }}
            />
            <MenuItem
              icon={chat.pinned ? <PinOff size={14} /> : <Pin size={14} />}
              label={chat.pinned ? t("sidebar.unpin") : t("sidebar.pin")}
              onSelect={() => void togglePin()}
            />
            <DropdownMenu.Separator className="my-1 h-px bg-line" />
            <DropdownMenu.Item
              onSelect={() => {
                setDeleteError(null);
                setDeleteOpen(true);
              }}
              className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-danger outline-none hover:bg-danger-soft focus:bg-danger-soft"
            >
              <Trash2 size={14} />
              {t("sidebar.delete")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      {pinError && (
        <div
          role="alert"
          className="mx-2 mb-1 rounded-md bg-danger-soft px-2.5 py-1.5 text-[11px] leading-4 text-danger"
        >
          {pinError}
        </div>
      )}
      <Dialog.Root
        open={renameOpen}
        onOpenChange={(open) => {
          setRenameOpen(open);
          if (!open) setRenameError(null);
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
          <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 w-[calc(100%-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-lg)] border border-line bg-raised p-5 shadow-[var(--shadow-lg)] outline-none">
            <Dialog.Title className="text-sm font-semibold text-ink">
              {t("sidebar.rename")}
            </Dialog.Title>
            <Dialog.Description className="mt-1.5 text-sm text-ink-secondary">
              {t("sidebar.renamePrompt")}
            </Dialog.Description>
            <form
              className="mt-4"
              onSubmit={(event) => {
                event.preventDefault();
                void submitRename();
              }}
            >
              <Input
                autoFocus
                value={renameValue}
                disabled={busy}
                aria-label={t("sidebar.rename")}
                onChange={(event) => setRenameValue(event.target.value)}
              />
              {renameError && (
                <div
                  role="alert"
                  className="mt-3 rounded-md bg-danger-soft px-2.5 py-2 text-xs text-danger"
                >
                  {renameError}
                </div>
              )}
              <div className="mt-5 flex justify-end gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy}
                  onClick={() => setRenameOpen(false)}
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  type="submit"
                  variant="primary"
                  size="sm"
                  disabled={busy || !renameValue.trim()}
                >
                  {t("common.confirm")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      <ConfirmDialog
        open={deleteOpen}
        title={t("sidebar.delete")}
        description={
          deleteError
            ? `${t("sidebar.deleteConfirm", {
                name: chat.name || t("sidebar.untitled"),
              })}\n${deleteError}`
            : t("sidebar.deleteConfirm", {
                name: chat.name || t("sidebar.untitled"),
              })
        }
        tone="danger"
        busy={busy}
        onOpenChange={(open) => {
          setDeleteOpen(open);
          if (!open) setDeleteError(null);
        }}
        onConfirm={() => void confirmDelete()}
      />
    </>
  );
}

function MenuItem({
  icon,
  label,
  onSelect,
}: {
  icon: React.ReactNode;
  label: string;
  onSelect: () => void;
}) {
  return (
    <DropdownMenu.Item
      onSelect={onSelect}
      className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active"
    >
      {icon}
      {label}
    </DropdownMenu.Item>
  );
}
