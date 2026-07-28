import * as Dialog from "@radix-ui/react-dialog";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  Blocks,
  Clock3,
  Inbox,
  MoreHorizontal,
  NotebookPen,
  PenLine,
  PenSquare,
  Pin,
  PinOff,
  Search,
  Settings,
  Trash2,
} from "lucide-react";
import { useState, type MouseEvent } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import type { ChatSpec } from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import { shortcutLabel } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useInboxStore } from "../../stores/inbox";
import {
  Button,
  ConfirmDialog,
  CountBadge,
  IconButton,
  Input,
  SkeletonRows,
} from "../ui";

export function Sidebar({ onSearch }: { onSearch: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { chats, chatsLoading, activeChatId, newChat } = useChatStore();
  const unreadCount = useInboxStore((state) => state.unreadCount);

  const startNewChat = () => {
    newChat();
    navigate("/");
  };

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-line bg-bg">
      <div className="p-3">
        <button
          type="button"
          onClick={startNewChat}
          className="flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-left text-sm text-ink transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
        >
          <PenSquare size={16} className="text-ink-muted" />
          <span className="flex-1">{t("sidebar.newChat")}</span>
          <kbd className="text-[11px] text-ink-muted">
            {shortcutLabel("N")}
          </kbd>
        </button>
        <NavLink
          to="/crons"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Clock3 size={16} className={isActive ? "text-ink-secondary" : "text-ink-muted"} />
              <span className="flex-1">{t("sidebar.crons")}</span>
            </>
          )}
        </NavLink>
        <NavLink
          to="/inbox"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Inbox size={16} className={isActive ? "text-ink-secondary" : "text-ink-muted"} />
              <span className="flex-1">{t("sidebar.inbox")}</span>
              <CountBadge count={unreadCount} />
            </>
          )}
        </NavLink>
        <NavLink
          to="/skills"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Blocks size={16} className={isActive ? "text-ink-secondary" : "text-ink-muted"} />
              <span className="flex-1">{t("sidebar.skills")}</span>
            </>
          )}
        </NavLink>
        <NavLink
          to="/memory"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <NotebookPen size={16} className={isActive ? "text-ink-secondary" : "text-ink-muted"} />
              <span className="flex-1">{t("sidebar.memory")}</span>
            </>
          )}
        </NavLink>
        <button
          type="button"
          onClick={onSearch}
          className="mt-0.5 flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-left text-sm text-ink transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
        >
          <Search size={16} className="text-ink-muted" />
          <span className="flex-1">{t("sidebar.searchChats")}</span>
          <kbd className="text-[11px] text-ink-muted">
            {shortcutLabel("K")}
          </kbd>
        </button>
      </div>

      <nav className="min-h-0 flex-1 overflow-y-auto px-3">
        <div className="px-3 pb-1 pt-2 text-xs text-ink-muted">
          {t("sidebar.recentChats")}
        </div>
        {chatsLoading && chats.length === 0 ? (
          <div className="px-2 py-1">
            <SkeletonRows rows={4} />
          </div>
        ) : chats.length === 0 ? (
          <div className="px-3 py-2 text-sm text-ink-muted">
            {t("sidebar.empty")}
          </div>
        ) : (
          <div className="space-y-0.5">
            {chats.map((chat) => (
              <ChatRow
                key={chat.id}
                chat={chat}
                active={chat.id === activeChatId}
              />
            ))}
          </div>
        )}
      </nav>

      <div className="border-t border-line p-3">
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            `flex items-center gap-2.5 rounded-[var(--radius-sm)] px-3 py-2 text-sm transition-colors duration-[var(--dur-fast)] ${
              isActive
                ? "bg-fill-active text-ink"
                : "text-ink-secondary hover:bg-fill-hover"
            }`
          }
        >
          {({ isActive }) => (
            <>
              <Settings size={16} className={isActive ? "text-ink-secondary" : "text-ink-muted"} />
              {t("sidebar.settings")}
            </>
          )}
        </NavLink>
      </div>
    </aside>
  );
}

function ChatRow({ chat, active }: { chat: ChatSpec; active: boolean }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameValue, setRenameValue] = useState(chat.name);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const { renameChat, togglePinned, deleteChat } = useChatStore();

  const openContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    setMenuOpen(true);
  };

  const submitRename = async () => {
    if (!renameValue.trim()) return;
    setBusy(true);
    await renameChat(chat.id, renameValue);
    setBusy(false);
    setRenameOpen(false);
  };

  const confirmDelete = async () => {
    setBusy(true);
    await deleteChat(chat.id);
    setBusy(false);
    setDeleteOpen(false);
  };

  return (
    <>
      <DropdownMenu.Root open={menuOpen} onOpenChange={setMenuOpen}>
      <div
        onContextMenu={openContextMenu}
        className={`group flex items-center rounded-md transition-colors duration-[var(--dur-fast)] ${
          active ? "bg-fill-active" : "hover:bg-fill-hover"
        }`}
      >
        <button
          type="button"
          onClick={() => navigate(`/chat/${chat.id}`)}
          className={`flex min-w-0 flex-1 items-center gap-2 overflow-hidden px-3 py-2 text-left text-sm ${
            active ? "text-ink" : "text-ink-secondary"
          }`}
        >
          {chat.status === "running" && (
            <span
              title={t("sidebar.running")}
              className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent"
            />
          )}
          {chat.pinned && (
            <Pin
              size={12}
              className={`shrink-0 text-ink-muted`}
            />
          )}
          <span className="min-w-0 flex-1 truncate">
            {chat.name || t("sidebar.untitled")}
          </span>
        </button>
        <DropdownMenu.Trigger asChild>
          <IconButton
            size="sm"
            title={t("sidebar.chatActions")}
            className="mr-1 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100 data-[state=open]:opacity-100"
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
              setRenameOpen(true);
            }}
          />
          <MenuItem
            icon={chat.pinned ? <PinOff size={14} /> : <Pin size={14} />}
            label={
              chat.pinned ? t("sidebar.unpin") : t("sidebar.pin")
            }
            onSelect={() => void togglePinned(chat.id)}
          />
          <DropdownMenu.Separator className="my-1 h-px bg-line" />
          <DropdownMenu.Item
            onSelect={() => setDeleteOpen(true)}
            className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-danger outline-none hover:bg-danger-soft focus:bg-danger-soft"
          >
            <Trash2 size={14} />
            {t("sidebar.delete")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
      </DropdownMenu.Root>
      <Dialog.Root open={renameOpen} onOpenChange={setRenameOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-ink/25" />
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
        description={t("sidebar.deleteConfirm", {
          name: chat.name || t("sidebar.untitled"),
        })}
        tone="danger"
        busy={busy}
        onOpenChange={setDeleteOpen}
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
