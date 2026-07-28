import { CloudUpload } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent } from "react";
import { useParams } from "react-router-dom";
import { Composer } from "../components/chat/Composer";
import { MessageList } from "../components/chat/MessageList";
import { Banner } from "../components/ui/Banner";
import { Button, Card, SkeletonRows } from "../components/ui";
import { getChatBanner } from "../lib/chatBanner";
import { useTranslation } from "../lib/i18n";
import { shortcutLabel } from "../lib/shortcuts";
import { useChatStore } from "../stores/chat";

export function ChatView() {
  const { t } = useTranslation();
  const { chatId } = useParams();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [switchingModel, setSwitchingModel] = useState<string | null>(null);
  const {
    activeChatId,
    stream,
    historyLoading,
    error,
    isStreaming,
    sessionId,
    pendingApprovals,
    openChat,
    pollApprovals,
    switchRateLimitedModel,
    clearError,
    addImages,
  } = useChatStore();
  const [dragging, setDragging] = useState(false);
  const dragDepth = useRef(0);

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
  const banner = getChatBanner(error, stream.rateLimited);
  const searchShortcut = shortcutLabel("K");
  const isEmpty =
    stream.messages.length === 0 && pendingApprovals.length === 0;

  useEffect(() => {
    if (chatId && chatId !== activeChatId) void openChat(chatId);
  }, [activeChatId, chatId, openChat]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [pendingApprovals, stream.messages]);

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

  return (
    <div
      className="relative flex h-full flex-col"
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
                        void switchRateLimitedModel(alternative).finally(() =>
                          setSwitchingModel(null),
                        );
                      }}
                      className="border-warn/30 text-warn"
                    >
                      {switchingModel === key
                        ? t("chat.switchingModel")
                        : t("chat.switchModel", {
                            name:
                              alternative.model_name ||
                              alternative.model_id,
                          })}
                    </Button>
                  );
                })
              : undefined
          }
        >
          {banner.message}
        </Banner>
      )}

      {!historyLoading && isEmpty ? (
        <div className="flex min-h-0 flex-1 flex-col justify-center pb-[12vh]">
          <h1 className="text-center text-2xl font-medium tracking-tight text-ink">
            {t("chat.emptyTitle")}
          </h1>
          <p className="mt-2 text-center text-sm text-ink-muted">
            {t("chat.emptyHint", { shortcut: searchShortcut })}
          </p>
          <div className="mt-4 w-full">
            <Composer />
          </div>
        </div>
      ) : (
        <>
          <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
            {historyLoading ? (
              <div className="mx-auto w-full max-w-3xl px-6 py-8">
                <Card className="p-4">
                  <SkeletonRows rows={6} />
                </Card>
              </div>
            ) : (
              <MessageList messages={stream.messages} />
            )}
          </div>
          <Composer />
        </>
      )}
    </div>
  );
}
