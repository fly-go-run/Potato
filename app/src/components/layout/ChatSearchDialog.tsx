import * as Dialog from "@radix-ui/react-dialog";
import { Pin, Search } from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { useNavigate } from "react-router-dom";
import { filterChats } from "../../lib/chats";
import { useTranslation } from "../../lib/i18n";
import { useChatStore } from "../../stores/chat";

export function ChatSearchDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const chats = useChatStore((state) => state.chats);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const selectedRef = useRef<HTMLButtonElement>(null);
  const results = useMemo(() => filterChats(chats, query), [chats, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query, open]);

  useEffect(() => {
    selectedRef.current?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const chooseChat = (chatId: string) => {
    navigate(`/chat/${chatId}`);
    onOpenChange(false);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing || results.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((current) => (current + 1) % results.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex(
        (current) => (current - 1 + results.length) % results.length,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      const selected = results[selectedIndex];
      if (selected) chooseChat(selected.id);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed left-1/2 top-[18%] z-50 w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 overflow-hidden rounded-lg border border-line bg-raised shadow-raised outline-none">
          <Dialog.Title className="sr-only">{t("search.title")}</Dialog.Title>
          <Dialog.Description className="sr-only">
            {t("search.description")}
          </Dialog.Description>
          <div className="flex items-center gap-2 border-b border-line px-4">
            <Search size={16} className="shrink-0 text-ink-muted" />
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t("search.placeholder")}
              aria-activedescendant={
                results[selectedIndex]
                  ? `chat-search-${results[selectedIndex].id}`
                  : undefined
              }
              className="h-12 min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-ink-muted"
            />
          </div>
          <div className="max-h-80 overflow-y-auto p-2">
            {results.length === 0 ? (
              <div className="px-3 py-8 text-center text-sm text-ink-muted">
                {t("search.empty")}
              </div>
            ) : (
              results.map((chat, index) => {
                const selected = index === selectedIndex;
                return (
                  <button
                    ref={selected ? selectedRef : undefined}
                    id={`chat-search-${chat.id}`}
                    key={chat.id}
                    type="button"
                    onMouseMove={() => setSelectedIndex(index)}
                    onClick={() => chooseChat(chat.id)}
                    className={`flex w-full min-w-0 items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                      selected
                        ? "bg-accent-soft text-accent"
                        : "text-ink-secondary hover:bg-line/50"
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
                );
              })
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
