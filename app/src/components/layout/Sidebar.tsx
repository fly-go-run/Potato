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
          className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-line/50"
        >
          <PenSquare size={16} className="text-ink-secondary" />
          <span className="flex-1">{t("sidebar.newChat")}</span>
          <kbd className="text-[11px] text-ink-muted">
            {shortcutLabel("N")}
          </kbd>
        </button>
        <NavLink
          to="/crons"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              isActive
                ? "bg-accent-soft text-accent"
                : "text-ink hover:bg-line/50"
            }`
          }
        >
          <Clock3 size={16} className="text-ink-secondary" />
          <span className="flex-1">{t("sidebar.crons")}</span>
        </NavLink>
        <NavLink
          to="/inbox"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              isActive
                ? "bg-accent-soft text-accent"
                : "text-ink hover:bg-line/50"
            }`
          }
        >
          <Inbox size={16} className="text-ink-secondary" />
          <span className="flex-1">{t("sidebar.inbox")}</span>
          {unreadCount > 0 && (
            <span className="min-w-5 rounded-full bg-accent px-1.5 py-0.5 text-center text-[10px] font-medium leading-none text-surface">
              {unreadCount > 99 ? "99+" : unreadCount}
            </span>
          )}
        </NavLink>
        <NavLink
          to="/skills"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              isActive
                ? "bg-accent-soft text-accent"
                : "text-ink hover:bg-line/50"
            }`
          }
        >
          <Blocks size={16} className="text-ink-secondary" />
          <span className="flex-1">{t("sidebar.skills")}</span>
        </NavLink>
        <NavLink
          to="/memory"
          className={({ isActive }) =>
            `mt-0.5 flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              isActive
                ? "bg-accent-soft text-accent"
                : "text-ink hover:bg-line/50"
            }`
          }
        >
          <NotebookPen size={16} className="text-ink-secondary" />
          <span className="flex-1">{t("sidebar.memory")}</span>
        </NavLink>
        <button
          type="button"
          onClick={onSearch}
          className="mt-0.5 flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-line/50"
        >
          <Search size={16} className="text-ink-secondary" />
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
          <div className="px-3 py-2 text-sm text-ink-muted">
            {t("sidebar.loading")}
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
            `flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              isActive
                ? "bg-accent-soft text-accent"
                : "text-ink-secondary hover:bg-line/50"
            }`
          }
        >
          <Settings size={16} />
          {t("sidebar.settings")}
        </NavLink>
      </div>
    </aside>
  );
}

function ChatRow({ chat, active }: { chat: ChatSpec; active: boolean }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = useState(false);
  const { renameChat, togglePinned, deleteChat } = useChatStore();

  const openContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    setMenuOpen(true);
  };

  return (
    <DropdownMenu.Root open={menuOpen} onOpenChange={setMenuOpen}>
      <div
        onContextMenu={openContextMenu}
        className={`group flex items-center rounded-md transition-colors ${
          active ? "bg-accent-soft" : "hover:bg-line/50"
        }`}
      >
        <button
          type="button"
          onClick={() => navigate(`/chat/${chat.id}`)}
          className={`flex min-w-0 flex-1 items-center gap-2 overflow-hidden px-3 py-2 text-left text-sm ${
            active ? "text-accent" : "text-ink-secondary"
          }`}
        >
          {chat.status === "running" && (
            <span
              title={t("sidebar.running")}
              className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent"
            />
          )}
          {chat.pinned && (
            <Pin size={12} className="shrink-0 text-ink-muted" />
          )}
          <span className="min-w-0 flex-1 truncate">
            {chat.name || t("sidebar.untitled")}
          </span>
        </button>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            title={t("sidebar.chatActions")}
            className="mr-1 rounded-sm p-1 text-ink-muted opacity-0 outline-none transition-opacity hover:bg-line group-hover:opacity-100 focus:opacity-100 data-[state=open]:opacity-100"
          >
            <MoreHorizontal size={15} />
          </button>
        </DropdownMenu.Trigger>
      </div>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={4}
          className="z-50 min-w-32 rounded-md border border-line bg-raised p-1 shadow-raised"
        >
          <MenuItem
            icon={<PenLine size={14} />}
            label={t("sidebar.rename")}
            onSelect={() => {
              const name = window.prompt(t("sidebar.renamePrompt"), chat.name);
              if (name) void renameChat(chat.id, name);
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
            onSelect={() => {
              if (
                window.confirm(
                  t("sidebar.deleteConfirm", {
                    name: chat.name || t("sidebar.untitled"),
                  }),
                )
              ) {
                void deleteChat(chat.id);
              }
            }}
            className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-danger outline-none hover:bg-danger-soft focus:bg-danger-soft"
          >
            <Trash2 size={14} />
            {t("sidebar.delete")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
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
      className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-line/50 focus:bg-line/50"
    >
      {icon}
      {label}
    </DropdownMenu.Item>
  );
}
