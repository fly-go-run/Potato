import { PanelLeft, Search, SquarePen } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import type { MouseEvent } from "react";
import { isMacDesktopShell, startDesktopWindowDrag } from "../../lib/desktop";
import { useTranslation } from "../../lib/i18n";
import { shortcutLabel } from "../../lib/shortcuts";
import { useChatStore } from "../../stores/chat";
import { useUiStore } from "../../stores/ui";

/** 顶栏图标：静息无底，hover 才起一块圆角。 */
export const chromeIconClass =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-md)] text-icon transition-colors duration-[150ms] ease-out hover:bg-fill-hover hover:text-icon-strong active:bg-fill-active";

/** 对标 ChatGPT 桌面端：红绿灯右侧并排侧栏 / 新建 / 搜索。 */
export function ChromeActions({
  sidebarCollapsed,
  onSearch,
}: {
  sidebarCollapsed: boolean;
  onSearch: () => void;
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
      <button
        type="button"
        title={`${t("sidebar.searchChats")} · ${shortcutLabel("K")}`}
        aria-label={t("sidebar.searchChats")}
        onClick={onSearch}
        className={chromeIconClass}
      >
        <Search size={16} strokeWidth={1.75} />
      </button>
    </>
  );
}

/**
 * 侧栏收起：三个按钮贴在左上角顶栏，和红绿灯同一行。
 * 会话路由下按钮右侧补当前会话名，避免收起后丢标题。
 */
export function CollapsedRail({ onSearch }: { onSearch: () => void }) {
  const location = useLocation();
  const chats = useChatStore((state) => state.chats);
  const activeChatId = useChatStore((state) => state.activeChatId);
  const mac = isMacDesktopShell();
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
      className={`absolute left-0 top-0 z-40 flex h-11 items-center gap-0.5 ${
        mac ? "pl-[4.75rem] pr-2" : "pl-3 pr-2"
      }`}
    >
      <ChromeActions sidebarCollapsed onSearch={onSearch} />
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
