import { PanelLeft, SquarePen } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import type { MouseEvent } from "react";
import { isMacDesktopShell, startDesktopWindowDrag } from "../../lib/desktop";
import { useDesktopFullscreen } from "../../lib/useDesktopFullscreen";
import { useTranslation } from "../../lib/i18n";
import { shortcutLabel } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useUiStore } from "../../stores/ui";

/** 顶栏图标：静息无底，hover 才起一块圆角。 */
export const chromeIconClass =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-md)] text-icon transition-colors duration-[150ms] ease-out hover:bg-fill-hover hover:text-icon-strong active:bg-fill-active";

/** 侧栏收起时贴在左上：展开 + 新建。展开态的新建走侧栏导航行。 */
export function ChromeActions({
  sidebarCollapsed,
}: {
  sidebarCollapsed: boolean;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const newChat = useChatStore((state) => state.newChat);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);

  return (
    <>
      <button
        type="button"
        title={`${
          sidebarCollapsed ? t("sidebar.expand") : t("sidebar.collapse")
        } · ${shortcutLabel("B")}`}
        aria-label={
          sidebarCollapsed ? t("sidebar.expand") : t("sidebar.collapse")
        }
        onClick={toggleSidebar}
        className={chromeIconClass}
      >
        <PanelLeft size={16} strokeWidth={1.75} />
      </button>
      <button
        type="button"
        title={`${t("sidebar.newChat")} · ${shortcutLabel("N")}`}
        aria-label={t("sidebar.newChat")}
        onClick={() => {
          newChat();
          navigate("/");
        }}
        className={chromeIconClass}
      >
        <SquarePen size={16} strokeWidth={1.75} />
      </button>
    </>
  );
}

/**
 * 侧栏收起：侧栏开关/新建两钮贴在左上角顶栏，和红绿灯同一行。
 * 会话路由下按钮右侧补当前会话名，避免收起后丢标题。
 */
export function CollapsedRail() {
  const location = useLocation();
  const chats = useChatStore((state) => state.chats);
  const activeChatId = useChatStore((state) => state.activeChatId);
  const mac = isMacDesktopShell();
  const fullscreen = useDesktopFullscreen();
  // 窗口态给灯簇让位；全屏灯隐藏后与网页/Windows 同左缘。
  const insetForTrafficLights = mac && !fullscreen;
  const isChatRoute =
    location.pathname === "/" || location.pathname.startsWith("/chat/");
  const activeChat = activeChatId
    ? chats.find((chat) => chat.id === activeChatId)
    : undefined;
  // 标题生成之前一律空着:会话没建立、或名字还没起,都不显示占位文案
  const title = activeChat?.name || null;

  const onTitlebarMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (!mac || event.button !== 0) return;
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
    <div
      data-tauri-drag-region={mac || undefined}
      onMouseDown={onTitlebarMouseDown}
      // mac 窗口：灯簇右缘约 70px，5.75rem 再空约 22px，避免贴绿灯。
      className={`absolute left-0 top-0 z-40 flex h-11 items-center gap-0.5 pr-2 transition-[padding-left] duration-200 ${
        insetForTrafficLights ? "pl-[5.75rem]" : "pl-3"
      }`}
    >
      <ChromeActions sidebarCollapsed />
      {isChatRoute && title && (
        <span
          data-tauri-drag-region={mac || undefined}
          className="ml-1.5 min-w-0 max-w-[16rem] truncate text-[13px] font-medium text-ink"
        >
          {title}
        </span>
      )}
    </div>
  );
}
