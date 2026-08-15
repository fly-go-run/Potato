import * as Dialog from "@radix-ui/react-dialog";
import {
  LayoutGrid,
  Clock3,
  MessageSquare,
  Notebook,
  SquarePen,
  Pin,
  Search,
  Settings,
  type LucideIcon,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { useLocation, useNavigate } from "react-router-dom";
import type { ChatSpec } from "../../lib/api";
import { filterChats } from "../../lib/chats";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import { loadSessionProject } from "../../lib/projects";
import { relativeTime } from "../../lib/relativeTime";
import { useChatStore } from "../../stores/chat";

interface PaletteItem {
  id: string;
  label: string;
  icon: LucideIcon;
  execute: () => void;
  chat?: ChatSpec;
}

interface PaletteSection {
  id: string;
  title: TranslationKey;
  items: PaletteItem[];
}

export function ChatSearchDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const chats = useChatStore((state) => state.chats);
  const newChat = useChatStore((state) => state.newChat);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const selectedRef = useRef<HTMLButtonElement>(null);

  const sections = useMemo<PaletteSection[]>(() => {
    const normalized = query.trim().toLocaleLowerCase();
    const matches = (label: string) =>
      !normalized || label.toLocaleLowerCase().includes(normalized);
    const closeAndNavigate = (path: string) => {
      navigate(path);
      onOpenChange(false);
    };

    const action: PaletteItem = {
      id: "action-new-chat",
      label: t("sidebar.newChat"),
      icon: SquarePen,
      execute: () => {
        newChat();
        closeAndNavigate("/");
      },
    };
    const pages: PaletteItem[] = [
      {
        id: "page-crons",
        label: t("sidebar.crons"),
        icon: Clock3,
        execute: () => closeAndNavigate("/crons"),
      },
      {
        id: "page-skills",
        label: t("sidebar.skills"),
        icon: LayoutGrid,
        execute: () => closeAndNavigate("/skills"),
      },
      {
        id: "page-memory",
        label: t("sidebar.memory"),
        icon: Notebook,
        execute: () => closeAndNavigate("/memory"),
      },
      {
        id: "page-settings",
        label: t("sidebar.settings"),
        icon: Settings,
        execute: () => {
          navigate("/settings", { state: { background: location } });
          onOpenChange(false);
        },
      },
    ];
    const chatResults = normalized
      ? filterChats(chats, query)
      : chats.slice(0, 5);
    const chatItems = chatResults.map<PaletteItem>((chat) => ({
      id: `chat-${chat.id}`,
      label: chat.name || t("sidebar.untitled"),
      icon: MessageSquare,
      chat,
      execute: () => closeAndNavigate(`/chat/${chat.id}`),
    }));

    return [
      {
        id: "actions",
        title: "command.actions",
        items: matches(action.label) ? [action] : [],
      },
      {
        id: "pages",
        title: "command.pages",
        items: pages.filter((page) => matches(page.label)),
      },
      {
        id: "chats",
        title: "command.chats",
        items: chatItems,
      },
    ];
  }, [chats, location, navigate, newChat, onOpenChange, query, t]);

  const items = sections.flatMap((section) => section.items);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSelectedIndex(0);
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    setSelectedIndex((current) =>
      items.length === 0 ? 0 : Math.min(current, items.length - 1),
    );
  }, [items.length]);

  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing || items.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((current) => (current + 1) % items.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex(
        (current) => (current - 1 + items.length) % items.length,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      items[selectedIndex]?.execute();
    }
  };

  let itemIndex = -1;

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-[18%] z-50 w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 overflow-hidden rounded-[var(--radius-lg)] border border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <Dialog.Title className="sr-only">{t("search.title")}</Dialog.Title>
          <Dialog.Description className="sr-only">
            {t("search.description")}
          </Dialog.Description>
          <div className="flex items-center gap-2 border-b border-line px-4">
            <Search size={16} strokeWidth={1.75} className="shrink-0 text-icon" />
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t("search.placeholder")}
              aria-activedescendant={
                items[selectedIndex]
                  ? `command-palette-${items[selectedIndex].id}`
                  : undefined
              }
              className="h-12 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-muted"
            />
          </div>
          <div className="max-h-80 overflow-y-auto p-2">
            {items.length === 0 ? (
              <div className="px-3 py-8 text-center text-sm text-ink-tertiary">
                {t("search.empty")}
              </div>
            ) : (
              sections.map((section) => {
                if (section.items.length === 0) return null;
                return (
                  <section key={section.id}>
                    <h2 className="px-3 pb-1 pt-2 text-[11px] text-ink-tertiary">
                      {t(section.title)}
                    </h2>
                    {section.items.map((item) => {
                      itemIndex += 1;
                      const index = itemIndex;
                      const selected = index === selectedIndex;
                      const Icon = item.icon;
                      const project = item.chat
                        ? loadSessionProject(item.chat.session_id)
                        : null;
                      const updatedAt = item.chat
                        ? relativeTime(item.chat.updated_at)
                        : null;
                      return (
                        <button
                          ref={selected ? selectedRef : undefined}
                          id={`command-palette-${item.id}`}
                          key={item.id}
                          type="button"
                          onClick={item.execute}
                          onMouseEnter={() => setSelectedIndex(index)}
                          className={`flex w-full min-w-0 items-center gap-2.5 rounded-md px-3 py-2 text-left text-sm transition-colors duration-[var(--dur-fast)] ${
                            selected
                              ? "bg-fill-active text-ink"
                              : "text-ink-secondary hover:bg-fill-hover"
                          }`}
                        >
                          <Icon size={14} strokeWidth={1.8} className="shrink-0 text-icon" />
                          {item.chat?.pinned && (
                            <Pin
                              size={12}
                              strokeWidth={1.8}
                              className="shrink-0 text-icon"
                            />
                          )}
                          <span className="min-w-0 flex-1">
                            <span className="block truncate">{item.label}</span>
                            {item.chat && (
                              <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] font-normal text-ink-tertiary">
                                {project && (
                                  <span className="min-w-0 truncate">
                                    {project.name}
                                  </span>
                                )}
                                {project && updatedAt && <span>·</span>}
                                {updatedAt && (
                                  <span className="shrink-0">
                                    {t(updatedAt.key, updatedAt.params)}
                                  </span>
                                )}
                              </span>
                            )}
                          </span>
                        </button>
                      );
                    })}
                  </section>
                );
              })
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
