import {
  ArrowDown,
  CloudUpload,
  FolderClosed,
  PanelRightOpen,
  Search,
  X,
} from "lucide-react";
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Composer } from "../components/chat/Composer";
import { collectConversationArtifacts } from "../lib/conversationArtifacts";
import { collectFileChanges } from "../lib/fileChanges";
import { textFromContent } from "../lib/content";
import { MessageList } from "../components/chat/MessageList";
import { Banner } from "../components/ui/Banner";
import { Button, Card, SkeletonRows } from "../components/ui";
import { getChatBanner } from "../lib/chatBanner";
import { isMacDesktopShell, startDesktopWindowDrag } from "../lib/desktop";
import { useTranslation, type TranslationKey } from "../lib/i18n";
import { BOTTOM_THRESHOLD_PX } from "../lib/scroll";
import type { StreamMessage } from "../lib/stream";
import { shortcutLabel } from "../lib/shortcuts";
import { useChatStore } from "../stores/chat";
import { useUiStore } from "../stores/ui";

const ConversationSidePanel = lazy(() =>
  import("../components/chat/ConversationSidePanel").then((module) => ({
    default: module.ConversationSidePanel,
  })),
);


/** 时段问候:口号退役,首页对人不对市场说话。 */
function timeGreeting(t: (key: TranslationKey) => string): string {
  const hour = new Date().getHours();
  if (hour >= 5 && hour < 12) return t("chat.greeting.morning");
  if (hour >= 12 && hour < 18) return t("chat.greeting.afternoon");
  return t("chat.greeting.evening");
}


