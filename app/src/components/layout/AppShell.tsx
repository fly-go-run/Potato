import { MessageCirclePlus, PanelLeft } from "lucide-react";
import { lazy, Suspense, useEffect, useState, type MouseEvent } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { useTranslation } from "../../lib/i18n";
import {
  isMacDesktopShell,
  notifyDesktopReady,
  startDesktopWindowDrag,
} from "../../lib/desktop";
import { isPrimaryShortcut, shortcutLabel } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useInboxStore } from "../../stores/inbox";
import { useUiStore } from "../../stores/ui";
import { IconButton } from "../ui";
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
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const [searchOpen, setSearchOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const initialize = useChatStore((state) => state.initialize);
  const newChat = useChatStore((state) => state.newChat);
  const refreshUnread = useInboxStore((state) => state.refreshUnread);
  const sidebarCollapsed = useUiStore((state) => state.sidebarCollapsed);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);

  useEffect(() => {
    let active = true;
    const startup = Promise.allSettled([initialize(), refreshUnread()]);
    // 首屏数据通常会在这段时间内就绪；但任何接口悬挂都不能让原生窗口
    // 永久隐藏。超时后先展示可交互壳层，数据仍在后台继续加载。
    const revealTimer = window.setTimeout(() => {
      if (active) void notifyDesktopReady();
    }, 1500);
    void startup.finally(() => {
      window.clearTimeout(revealTimer);
      if (!active) return;
      window.requestAnimationFrame(() => {
        if (active) void notifyDesktopReady();
      });
    });
    return () => {
      active = false;
      window.clearTimeout(revealTimer);
    };
  }, [initialize, refreshUnread]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.altKey ||
        !isPrimaryShortcut(event)
      ) {
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
          window.location.hash.replace(/^#/, "").split("?")[0] ===
          "/settings";
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

  const startNewChat = () => {
    newChat();
    navigate("/");
  };

  const onOverlayMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button === 0) startDesktopWindowDrag();
  };

  return (
    <div className="relative flex h-full">
      {/* macOS overlay 标题栏：只覆盖没有控件的区域，避免透明拖拽层
          截走左上角按钮的点击。侧栏展开时，左侧由 Sidebar 自己负责拖拽。 */}
      {isMacDesktopShell() && (
        <div
          data-tauri-drag-region
          onMouseDown={onOverlayMouseDown}
          className={`absolute right-0 top-0 z-20 h-11 ${
            sidebarCollapsed ? "left-36" : "left-[16.5rem]"
          }`}
        />
      )}
      {!sidebarCollapsed && <Sidebar onSearch={() => setSearchOpen(true)} />}
      {sidebarCollapsed && (
        // 收起态：主区满宽，左上角保留侧栏切换 + 新建会话两个裸图标。
        // macOS 壳下与红绿灯同处 44px 标题栏，并给其右侧留出拖拽空间。
        <div
          className={`absolute z-40 flex items-center gap-3 ${
            isMacDesktopShell() ? "left-[5.5rem] top-2" : "left-3 top-3.5"
          }`}
        >
          <IconButton
            size="sm"
            title={`${t("sidebar.expand")} · ${shortcutLabel("B")}`}
            aria-label={t("sidebar.expand")}
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
      )}
      <main className="min-w-0 flex-1 bg-canvas">
        <Outlet />
      </main>
      <Suspense fallback={null}>
        <ChatSearchDialog open={searchOpen} onOpenChange={setSearchOpen} />
        <ShortcutsDialog
          open={shortcutsOpen}
          onOpenChange={setShortcutsOpen}
        />
      </Suspense>
    </div>
  );
}
