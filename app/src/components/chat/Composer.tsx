import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ArrowUp,
  Check,
  ChevronDown,
  FileText,
  Plus,
  ShieldCheck,
  Square,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type SyntheticEvent,
} from "react";
import { Link, useNavigate } from "react-router-dom";
import { Button, IconButton } from "../ui";
import { useTranslation } from "../../lib/i18n";
import { skillApi, type SkillInfo } from "../../lib/capabilities";
import {
  applyTrigger,
  detectTrigger,
  type ComposerTrigger,
} from "../../lib/composerTrigger";
import {
  useChatStore,
  type ApprovalLevel,
} from "../../stores/chat";
import { ModelPicker } from "./ModelPicker";
import { ProjectPicker } from "./ProjectPicker";
import { TriggerPopover, type TriggerItem } from "./TriggerPopover";

/* 发送/停止是签名控件(对标 WB 的圆钮):36px 实心圆 + 粗箭头;
 * 禁用态保持实心近黑只降透明(r2 审查:灰底灰箭头读作"控件坏了",
 * 且它是空态首屏唯一的视觉锚点,必须始终"在")。 */
const sendButtonClass =
  "grid h-9 w-9 shrink-0 place-items-center rounded-full bg-btn-primary text-btn-primary-ink " +
  "shadow-[var(--shadow-control)] transition-[background-color,color,opacity] duration-[var(--dur-fast)] " +
  "hover:bg-btn-primary-hover active:opacity-90 " +
  "disabled:pointer-events-none disabled:opacity-45 " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring " +
  "focus-visible:ring-offset-2 focus-visible:ring-offset-surface";

const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg|bmp|avif)$/i;

