import { lazy, Suspense, useEffect, useState } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import { isPrimaryShortcut } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useInboxStore } from "../../stores/inbox";
import { Sidebar } from "./Sidebar";

const ChatSearchDialog = lazy(() =>
  import("./ChatSearchDialog").then((module) => ({
    default: module.ChatSearchDialog,
  })),
);

/** 顶层两栏：左侧会话栏 + 右侧主区（聊天 / 设置）。 */
export function AppShell() {
  const navigate = useNavigate();
  const [searchOpen, setSearchOpen] = useState(false);
  const initialize = useChatStore((state) => state.initialize);
  const newChat = useChatStore((state) => state.newChat);
  const refreshUnread = useInboxStore((state) => state.refreshUnread);

  useEffect(() => {
    void initialize();
    void refreshUnread();
  }, [initialize, refreshUnread]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.altKey ||
        event.shiftKey ||
        !isPrimaryShortcut(event)
      ) {
        return;
      }
      const key = event.key.toLocaleLowerCase();
      if (key === "n") {
        event.preventDefault();
        setSearchOpen(false);
        newChat();
        navigate("/");
      } else if (key === "k") {
        event.preventDefault();
        setSearchOpen(true);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigate, newChat]);

  return (
    <div className="flex h-full">
      <Sidebar onSearch={() => setSearchOpen(true)} />
      <main className="min-w-0 flex-1 bg-surface">
        <Outlet />
      </main>
      <Suspense fallback={null}>
        <ChatSearchDialog open={searchOpen} onOpenChange={setSearchOpen} />
      </Suspense>
    </div>
  );
}
