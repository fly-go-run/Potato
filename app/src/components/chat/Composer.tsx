import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ArrowUp,
  Check,
  ChevronDown,
  FileText,
  Paperclip,
  Square,
  X,
} from "lucide-react";
import {
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
} from "react";
import { Link, useNavigate } from "react-router-dom";
import { Button, IconButton } from "../ui";
import { useTranslation } from "../../lib/i18n";
import {
  useChatStore,
  type ApprovalLevel,
} from "../../stores/chat";
import { ProjectPicker } from "./ProjectPicker";

export function Composer() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const {
    activeModel,
    modelLoading,
    isStreaming,
    isSubmitting,
    pendingImages,
    stream,
    approvalLevel,
    sendMessage,
    stop,
    addImages,
    removeImage,
    setApprovalLevel,
  } = useChatStore();
  const approvalLevels: Array<{
    value: ApprovalLevel;
    label: string;
  }> = [
    { value: "AUTO", label: t("composer.approval.auto") },
    { value: "SMART", label: t("composer.approval.smart") },
    { value: "STRICT", label: t("composer.approval.strict") },
    { value: "OFF", label: t("composer.approval.off") },
  ];
  const model = activeModel?.active_llm;
  const busy = isStreaming || isSubmitting;
  const canSend = Boolean(
    model && (text.trim() || pendingImages.length > 0) && !busy,
  );

  const submit = () => {
    if (!canSend) return;
    const value = text;
    setText("");
    void sendMessage(value, navigate).then((accepted) => {
      if (!accepted) setText((current) => current || value);
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.clipboardData.files);
    if (files.length) addImages(files);
  };

  return (
    <div className="px-4 pb-5 pt-2 sm:px-6">
      {!model && !modelLoading && (
        <div className="mx-auto mb-2 max-w-3xl text-center text-xs text-warn">
          {t("composer.modelMissing")}
          <Link to="/settings" className="ml-1 underline underline-offset-2">
            {t("composer.openSettings")}
          </Link>
        </div>
      )}
      <div className="mx-auto max-w-3xl rounded-[22px] border border-line bg-surface shadow-[var(--shadow-md)] transition-colors duration-[var(--dur-fast)] focus-within:border-line-strong">
        {pendingImages.length > 0 && (
          <div className="flex gap-2 overflow-x-auto px-3 pt-3">
            {pendingImages.map((attachment) => (
              <div
                key={attachment.id}
                className="group relative flex h-16 w-24 shrink-0 items-center justify-center overflow-hidden rounded-md border border-line bg-bubble-tool"
              >
                {attachment.previewUrl ? (
                  <img
                    src={attachment.previewUrl}
                    alt={attachment.file.name}
                    className="h-full w-full object-cover"
                  />
                ) : (
                  <div className="flex min-w-0 flex-col items-center gap-1 px-2 text-ink-secondary">
                    <FileText size={18} />
                    <span className="w-full truncate text-center text-[10px]">
                      {attachment.file.name}
                    </span>
                  </div>
                )}
                <IconButton
                  size="sm"
                  disabled={busy}
                  title={t("composer.removeAttachment")}
                  onClick={() => removeImage(attachment.id)}
                  className="absolute right-1 top-1 h-6 w-6 bg-raised shadow-[var(--shadow-sm)]"
                >
                  <X size={12} />
                </IconButton>
              </div>
            ))}
          </div>
        )}

        <textarea
          rows={2}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
          disabled={busy}
          placeholder={
            isSubmitting
              ? t("composer.uploading")
              : isStreaming
                ? t("composer.generating")
                : t("composer.placeholder")
          }
          className="block w-full resize-none bg-transparent px-4.5 pt-3.5 text-[15px] leading-6 text-ink outline-none placeholder:text-ink-muted disabled:cursor-not-allowed"
        />
        <div className="flex items-center gap-1 px-2 pb-2">
          <input
            ref={fileInputRef}
            type="file"
            multiple
            className="hidden"
            onChange={(event) => {
              addImages(Array.from(event.target.files ?? []));
              event.target.value = "";
            }}
          />
          <IconButton
            size="sm"
            disabled={busy}
            title={t("composer.addAttachment")}
            onClick={() => fileInputRef.current?.click()}
          >
            <Paperclip size={16} />
          </IconButton>

          <ProjectPicker />

          <Link
            to="/settings"
            title={
              model
                ? `${model.provider_id} / ${model.model}`
                : t("composer.selectModel")
            }
            className="max-w-48 truncate rounded-md px-2 py-1.5 text-xs text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
          >
            {modelLoading
              ? t("composer.loadingModel")
              : model?.model || t("composer.noModel")}
          </Link>

          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="hidden px-2 sm:flex"
              >
                {
                  approvalLevels.find((item) => item.value === approvalLevel)
                    ?.label
                }
                <ChevronDown size={13} />
              </Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content
                sideOffset={6}
                align="start"
                className="qp-pop z-50 min-w-36 rounded-[var(--radius-md)] border border-line bg-raised p-1 shadow-[var(--shadow-md)]"
              >
                {approvalLevels.map((item) => (
                  <DropdownMenu.Item
                    key={item.value}
                    onSelect={() => setApprovalLevel(item.value)}
                    className="flex cursor-default items-center justify-between rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active"
                  >
                    {item.label}
                    {approvalLevel === item.value && (
                      <Check size={13} className="text-accent" />
                    )}
                  </DropdownMenu.Item>
                ))}
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>

          <div className="flex-1" />

          {stream.turnUsage?.context_usage?.context_usage_ratio !== undefined && (
            <span className="hidden text-[11px] text-ink-muted sm:inline">
              {t("chat.contextUsed", {
                ratio: (
                  stream.turnUsage.context_usage.context_usage_ratio * 100
                ).toFixed(1),
              })}
            </span>
          )}

          {isStreaming ? (
            <Button
              variant="primary"
              size="sm"
              title={t("composer.stop")}
              onClick={() => void stop()}
              className="w-8 px-0"
            >
              <Square size={14} fill="currentColor" />
            </Button>
          ) : (
            <Button
              variant="primary"
              size="sm"
              title={t("composer.send")}
              disabled={!canSend}
              onClick={submit}
              className="w-8 px-0"
            >
              <ArrowUp size={16} />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