export function ChatView() {
  const { t } = useTranslation();
  const { chatId } = useParams();
  const routerNavigate = useNavigate();
  const sidebarCollapsed = useUiStore((state) => state.sidebarCollapsed);
  const scrollRef = useRef<HTMLDivElement>(null);
  /** 已锚定的最新用户消息 id;null = 本会话还没做过首次填充。 */
  const anchoredUserIdRef = useRef<string | null>(null);
  const atBottomRef = useRef(true);
  const [atBottom, setAtBottom] = useState(true);
  const [hasNewContent, setHasNewContent] = useState(false);
  const [switchingModel, setSwitchingModel] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeSearchMessageId, setActiveSearchMessageId] = useState("");
  const [sidePanelOpen, setSidePanelOpen] = useState(false);
  const [selectedFilePath, setSelectedFilePath] = useState("");
  const [selectedChangePath, setSelectedChangePath] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const activeChatId = useChatStore((state) => state.activeChatId);
  const messages = useChatStore((state) => state.stream.messages);
  const responseStatus = useChatStore((state) => state.stream.responseStatus);
  const rateLimited = useChatStore((state) => state.stream.rateLimited);
  const historyLoading = useChatStore((state) => state.historyLoading);
  const error = useChatStore((state) => state.error);
  const isStreaming = useChatStore((state) => state.isStreaming);
  const sessionId = useChatStore((state) => state.sessionId);
  const pendingApprovals = useChatStore((state) => state.pendingApprovals);
  const openChat = useChatStore((state) => state.openChat);
  const pollApprovals = useChatStore((state) => state.pollApprovals);
  const switchRateLimitedModel = useChatStore(
    (state) => state.switchRateLimitedModel,
  );
  const sendMessage = useChatStore((state) => state.sendMessage);
  const clearError = useChatStore((state) => state.clearError);
  const addImages = useChatStore((state) => state.addImages);
  const chats = useChatStore((state) => state.chats);
  const activeProject = useChatStore((state) => state.project);
  const activeChatName =
    chats.find((chat) => chat.id === activeChatId)?.name ?? "";
  const [dragging, setDragging] = useState(false);
  const dragDepth = useRef(0);
  const artifacts = useMemo(
    () => collectConversationArtifacts(messages),
    [messages],
  );
  // 限流后"切换模型"要能接着把失败的那次请求补回去,
  // 否则用户以为已恢复,实际任务还停在原地。
  const lastUserText = useMemo(() => {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const message = messages[index]!;
      if (message.role === "user" && message.type === "message") {
        const text = textFromContent(message.content).trim();
        if (text) return text;
      }
    }
    return "";
  }, [messages]);
  const fileChanges = useMemo(() => collectFileChanges(messages), [messages]);
  const searchMatches = useMemo(() => {
    const query = searchQuery.trim().toLocaleLowerCase();
    if (!query) return [];
    return messages
      .filter(
        (message) =>
          (message.role === "user" || message.role === "assistant") &&
          (message.type === "message" || message.type === "result"),
      )
      .map((message) => ({
        id: message.id,
        role: message.role,
        text: textFromContent(message.content).trim(),
      }))
      .filter((message) => message.text.toLocaleLowerCase().includes(query));
  }, [searchQuery, messages]);

  const onDragEnter = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) return;
    event.preventDefault();
    dragDepth.current += 1;
    setDragging(true);
  };
  const onDragLeave = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) return;
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) setDragging(false);
  };
  const onDragOver = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) return;
    event.preventDefault();
  };
  const onDrop = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) return;
    event.preventDefault();
    dragDepth.current = 0;
    setDragging(false);
    const files = Array.from(event.dataTransfer.files);
    if (files.length) addImages(files);
  };
  const banner = getChatBanner(error, rateLimited);
  const bannerMessage =
    banner && /^internal server error$/i.test(banner.message.trim())
      ? t("chat.error.serviceUnavailable")
      : banner?.message;
  const isEmpty = messages.length === 0 && pendingApprovals.length === 0;

  useEffect(() => {
    if (chatId && chatId !== activeChatId) void openChat(chatId);
  }, [activeChatId, chatId, openChat]);

  /**
   * 「底部」一律按**内容末尾**计,不按 scrollHeight:尾轮为问题锚顶
   * 预留的 TAIL_MIN_HEIGHT 空白不算内容,否则「回到底部」会把人送进
   * 一片空白,「有新内容」指示也会在全部内容都可见时还亮着。
   * 逐轮取子元素的实际 bottom(轮容器自身带 min-height,不可信)。
   */
  const contentBottomOf = (element: HTMLElement): number => {
    const wrapper = element.querySelector("[data-chat-content]");
    if (!wrapper) return element.scrollHeight;
    const containerTop = element.getBoundingClientRect().top;
    let bottom = 0;
    for (const block of wrapper.children) {
      const parts = block.children.length > 0 ? block.children : [block];
      for (const part of parts) {
        const rect = part.getBoundingClientRect();
        if (rect.bottom > bottom) bottom = rect.bottom;
      }
    }
    if (bottom === 0) return element.scrollHeight;
    return element.scrollTop + (bottom - containerTop);
  };
  const distanceToContentBottom = (element: HTMLElement): number =>
    contentBottomOf(element) - (element.scrollTop + element.clientHeight);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const handleScroll = () => {
      const nextAtBottom =
        distanceToContentBottom(element) <= BOTTOM_THRESHOLD_PX;
      atBottomRef.current = nextAtBottom;
      setAtBottom(nextAtBottom);
      if (nextAtBottom) setHasNewContent(false);
    };
    element.addEventListener("scroll", handleScroll, { passive: true });
    return () => element.removeEventListener("scroll", handleScroll);
  }, [historyLoading, isEmpty]);

  /* 滚动模型:问题锚顶,不跟随。
   *
   * 发送新问题时把它滚到视口顶部,答案在其下方向下生长;流式期间视口
   * 一动不动——回答结束时用户看到的是答案的**开头**,顺着读下去,而不是
   * 被拖到结尾再往回找。旧的"每帧钉在底部"会把上方所有已读内容顶得
   * 不停位移,是抖动感的最大放大器。
   * 打开历史会话仍一次性落底(最近内容在底部是历史浏览的常识)。 */
  useEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    let lastUser: StreamMessage | undefined;
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      if (messages[index]!.role === "user") {
        lastUser = messages[index];
        break;
      }
    }
    const anchorToQuestion = (userId: string, smooth: boolean) => {
      window.requestAnimationFrame(() => {
        const target = document.getElementById(`message-${userId}`);
        if (!target || !scrollRef.current) return;
        const container = scrollRef.current;
        // 32px 呼吸:问题贴着标题栏会显得局促,留出一行余量。
        const offset =
          container.scrollTop +
          target.getBoundingClientRect().top -
          container.getBoundingClientRect().top -
          32;
        container.scrollTo({
          top: Math.max(0, offset),
          behavior: smooth ? "smooth" : "auto",
        });
      });
    };
    // 首次填充:打开历史会话落底;但新会话首帧(正在流式)必须锚顶——
    // 此时底部是 TAIL_MIN_HEIGHT 预留的锚定空间,落底会把问题推出视口。
    if (anchoredUserIdRef.current === null) {
      if (messages.length === 0) return;
      anchoredUserIdRef.current = lastUser?.id ?? "";
      if (isStreaming && lastUser) {
        anchorToQuestion(lastUser.id, false);
      } else {
        element.scrollTop = element.scrollHeight;
      }
      return;
    }
    // 新问题出现:锚到视口顶部(留 12px 呼吸),此后不再自动滚动。
    if (lastUser && lastUser.id !== anchoredUserIdRef.current) {
      anchoredUserIdRef.current = lastUser.id;
      anchorToQuestion(lastUser.id, true);
      return;
    }
    // 内容生长:视口不动,只维护「下方有新内容」指示。
    const nowAtBottom = distanceToContentBottom(element) <= BOTTOM_THRESHOLD_PX;
    atBottomRef.current = nowAtBottom;
    setAtBottom(nowAtBottom);
    if (!nowAtBottom) setHasNewContent(true);
  }, [pendingApprovals, messages, isStreaming]);

  useEffect(() => {
    atBottomRef.current = true;
    setAtBottom(true);
    setHasNewContent(false);
    // 换会话重置锚定基线,下一次消息填充按「首次填充」处理。
    anchoredUserIdRef.current = null;
    setSearchOpen(false);
    setSearchQuery("");
    setActiveSearchMessageId("");
    setSidePanelOpen(false);
    setSelectedFilePath("");
    setSelectedChangePath("");
  }, [chatId]);

  useEffect(() => {
    if (!searchOpen) return;
    window.requestAnimationFrame(() => searchInputRef.current?.focus());
  }, [searchOpen]);

  useEffect(() => {
    setActiveSearchMessageId(searchMatches[0]?.id ?? "");
  }, [searchMatches]);

  useEffect(() => {
    if (!isStreaming || !sessionId) return;
    const controller = new AbortController();
    let polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        await pollApprovals(controller.signal);
      } finally {
        polling = false;
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), 2500);
    return () => {
      controller.abort();
      window.clearInterval(interval);
    };
  }, [isStreaming, pollApprovals, sessionId]);

  const scrollToBottom = () => {
    const element = scrollRef.current;
    if (element) {
      // 让最后一条内容的末尾贴近输入框(留 16px),而不是滚到预留空白的尽头。
      element.scrollTop = Math.max(
        0,
        contentBottomOf(element) - element.clientHeight + 16,
      );
    }
    atBottomRef.current = true;
    setAtBottom(true);
    setHasNewContent(false);
  };

  const showBackToBottom = !atBottom && (isStreaming || hasNewContent);

  const openFilePreview = useCallback((path: string) => {
    if (!path) return;
    setSelectedChangePath("");
    setSelectedFilePath(path);
    setSidePanelOpen(true);
  }, []);

  const openChangeDiff = useCallback((path: string) => {
    if (!path) return;
    setSelectedFilePath("");
    setSelectedChangePath(path);
    setSidePanelOpen(true);
  }, []);

  const backToPanelHome = () => {
    setSelectedFilePath("");
    setSelectedChangePath("");
  };

  const closeSidePanel = () => {
    backToPanelHome();
    setSidePanelOpen(false);
  };

  const onTitlebarMouseDown = (event: ReactMouseEvent<HTMLElement>) => {
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

  const locateMessage = (messageId: string) => {
    const element = document.getElementById(`message-${messageId}`);
    if (!element) return;
    setActiveSearchMessageId(messageId);
    element.scrollIntoView({ block: "center", behavior: "smooth" });
  };

  const cycleSearch = (direction: 1 | -1) => {
    if (searchMatches.length === 0) return;
    const current = searchMatches.findIndex(
      (match) => match.id === activeSearchMessageId,
    );
    const next =
      (Math.max(current, 0) + direction + searchMatches.length) %
      searchMatches.length;
    const match = searchMatches[next];
    if (match) locateMessage(match.id);
  };

  const handleSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      setSearchOpen(false);
    } else if (event.key === "Enter") {
      event.preventDefault();
      cycleSearch(event.shiftKey ? -1 : 1);
    }
  };

  return (
    <div
      className="relative flex h-full min-h-0 flex-col overflow-hidden"
      onDragEnter={onDragEnter}
      onDragLeave={onDragLeave}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      {dragging && (
        <div className="pointer-events-none absolute inset-3 z-50 flex flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed border-accent bg-accent-soft/60">
          <CloudUpload size={28} className="text-accent" />
          <span className="text-sm font-medium text-accent">
            {t("chat.dropFiles")}
          </span>
        </div>
      )}
      {banner && (
        <div className="pointer-events-none absolute inset-x-0 top-0 z-40">
          <div className="pointer-events-auto">
            <Banner
              tone={banner.tone}
              onDismiss={clearError}
              actions={
                banner.alternatives.length > 0
                  ? banner.alternatives.map((alternative) => {
                      const key = `${alternative.provider_id}/${alternative.model_id}`;
                      return (
                        <Button
                          key={key}
                          variant="secondary"
                          size="sm"
                          disabled={switchingModel !== null}
                          onClick={() => {
                            setSwitchingModel(key);
                            void switchRateLimitedModel(alternative)
                              .then((switched) => {
                                // 切换失败还在旧模型上,重发只会再次限流
                                if (switched !== false && lastUserText) {
                                  return sendMessage(
                                    lastUserText,
                                    routerNavigate,
                                  );
                                }
                                return undefined;
                              })
                              .finally(() => setSwitchingModel(null));
                          }}
                          className="border-warn/30 text-warn"
                        >
                          {switchingModel === key
                            ? t("chat.switchingModel")
                            : t(
                                lastUserText
                                  ? "chat.switchModelRetry"
                                  : "chat.switchModel",
                                {
                                  name:
                                    alternative.model_name ||
                                    alternative.model_id,
                                },
                              )}
                        </Button>
                      );
                    })
                  : undefined
              }
            >
              {bannerMessage}
            </Banner>
          </div>
        </div>
      )}

      {!historyLoading && isEmpty ? (
        // WorkBuddy 的空态从视口上方约 15% 开始；固定上边距比靠
        // 大块 padding-bottom 挤压居中更稳定，矮窗口也不会顶到标题栏。
        // 能力胶囊 → composer;胶囊贴着 composer(其自带 pt-2)形成一组输入区
        <div className="qp-fade-in flex min-h-0 flex-1 flex-col justify-start pb-10 pt-[12vh]">
          <div className="px-4 sm:px-6">
            <h1 className="font-display text-center text-[32px] font-semibold leading-[42px] tracking-[-0.025em] text-ink sm:text-[34px]">
              {timeGreeting(t)}
            </h1>
            <p className="sr-only">
              {t("chat.emptyHint", { shortcut: shortcutLabel("K") })}
            </p>
          </div>
          <div className="mt-16">
            <Composer wide />
          </div>
        </div>
      ) : (
        <>
          {/* 会话页头(对标 WB 44px 任务栏):滚动不带走任务身份。
              macOS 壳下同时充当可拖拽区。 */}
          <header
            data-tauri-drag-region
            onMouseDown={onTitlebarMouseDown}
            className={`qp-fade-in relative z-30 flex h-11 shrink-0 items-center border-b border-line bg-canvas pr-4 ${
              isMacDesktopShell() && sidebarCollapsed ? "pl-40" : "pl-4"
            }`}
          >
            <div
              data-tauri-drag-region
              className="flex min-w-0 flex-1 items-center gap-2"
            >
              <span
                data-tauri-drag-region
                className="min-w-0 truncate text-[13px] font-medium text-ink"
              >
                {activeChatName || t("sidebar.untitled")}
              </span>
              {activeProject && (
                <span
                  data-tauri-drag-region
                  className="flex min-w-0 max-w-[13rem] items-center gap-1 border-l border-line pl-2 text-[11px] text-ink-tertiary"
                  title={activeProject.path}
                >
                  <FolderClosed
                    data-tauri-drag-region
                    size={12}
                    className="shrink-0"
                  />
                  <span data-tauri-drag-region className="truncate">
                    {activeProject.name}
                  </span>
                </span>
              )}
            </div>
            <div className="ml-2 flex shrink-0 items-center gap-0.5">
              <button
                type="button"
                onClick={() => setSearchOpen((value) => !value)}
                title={t("chat.search.title")}
                aria-label={t("chat.search.title")}
                aria-expanded={searchOpen}
                className={`flex h-8 w-8 items-center justify-center rounded-[var(--radius-sm)] transition-colors ${
                  searchOpen
                    ? "bg-fill-active text-icon-strong"
                    : "text-icon hover:bg-fill-hover hover:text-icon-strong"
                }`}
              >
                <Search size={15} />
              </button>
              <button
                type="button"
                onClick={() =>
                  sidePanelOpen ? closeSidePanel() : setSidePanelOpen(true)
                }
                title={t("chat.panel.open")}
                aria-label={t("chat.panel.open")}
                aria-expanded={sidePanelOpen}
                className={`relative flex h-8 w-8 items-center justify-center rounded-[var(--radius-sm)] transition-colors ${
                  sidePanelOpen
                    ? "bg-fill-active text-icon-strong"
                    : "text-icon hover:bg-fill-hover hover:text-icon-strong"
                }`}
              >
                <PanelRightOpen size={15} />
                {artifacts.length + fileChanges.length > 0 && (
                  <span className="absolute right-0.5 top-0.5 flex min-w-3.5 items-center justify-center rounded-full bg-btn-primary px-1 text-[9px] leading-3.5 text-btn-primary-ink">
                    {artifacts.length + fileChanges.length > 9
                      ? "9+"
                      : artifacts.length + fileChanges.length}
                  </span>
                )}
              </button>
            </div>
          </header>
          {searchOpen && (
            <div className="absolute right-3 top-12 z-30 w-[min(24rem,calc(100%-1.5rem))] overflow-hidden rounded-[var(--radius-md)] border border-line bg-raised shadow-[var(--shadow-lg)]">
              <div className="flex h-11 items-center gap-2 border-b border-line px-3">
                <Search size={14} className="shrink-0 text-icon" />
                <input
                  ref={searchInputRef}
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  onKeyDown={handleSearchKeyDown}
                  placeholder={t("chat.search.placeholder")}
                  aria-label={t("chat.search.title")}
                  className="min-w-0 flex-1 bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-muted"
                />
                {searchQuery && (
                  <span className="shrink-0 text-[11px] tabular-nums text-ink-tertiary">
                    {t("chat.search.count", { count: searchMatches.length })}
                  </span>
                )}
                <button
                  type="button"
                  onClick={() => setSearchOpen(false)}
                  title={t("chat.search.close")}
                  aria-label={t("chat.search.close")}
                  className="flex h-7 w-7 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-icon hover:bg-fill-hover hover:text-icon-strong"
                >
                  <X size={14} />
                </button>
              </div>
              <div className="max-h-64 overflow-y-auto p-1.5">
                {!searchQuery.trim() ? (
                  <div className="px-3 py-5 text-center text-xs text-ink-tertiary">
                    {t("chat.search.hint")}
                  </div>
                ) : searchMatches.length === 0 ? (
                  <div className="px-3 py-5 text-center text-xs text-ink-tertiary">
                    {t("chat.search.empty")}
                  </div>
                ) : (
                  searchMatches.map((match) => (
                    <button
                      key={match.id}
                      type="button"
                      onClick={() => locateMessage(match.id)}
                      className={`block w-full rounded-[var(--radius-sm)] px-3 py-2 text-left transition-colors ${
                        activeSearchMessageId === match.id
                          ? "bg-fill-active"
                          : "hover:bg-fill-hover"
                      }`}
                    >
                      <span className="block text-[11px] text-ink-tertiary">
                        {match.role === "user"
                          ? t("chat.search.you")
                          : t("chat.search.assistant")}
                      </span>
                      <span className="mt-0.5 block truncate text-[13px] text-ink-secondary">
                        {match.text}
                      </span>
                    </button>
                  ))
                )}
              </div>
            </div>
          )}
          <div className="qp-fade-in flex min-h-0 flex-1">
            <section className="flex min-h-0 min-w-0 flex-1 flex-col">
              <div
                ref={scrollRef}
                className="min-h-0 flex-1 overflow-y-auto overscroll-contain"
              >
                {historyLoading ? (
                  <div className="mx-auto w-full max-w-[48rem] px-8 py-10">
                    <Card className="p-4">
                      <SkeletonRows rows={6} />
                    </Card>
                  </div>
                ) : (
                  <MessageList
                    messages={messages}
                    activeMessageId={activeSearchMessageId}
                    onOpenFile={openFilePreview}
                    onOpenChange={openChangeDiff}
                  />
                )}
              </div>
              <div className="relative">
                {showBackToBottom && (
                  <div className="pointer-events-none absolute left-1/2 top-0 z-20 -translate-x-1/2 -translate-y-full pb-2">
                    <button
                      type="button"
                      onClick={scrollToBottom}
                      className="pointer-events-auto flex items-center gap-1.5 whitespace-nowrap rounded-full border border-line bg-raised px-3 py-1.5 text-xs text-ink-secondary shadow-[var(--shadow-md)] transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
                    >
                      <ArrowDown size={13} />
                      {t("chat.backToBottom")}
                    </button>
                  </div>
                )}
                <Composer />
              </div>
            </section>
            {sidePanelOpen && (
              <Suspense fallback={null}>
                <ConversationSidePanel
                  messages={messages}
                  artifacts={artifacts}
                  changes={fileChanges}
                  responseStatus={responseStatus}
                  selectedFilePath={selectedFilePath}
                  selectedChangePath={selectedChangePath}
                  onClose={closeSidePanel}
                  onFileClose={backToPanelHome}
                  onOpenFile={openFilePreview}
                  onOpenChange={openChangeDiff}
                  onLocate={locateMessage}
                />
              </Suspense>
            )}
          </div>
        </>
      )}
    </div>
  );
}
