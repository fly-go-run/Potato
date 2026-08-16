import { lazy, Suspense, useEffect, useState, type MouseEvent } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import {
  isMacDesktopShell,
  notifyDesktopReady,
  startDesktopWindowDrag,
} from "../../lib/desktop";
import { isPrimaryShortcut } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useUiStore } from "../../stores/ui";
import { CollapsedRail } from "./CollapsedRail";
import { Sidebar } from "./Sidebar";

const ChatSearchDialog = lazy(() =>
  import("./ChatSearchDialog").then((module) => ({
    default: module.ChatSearchDialog,
  })),
);
const ShortcutsDialog = lazy(() =>
  import("./ShortcutsDialog").then((module) => ({
    default: module.ShortcutsDialog,
  })),
);

/** 顶层两栏：左侧会话栏 + 右侧主区（聊天 / 设置）。 */
export function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [searchOpen, setSearchOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const initialize = useChatStore((state) => state.initialize);
  const newChat = useChatStore((state) => state.newChat);
  const sidebarCollapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);

  useEffect(() => {
    void notifyDesktopReady();
    void initialize();
  }, [initialize]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.altKey || !isPrimaryShortcut(event)) {
        return;
      }
      const key = event.key.toLocaleLowerCase();
      if (key === "n" && !event.shiftKey) {
        event.preventDefault();
        setSearchOpen(false);
        setShortcutsOpen(false);
        newChat();
        navigate("/");
      } else if (key === "k" && !event.shiftKey) {
        event.preventDefault();
        setShortcutsOpen(false);
        setSearchOpen(true);
      } else if (key === "b" && !event.shiftKey) {
        event.preventDefault();
        toggleSidebar();
      } else if (key === "/" && !event.shiftKey) {
        event.preventDefault();
        setSearchOpen(false);
        setShortcutsOpen(true);
      } else if (key === "," && !event.shiftKey) {
        // ⌘, 开/关设置,与桌面应用惯例一致。
        // 注意:设置以 background-location 覆盖层渲染时,AppShell 位于
        // <Routes location={background}> 子树,useLocation 拿到的是背景页;
        // 判断"当前是否在设置"必须看真实 hash。
        event.preventDefault();
        setSearchOpen(false);
        setShortcutsOpen(false);
        const onSettings =
          window.location.hash.replace(/^#/, "").split("?")[0] === "/settings";
        if (onSettings) {
          // 与设置面板 closePanel 同一逻辑:优先退回来路。
          const state = window.history.state as { idx?: number } | null;
          if (typeof state?.idx === "number" && state.idx > 0) navigate(-1);
          else navigate("/");
        } else {
          navigate("/settings", { state: { background: location } });
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [location, navigate, newChat, toggleSidebar]);

  const onOverlayMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button === 0) startDesktopWindowDrag();
  };

  const mac = isMacDesktopShell();

  return (
    // min-h-0 + overflow-hidden: keep the shell viewport-bound so only
    // nested scroll regions (chat messages, sidebar list, page bodies)
    // move — never the chrome (top bar / side rail) with the document.
    <div className="relative flex h-full min-h-0 overflow-hidden">
      {/* macOS overlay：红绿灯和左上按钮同一行；主列顶上只留透明拖区。 */}
      {mac && (
        <div
          data-tauri-drag-region
          onMouseDown={onOverlayMouseDown}
          className={`absolute right-0 top-0 z-20 h-11 ${
            sidebarCollapsed ? "left-48" : "left-[16.5rem]"
          }`}
        />
      )}
      {sidebarCollapsed ? (
        <CollapsedRail />
      ) : (
        <Sidebar onSearch={() => setSearchOpen(true)} />
      )}
      <main className="min-h-0 min-w-0 flex-1 overflow-hidden bg-canvas">
        <Outlet />
      </main>
      <Suspense fallback={null}>
        <ChatSearchDialog open={searchOpen} onOpenChange={setSearchOpen} />
        <ShortcutsDialog open={shortcutsOpen} onOpenChange={setShortcutsOpen} />
      </Suspense>
    </div>
  );
}