export function Composer({ wide = false }: { wide?: boolean }) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // `/` 技能、`@` 文件引用:触发态 + 键盘选中项 + 懒加载的技能列表
  const [trigger, setTrigger] = useState<ComposerTrigger | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const [dismissedToken, setDismissedToken] = useState<string | null>(null);
  const [skills, setSkills] = useState<SkillInfo[] | null>(null);
  const skillsRequested = useRef(false);

  // 随内容自动增高:2 行(64px)起步,192px(约 7 行,与 max-h-48 一致)封顶后内部滚动
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.max(64, Math.min(el.scrollHeight, 192))}px`;
  }, [text]);
  const {
    activeModel,
    modelLoading,
    isStreaming,
    isSubmitting,
    pendingImages,
    stream,
    approvalLevel,
    composerDraft,
    sendMessage,
    stop,
    addImages,
    removeImage,
    setApprovalLevel,
    setComposerDraft,
  } = useChatStore();

  // 建议卡等外部入口写入的草稿:填入、聚焦、光标停在文末方便续写
  useEffect(() => {
    if (composerDraft === null) return;
    setText(composerDraft);
    setComposerDraft(null);
    const el = textareaRef.current;
    if (el) {
      el.focus();
      const end = composerDraft.length;
      requestAnimationFrame(() => el.setSelectionRange(end, end));
    }
  }, [composerDraft, setComposerDraft]);

  /* 触发检测统一走这里(输入、点击、方向键都会改变光标)。
   * Esc 关闭后记住被关的 token,同一 token 内不再弹出。 */
  const syncTrigger = (el: HTMLTextAreaElement) => {
    const next = detectTrigger(el.value, el.selectionStart ?? 0);
    const token = next ? `${next.kind}:${next.start}` : null;
    if (next && dismissedToken === token) {
      setTrigger(null);
      return;
    }
    if (!next) setDismissedToken(null);
    setTrigger((current) => {
      if (
        current?.kind !== next?.kind ||
        current?.start !== next?.start
      ) {
        setActiveIndex(0);
      }
      return next;
    });
    if (next?.kind === "slash" && !skillsRequested.current) {
      skillsRequested.current = true;
      skillApi
        .list()
        .then((items) => setSkills(items))
        .catch(() => setSkills([]));
    }
  };

  // 会话内可引用的文件:已发送消息里的 file 块 + 待发送附件(去重,新的在前)
  const conversationFiles = useMemo(() => {
    const seen = new Map<string, TriggerItem>();
    for (const message of stream.messages) {
      for (const block of message.content) {
        if (block.type === "file" && block.filename) {
          seen.set(block.filename, {
            value: block.filename,
            icon: IMAGE_EXT.test(block.filename) ? "image" : "file",
          });
        }
      }
    }
    for (const attachment of pendingImages) {
      seen.set(attachment.file.name, {
        value: attachment.file.name,
        description: t("composer.trigger.pending"),
        icon: IMAGE_EXT.test(attachment.file.name) ? "image" : "file",
      });
    }
    return Array.from(seen.values()).reverse();
  }, [stream.messages, pendingImages, t]);

  const triggerItems = useMemo<TriggerItem[]>(() => {
    if (!trigger) return [];
    const needle = trigger.query.toLocaleLowerCase();
    if (trigger.kind === "slash") {
      return (skills ?? [])
        .filter((skill) => skill.enabled)
        .filter(
          (skill) =>
            !needle ||
            `${skill.name} ${skill.description}`
              .toLocaleLowerCase()
              .includes(needle),
        )
        .map((skill) => ({
          value: skill.name,
          description: skill.description,
          icon: "skill" as const,
          emoji: skill.emoji,
        }));
    }
    return conversationFiles.filter(
      (item) => !needle || item.value.toLocaleLowerCase().includes(needle),
    );
  }, [trigger, skills, conversationFiles]);

  const skillsLoading = trigger?.kind === "slash" && skills === null;

  const selectTriggerItem = (item: TriggerItem) => {
    const el = textareaRef.current;
    if (!el || !trigger) return;
    const caret = el.selectionStart ?? text.length;
    const applied = applyTrigger(text, caret, trigger, item.value);
    setText(applied.text);
    setTrigger(null);
    el.focus();
    requestAnimationFrame(() =>
      el.setSelectionRange(applied.caret, applied.caret),
    );
  };

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
  const approvalLabel =
    approvalLevels.find((item) => item.value === approvalLevel)?.label ??
    t("composer.approval.auto");

  // WorkBuddy keeps the permission selector inside the white composer card
  // on session pages, while the home page also exposes the wider environment
  // tray below it. Keep one menu implementation so both placements stay in
  // sync; only the presentation/label changes by placement.
  const renderApprovalControl = (insideComposer: boolean) => (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          variant="ghost"
          size="sm"
          className={insideComposer ? "px-2" : "hidden px-2 sm:flex"}
        >
          <ShieldCheck size={15} className="text-ink-tertiary" />
          {insideComposer ? t("composer.defaultPermission") : approvalLabel}
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
  );

  const submit = () => {
    if (!canSend) return;
    const value = text;
    setText("");
    setTrigger(null);
    void sendMessage(value, navigate).then((accepted) => {
      if (!accepted) setText((current) => current || value);
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (trigger && !event.nativeEvent.isComposing) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        if (!triggerItems.length) return;
        const delta = event.key === "ArrowDown" ? 1 : -1;
        setActiveIndex(
          (current) =>
            (current + delta + triggerItems.length) % triggerItems.length,
        );
        return;
      }
      if (
        (event.key === "Enter" || event.key === "Tab") &&
        triggerItems.length > 0
      ) {
        event.preventDefault();
        selectTriggerItem(triggerItems[activeIndex] ?? triggerItems[0]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setDismissedToken(`${trigger.kind}:${trigger.start}`);
        setTrigger(null);
        return;
      }
    }
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.clipboardData.files);
    if (files.length) addImages(files);
  };
  const widthClass = wide
    ? "w-full sm:w-[91%] sm:max-w-[90rem]"
    : "w-full max-w-[48rem]";

  return (
    <div className="px-4 pb-6 pt-3 sm:px-6">
      {!model && !modelLoading && (
        <div className={`mx-auto mb-2 text-center text-xs text-warn ${widthClass}`}>
          {t("composer.modelMissing")}
          <Link to="/settings" className="ml-1 underline underline-offset-2">
            {t("composer.openSettings")}
          </Link>
        </div>
      )}
      <div className={`relative mx-auto ${widthClass}`}>
        {trigger && (
          <TriggerPopover
            kind={trigger.kind}
            items={triggerItems}
            activeIndex={activeIndex}
            loading={Boolean(skillsLoading)}
            onSelect={selectTriggerItem}
            onHover={setActiveIndex}
          />
        )}
        {/* 双层结构(对标 WB):内层白卡=本次输入,外层托盘下挂会话环境
            (项目/审批)。圆角走标尺(18/14),阴影用 composer 专档
            (深色为 none:近黑背景上黑阴影是无效像素,层级靠表面差)。 */}
        <div
          className={
            wide
              ? "overflow-visible rounded-[var(--radius-bubble)] bg-composer-tray"
              : "overflow-visible"
          }
        >
          <div className="relative z-10 rounded-[var(--radius-bubble)] border border-line bg-surface shadow-[var(--shadow-composer)] transition-[border-color,box-shadow] duration-[var(--dur-fast)] focus-within:border-line-strong focus-within:shadow-[var(--shadow-composer-focus)]">
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
              ref={textareaRef}
              rows={2}
              value={text}
              onChange={(event) => {
                setText(event.target.value);
                syncTrigger(event.target);
              }}
              onSelect={(event: SyntheticEvent<HTMLTextAreaElement>) =>
                syncTrigger(event.currentTarget)
              }
              onBlur={() => setTrigger(null)}
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
              className="block min-h-[86px] max-h-48 w-full resize-none overflow-y-auto bg-transparent px-5 pb-1 pt-4 text-[15px] leading-6 text-ink outline-none placeholder:text-ink-muted disabled:cursor-not-allowed disabled:opacity-55"
            />
            <div className="flex items-center gap-1 px-3 pb-3">
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
                <Plus size={20} strokeWidth={1.9} />
              </IconButton>

              {!wide && renderApprovalControl(true)}

              <div className="flex-1" />

              <ModelPicker />

              {isStreaming ? (
                <button
                  type="button"
                  title={t("composer.stop")}
                  onClick={() => void stop()}
                  className={sendButtonClass}
                >
                  <Square size={15} fill="currentColor" />
                </button>
              ) : (
                <button
                  type="button"
                  title={t("composer.send")}
                  disabled={!canSend}
                  onClick={submit}
                  className={sendButtonClass}
                >
                  <ArrowUp size={18} strokeWidth={2.4} />
                </button>
              )}
            </div>
          </div>

          {wide && (
            /* 首页专属工作环境托盘；session 把默认权限留在白卡内，
             * 这里只承载工作区、首页审批档位和上下文用量。 */
            <div className="flex min-h-11 items-center gap-1 px-3 pb-2 pt-2">
              <ProjectPicker />
              {renderApprovalControl(false)}

              <div className="flex-1" />

              {stream.turnUsage?.context_usage?.context_usage_ratio !==
                undefined && (
                <span className="hidden pr-1 text-[11px] text-ink-muted sm:inline">
                  {t("chat.contextUsed", {
                    // 后端 ratio 已是百分数(context_stats.py 乘过 100)
                    ratio:
                      stream.turnUsage.context_usage.context_usage_ratio.toFixed(
                        1,
                      ),
                  })}
                </span>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
