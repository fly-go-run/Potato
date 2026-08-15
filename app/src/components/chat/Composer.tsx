import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ArrowUp,
  AtSign,
  Check,
  ChevronDown,
  FileText,
  Loader2,
  Mic,
  Paperclip,
  Plus,
  ShieldCheck,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type SyntheticEvent,
} from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { Button, IconButton } from "../ui";
import { ApiError, sttApi } from "../../lib/api";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import { skillApi, type SkillInfo } from "../../lib/capabilities";
import {
  skillDescription,
  skillDisplayName,
} from "../../lib/skillPresentation";
import { isImeCommitEnter } from "../../lib/ime";
import {
  applyTrigger,
  detectTrigger,
  type ComposerTrigger,
} from "../../lib/composerTrigger";
import { useChatStore, type ApprovalLevel } from "../../stores/chat";
import { useUiPrefs } from "../../stores/uiPrefs";
import { VoiceInputError, VoiceRecorder } from "../../lib/voiceInput";
import { ModelPicker } from "./ModelPicker";
import { ProjectPicker } from "./ProjectPicker";
import { TriggerPopover, type TriggerItem } from "./TriggerPopover";

type VoiceUiState = "idle" | "starting" | "recording" | "transcribing";

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
  const location = useLocation();
  const { t, language } = useTranslation();
  const [text, setText] = useState("");
  const [voiceState, setVoiceState] = useState<VoiceUiState>("idle");
  const [voiceError, setVoiceError] = useState<string | null>(null);
  const showContextUsage = useUiPrefs((state) => state.showContextUsage);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isComposingRef = useRef(false);
  const compositionEndAtRef = useRef(Number.NEGATIVE_INFINITY);
  const voiceRecorderRef = useRef<VoiceRecorder | null>(null);
  /** Guards against max-duration auto-stop racing with a manual stop. */
  const voiceResultHandledRef = useRef(false);
  /** Bumped to invalidate in-flight getUserMedia / start() work. */
  const voiceSessionRef = useRef(0);
  /**
   * 后端说语音可用之前不挂麦克风按钮。预配置的安装包(家人那台)如果没
   * 带语音密钥,给一个必定失败的按钮比没有按钮更糟——失败提示还只对
   * 开发者有意义。
   */
  const [voiceAvailable, setVoiceAvailable] = useState(false);

  // `/` 技能、`@` 文件引用:触发态 + 键盘选中项 + 懒加载的技能列表
  const [trigger, setTrigger] = useState<ComposerTrigger | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const [dismissedToken, setDismissedToken] = useState<string | null>(null);
  const [skills, setSkills] = useState<SkillInfo[] | null>(null);
  const [skillsError, setSkillsError] = useState(false);
  const skillsRequested = useRef(false);

  // 随内容自动增高,192px(约 7 行,与 max-h-48 一致)封顶后内部滚动。
  // 静息高度分档:首页(wide)两行起步做邀请,会话内一行起步少占阅读区
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    // JS 下限与 CSS min-h 同值(86/46),避免哪天删了 min-h 静息高度悄悄塌掉
    el.style.height = `${Math.max(wide ? 86 : 46, Math.min(el.scrollHeight, 192))}px`;
  }, [text, wide]);
  const activeModel = useChatStore((state) => state.activeModel);
  const modelLoading = useChatStore((state) => state.modelLoading);
  const isStreaming = useChatStore((state) => state.isStreaming);
  const isSubmitting = useChatStore((state) => state.isSubmitting);
  const pendingImages = useChatStore((state) => state.pendingImages);
  const turnUsage = useChatStore((state) => state.stream.turnUsage);
  const conversationFileKey = useChatStore((state) =>
    state.stream.messages
      .flatMap((message) =>
        message.content.flatMap((block) =>
          block.type === "file" && block.filename
            ? [`${block.filename}\u0000${block.file_url ?? ""}`]
            : [],
        ),
      )
      .join("\u0001"),
  );
  const approvalLevel = useChatStore((state) => state.approvalLevel);
  const composerDraft = useChatStore((state) => state.composerDraft);
  const sendMessage = useChatStore((state) => state.sendMessage);
  const stop = useChatStore((state) => state.stop);
  const addImages = useChatStore((state) => state.addImages);
  const removeImage = useChatStore((state) => state.removeImage);
  const setApprovalLevel = useChatStore((state) => state.setApprovalLevel);
  const setComposerDraft = useChatStore((state) => state.setComposerDraft);

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
      if (current?.kind !== next?.kind || current?.start !== next?.start) {
        setActiveIndex(0);
      }
      return next;
    });
    if (next?.kind === "slash" && !skillsRequested.current) {
      skillsRequested.current = true;
      setSkillsError(false);
      skillApi
        .list()
        .then((items) => {
          setSkills(items);
          setSkillsError(false);
        })
        .catch(() => {
          // 失败不能伪装成"没有技能",并允许下次输入 / 时重试。
          setSkills([]);
          setSkillsError(true);
          skillsRequested.current = false;
        });
    }
  };

  const retrySkills = () => {
    skillsRequested.current = false;
    setSkills(null);
    setSkillsError(false);
    skillsRequested.current = true;
    skillApi
      .list()
      .then((items) => setSkills(items))
      .catch(() => {
        setSkills([]);
        setSkillsError(true);
        skillsRequested.current = false;
      });
  };

  // 会话内可引用的文件:已发送消息里的 file 块 + 待发送附件(新的在前)。
  // 去重 key 带上 url:同名不同来源的文件不能互相覆盖,description
  // 展示来源片段帮助区分。
  const conversationFiles = useMemo(() => {
    const seen = new Map<string, TriggerItem>();
    for (const message of useChatStore.getState().stream.messages) {
      for (const block of message.content) {
        if (block.type === "file" && block.filename) {
          const url = block.file_url ?? "";
          const key = `${block.filename}::${url}`;
          const duplicateName = Array.from(seen.values()).some(
            (item) => item.value === block.filename,
          );
          seen.set(key, {
            value: block.filename,
            description:
              duplicateName && url
                ? url.split("/").slice(-2).join("/")
                : undefined,
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
  }, [conversationFileKey, pendingImages, t]);

  const triggerItems = useMemo<TriggerItem[]>(() => {
    if (!trigger) return [];
    const needle = trigger.query.toLocaleLowerCase();
    if (trigger.kind === "slash") {
      return (skills ?? [])
        .filter((skill) => skill.enabled)
        .filter(
          (skill) =>
            !needle ||
            `${skill.name} ${skillDisplayName(skill.name, language)} ${skill.description}`
              .toLocaleLowerCase()
              .includes(needle),
        )
        .map((skill) => ({
          value: skill.name,
          label: skillDisplayName(skill.name, language),
          description: skillDescription(skill.name, language),
          icon: "skill" as const,
          emoji: skill.emoji,
        }));
    }
    return conversationFiles.filter(
      (item) => !needle || item.value.toLocaleLowerCase().includes(needle),
    );
  }, [trigger, skills, conversationFiles, language]);

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
    {
      value: "AUTO",
      label: t("composer.approval.auto"),
    },
    {
      value: "SMART",
      label: t("composer.approval.smart"),
    },
    {
      value: "STRICT",
      label: t("composer.approval.strict"),
    },
    {
      value: "OFF",
      label: t("composer.approval.off"),
    },
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
          aria-label={t("composer.approval.aria")}
          className={insideComposer ? "px-2" : "hidden px-2 sm:flex"}
        >
          <ShieldCheck size={16} strokeWidth={1.75} />
          {/* 审批档位是安全边界,任何入口都必须显示当前档位 */}
          {approvalLabel}
          <ChevronDown size={14} strokeWidth={1.8} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          sideOffset={6}
          align="start"
          className="qp-pop z-50 min-w-64 rounded-[var(--radius-md)] border border-line bg-raised p-1 shadow-[var(--shadow-md)]"
        >
          {/* 缩短的 chip 文案(自动/关闭)靠这里的节头找回语境 */}
          <div className="px-2.5 pb-1 pt-1.5 text-[11px] text-ink-muted">
            {t("composer.approval.aria")}
          </div>
          {approvalLevels.map((item) => (
            <DropdownMenu.Item
              key={item.value}
              onSelect={() => setApprovalLevel(item.value)}
              className="flex cursor-default items-start justify-between gap-3 rounded-sm px-2.5 py-2 outline-none hover:bg-fill-hover focus:bg-fill-active"
            >
              <span className="min-w-0">
                <span className="block text-xs text-ink">{item.label}</span>
              </span>
              {approvalLevel === item.value && (
                <Check size={14} strokeWidth={1.8} className="mt-0.5 shrink-0 text-accent" />
              )}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );


  const insertTriggerSymbol = (symbol: string) => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    const start = textarea.selectionStart ?? text.length;
    const end = textarea.selectionEnd ?? start;
    const before = text.slice(0, start);
    const inserted = (before && !/\s$/.test(before) ? " " : "") + symbol;
    const next = before + inserted + text.slice(end);
    setText(next);
    const caret = start + inserted.length;
    requestAnimationFrame(() => {
      textarea.focus();
      textarea.setSelectionRange(caret, caret);
      syncTrigger(textarea);
    });
  };

  const submit = () => {
    if (!canSend) return;
    const value = text;
    setText("");
    setTrigger(null);
    void sendMessage(value, navigate).then((accepted) => {
      if (!accepted) setText((current) => current || value);
    });
  };

  const appendTranscript = useCallback((transcript: string) => {
    const piece = transcript.trim();
    if (!piece) return;
    setText((current) => {
      if (!current.trim()) return piece;
      const needsSpace = !/\s$/.test(current);
      return `${current}${needsSpace ? " " : ""}${piece}`;
    });
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.focus();
      const end = el.value.length;
      el.setSelectionRange(end, end);
    });
  }, []);

  const mapVoiceError = useCallback(
    (reason: unknown): string => {
      if (reason instanceof VoiceInputError) {
        if (reason.code === "permission") return t("composer.voice.micError");
        if (reason.code === "unsupported")
          return t("composer.voice.unsupported");
        if (reason.code === "too_short") return t("composer.voice.tooShort");
        if (reason.code === "empty") return t("composer.voice.empty");
        return t("composer.voice.startFailed");
      }
      if (reason instanceof ApiError) {
        // 按后端 detail.code 分支。以前是拿文案做正则,而 ApiError 当时
        // 根本不带 code,那几条 includes 从来没命中过。
        const byCode: Record<string, TranslationKey> = {
          TRANSCRIPTION_DISABLED: "composer.voice.enableInSettings",
          SPEECH_API_KEY_MISSING: "composer.voice.keyMissing",
          AUDIO_CONVERSION_UNAVAILABLE: "composer.voice.ffmpegMissing",
        };
        const key = byCode[reason.code];
        if (key) return t(key);
        return reason.message || t("composer.voice.transcriptionFailed");
      }
      if (reason instanceof Error && reason.message) return reason.message;
      return t("composer.voice.transcriptionFailed");
    },
    [t],
  );

  /**
   * 转写前的前置检查。只读不写:转写会把音频送到第三方,启用与否是用户
   * 在设置里的明示选择,不能因为点了一下麦克风就替他把全局配置打开
   * (这个配置同时管各渠道的语音附件转写)。
   */
  const ensureVoiceReady = useCallback(async () => {
    try {
      const status = await sttApi.speechStatus();
      if (status.transcription_provider_type === "disabled") {
        throw new ApiError(t("composer.voice.enableInSettings"), 400);
      }
      if (
        status.transcription_provider_type === "doubao_asr" &&
        !status.doubao_credentials_configured
      ) {
        throw new ApiError(t("composer.voice.keyMissing"), 400);
      }
      // 这里不看 ffmpeg:录音器直接产 16k wav,后端原样收。真需要转码的
      // 只有别处传来的 webm/mp4,那由 /transcribe 按实际后缀判定。
    } catch (reason) {
      if (reason instanceof ApiError) throw reason;
      // speech-status may be unavailable on older backends; proceed and let
      // /transcribe report the real error.
    }
  }, [t]);

  const transcribeFile = useCallback(
    async (file: File) => {
      setVoiceState("transcribing");
      setVoiceError(null);
      try {
        await ensureVoiceReady();
        const result = await sttApi.transcribe(file);
        appendTranscript(result.text ?? "");
      } catch (reason) {
        setVoiceError(mapVoiceError(reason));
      } finally {
        setVoiceState("idle");
      }
    },
    [appendTranscript, ensureVoiceReady, mapVoiceError],
  );

  const handleVoiceFile = useCallback(
    async (file: File) => {
      if (voiceResultHandledRef.current) return;
      voiceResultHandledRef.current = true;
      voiceRecorderRef.current = null;
      await transcribeFile(file);
    },
    [transcribeFile],
  );

  const startVoice = useCallback(async () => {
    setVoiceError(null);
    voiceResultHandledRef.current = false;
    const session = ++voiceSessionRef.current;
    const recorder = new VoiceRecorder();
    voiceRecorderRef.current = recorder;
    setVoiceState("starting");
    try {
      await recorder.start((file) => {
        void handleVoiceFile(file);
      });
      // Permission dialog / async start may outlive cancel or a newer start.
      if (session !== voiceSessionRef.current) {
        recorder.cancel();
        if (voiceRecorderRef.current === recorder) {
          voiceRecorderRef.current = null;
        }
        return;
      }
      setVoiceState("recording");
    } catch (reason) {
      if (session !== voiceSessionRef.current) return;
      voiceRecorderRef.current = null;
      setVoiceState("idle");
      setVoiceError(mapVoiceError(reason));
    }
  }, [handleVoiceFile, mapVoiceError]);

  const stopVoice = useCallback(async () => {
    const recorder = voiceRecorderRef.current;
    if (!recorder) {
      setVoiceState("idle");
      return;
    }
    try {
      const file = await recorder.stop();
      await handleVoiceFile(file);
    } catch (reason) {
      if (voiceResultHandledRef.current) return;
      voiceRecorderRef.current = null;
      setVoiceState("idle");
      setVoiceError(mapVoiceError(reason));
    }
  }, [handleVoiceFile, mapVoiceError]);

  const cancelVoice = useCallback(() => {
    voiceSessionRef.current += 1;
    voiceResultHandledRef.current = true;
    voiceRecorderRef.current?.cancel();
    voiceRecorderRef.current = null;
    setVoiceState("idle");
  }, []);

  const toggleVoice = useCallback(() => {
    if (busy || voiceState === "transcribing" || voiceState === "starting") {
      return;
    }
    if (voiceState === "recording") {
      void stopVoice();
      return;
    }
    void startVoice();
  }, [busy, startVoice, stopVoice, voiceState]);

  useEffect(() => {
    return () => {
      voiceSessionRef.current += 1;
      voiceResultHandledRef.current = true;
      voiceRecorderRef.current?.cancel();
      voiceRecorderRef.current = null;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void sttApi
      .speechStatus()
      .then((status) => {
        if (active) setVoiceAvailable(Boolean(status.ready));
      })
      .catch(() => {
        // 老后端没有 speech-status:保持隐藏,不去赌它能用。
      });
    return () => {
      active = false;
    };
    // 设置页是覆盖路由,关掉它时 pathname 会变——顺带把「刚在设置里
    // 打开了语音」这件事反映到按钮上,不用重开应用。
  }, [location.pathname]);

  useEffect(() => {
    if (voiceState !== "recording") return;
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancelVoice();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cancelVoice, voiceState]);

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const imeCommitEnter = isImeCommitEnter(
      event.nativeEvent,
      isComposingRef.current,
      compositionEndAtRef.current,
    );

    if (trigger && !event.nativeEvent.isComposing && !imeCommitEnter) {
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
    if (event.key === "Enter" && !event.shiftKey && !imeCommitEnter) {
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
        <div
          className={`mx-auto mb-2 text-center text-xs text-warn ${widthClass}`}
        >
          {t("composer.modelMissing")}
          <Link
            to="/settings"
            state={{ background: location }}
            className="ml-1 underline underline-offset-2"
          >
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
            errorText={
              trigger.kind === "slash" && skillsError
                ? t("composer.skillsLoadFailed")
                : undefined
            }
            onRetry={retrySkills}
            onSelect={selectTriggerItem}
            onHover={setActiveIndex}
          />
        )}
        {/* 单卡结构(2026-08-14 终版):托盘层退役,工作区/审批 chip
            收进输入卡底部控制行——与会话页 composer 同形。 */}
        <div className="overflow-visible">
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
                        <FileText size={18} strokeWidth={1.75} />
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
                      <X size={12} strokeWidth={1.8} />
                    </IconButton>
                  </div>
                ))}
              </div>
            )}

            <textarea
              ref={textareaRef}
              data-testid="composer-input"
              // rows=1:空文本区的 scrollHeight 起点是一行,静息高度由
              // min-h 分档(wide 86 / 会话 46)和 JS 下限决定,不被 rows 顶起
              rows={1}
              value={text}
              onChange={(event) => {
                setText(event.target.value);
                syncTrigger(event.target);
              }}
              onSelect={(event: SyntheticEvent<HTMLTextAreaElement>) =>
                syncTrigger(event.currentTarget)
              }
              onBlur={() => setTrigger(null)}
              onCompositionStart={() => {
                isComposingRef.current = true;
              }}
              onCompositionEnd={(event) => {
                isComposingRef.current = false;
                compositionEndAtRef.current = event.timeStamp;
              }}
              onKeyDown={onKeyDown}
              onPaste={onPaste}
              disabled={busy || voiceState === "transcribing"}
              placeholder={
                voiceState === "starting"
                  ? t("composer.voice.starting")
                  : voiceState === "recording"
                  ? t("composer.voice.listening")
                  : voiceState === "transcribing"
                  ? t("composer.voice.transcribing")
                  : isSubmitting
                  ? t("composer.uploading")
                  : isStreaming
                  ? t("composer.generating")
                  : t("composer.placeholder")
              }
              className={`block ${
                wide ? "min-h-[86px]" : "min-h-[46px]"
              } max-h-48 w-full resize-none overflow-y-auto bg-transparent px-5 pb-1 pt-4 text-[16px] leading-6 text-ink outline-none placeholder:text-ink-muted disabled:cursor-not-allowed disabled:opacity-55`}
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
              {/* 「+」=「往对话里加东西」的家:附件、@ 引用、/ 技能。
                  菜单项右侧的弱化符号就是教学——用几次自然改打快捷符。 */}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger asChild>
                  <IconButton
                    size="sm"
                    disabled={busy}
                    title={t("composer.add")}
                  >
                    <Plus size={20} strokeWidth={1.75} />
                  </IconButton>
                </DropdownMenu.Trigger>
                <DropdownMenu.Portal>
                  <DropdownMenu.Content
                    sideOffset={6}
                    align="start"
                    // 焦点必须回 textarea 而不是「+」:onBlur 会清 trigger 态
                    onCloseAutoFocus={(event) => event.preventDefault()}
                    className="qp-pop z-50 min-w-44 rounded-[var(--radius-md)] border border-line bg-raised p-1 shadow-[var(--shadow-md)]"
                  >
                    <DropdownMenu.Item
                      onSelect={() => fileInputRef.current?.click()}
                      className="flex cursor-default items-center gap-2.5 rounded-sm px-2.5 py-2 text-xs text-ink outline-none hover:bg-fill-hover focus:bg-fill-active"
                    >
                      <Paperclip size={14} strokeWidth={1.8} className="text-icon" />
                      {t("composer.addMenu.upload")}
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                      onSelect={() => insertTriggerSymbol("@")}
                      className="flex cursor-default items-center gap-2.5 rounded-sm px-2.5 py-2 text-xs text-ink outline-none hover:bg-fill-hover focus:bg-fill-active"
                    >
                      <AtSign size={14} strokeWidth={1.8} className="text-icon" />
                      <span className="flex-1">
                        {t("composer.addMenu.reference")}
                      </span>
                      <span className="font-mono text-[11px] text-ink-tertiary">
                        @
                      </span>
                    </DropdownMenu.Item>
                    <DropdownMenu.Item
                      onSelect={() => insertTriggerSymbol("/")}
                      className="flex cursor-default items-center gap-2.5 rounded-sm px-2.5 py-2 text-xs text-ink outline-none hover:bg-fill-hover focus:bg-fill-active"
                    >
                      <Sparkles size={14} strokeWidth={1.8} className="text-icon" />
                      <span className="flex-1">
                        {t("composer.addMenu.skill")}
                      </span>
                      <span className="font-mono text-[11px] text-ink-tertiary">
                        /
                      </span>
                    </DropdownMenu.Item>
                  </DropdownMenu.Content>
                </DropdownMenu.Portal>
              </DropdownMenu.Root>

              {wide && <ProjectPicker />}
              {renderApprovalControl(true)}

              <div className="flex-1" />

              {/* 上下文用量默认不显示(设置 → 通用 里可打开):它在绝大多数
                  会话里停在个位数,没有可操作性,却一直占着输入框旁的注意力 */}
              {showContextUsage &&
                turnUsage?.context_usage?.context_usage_ratio !== undefined && (
                  <span
                    className={`hidden pr-1 text-[11px] sm:inline ${
                      turnUsage.context_usage.context_usage_ratio >= 80
                        ? "text-warn"
                        : "text-ink-tertiary"
                    }`}
                  >
                    {t("chat.contextUsed", {
                      ratio:
                        turnUsage.context_usage.context_usage_ratio.toFixed(1),
                    })}
                  </span>
                )}

              <ModelPicker />

              {voiceAvailable && (
                <IconButton
                  size="sm"
                  data-testid="composer-voice"
                  disabled={
                    busy ||
                    voiceState === "transcribing" ||
                    voiceState === "starting"
                  }
                  title={
                    voiceState === "recording"
                      ? t("composer.voice.stop")
                      : voiceState === "starting"
                      ? t("composer.voice.starting")
                      : voiceState === "transcribing"
                      ? t("composer.voice.transcribing")
                      : t("composer.voice.start")
                  }
                  aria-pressed={voiceState === "recording"}
                  onClick={toggleVoice}
                  // mr-[17px]:让麦克风左右的留白在静止态看起来一样宽。
                  //
                  // 行上是统一的 gap-1(4px),但两侧的邻居性质不同:模型选择器
                  // 有 14px 内边距且静止态不铺底色,那 14px 就是纯死白,全部计入
                  // 左侧空隙;发送键是实心圆,右侧只有 gap 本身。实测墨迹到墨迹
                  // 是 29.6px vs 16.7px,差了一倍。补到 17px 让两边都是 ~29.6px。
                  // 放在麦克风上而不是发送键上:语音不可用时它整个不渲染,不会
                  // 连带改动原有布局。
                  className={`mr-[17px] ${
                    voiceState === "recording"
                      ? "text-danger hover:text-danger"
                      : ""
                  }`}
                >
                  {voiceState === "transcribing" ||
                  voiceState === "starting" ? (
                    <Loader2 size={16} strokeWidth={1.75} className="animate-spin" />
                  ) : voiceState === "recording" ? (
                    // 录音中按钮的动作是「停止」,就得画成停止:红色脉冲的
                    // 麦克风只说明正在录,没告诉用户点下去会怎样。方块与
                    // 发送键的停流按钮同一套语义。
                    <Square
                      size={16}
                      fill="currentColor"
                      className="animate-pulse"
                    />
                  ) : (
                    <Mic size={18} strokeWidth={1.75} />
                  )}
                </IconButton>
              )}

              {isStreaming ? (
                <button
                  type="button"
                  data-testid="composer-stop"
                  title={t("composer.stop")}
                  onClick={() => void stop()}
                  className={sendButtonClass}
                >
                  <Square size={16} fill="currentColor" />
                </button>
              ) : (
                <button
                  type="button"
                  data-testid="composer-send"
                  title={t("composer.send")}
                  disabled={!canSend || voiceState !== "idle"}
                  onClick={submit}
                  className={sendButtonClass}
                >
                  <ArrowUp size={16} strokeWidth={2.4} />
                </button>
              )}
            </div>
          </div>

          {voiceError && (
            <p
              data-testid="composer-voice-error"
              className="mt-2 px-1 text-center text-xs text-danger"
              role="alert"
            >
              {voiceError}
            </p>
          )}

        </div>
      </div>
    </div>
  );
}
