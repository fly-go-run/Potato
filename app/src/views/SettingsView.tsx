import * as Dialog from "@radix-ui/react-dialog";
import {
  Bot,
  ChevronLeft,
  ChevronRight,
  Download,
  HardDrive,
  Info,
  Keyboard,
  LoaderCircle,
  PlugZap,
  Plus,
  Radar,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import {
  Badge,
  Button,
  ConfirmDialog,
  IconButton,
  Input,
  SegmentedControl,
  Select,
  SkeletonRows,
  Switch,
} from "../components/ui";
import { ShortcutList } from "../components/layout/ShortcutsDialog";
import { APP_NAME, APP_VERSION } from "../lib/appInfo";
import { cn } from "../lib/cn";
import {
  apiFetch,
  apiJson,
  modelApi,
  providerConfigured,
  providerReady,
  settingsApi,
  sttApi,
  webSearchApi,
  CUSTOM_PROVIDER_PROTOCOLS,
  type ChatModelName,
  type ModelInfo,
  type ProviderInfo,
  type SandboxStatus,
  type TranscriptionProviderType,
  type WebSearchBackend,
  type WebSearchSettings,
} from "../lib/api";
import {
  pluginApi,
  runOptimisticSkillToggle,
  skillApi,
  type SkillInfo,
} from "../lib/capabilities";
import {
  getDesktopWindowStatePreference,
  hasDesktopHostBridge,
  resetDesktopWindowState,
  setDesktopWindowStatePreference,
} from "../lib/desktop";
import { skillDisplayName } from "../lib/skillPresentation";
import { useTranslation, type TranslationKey } from "../lib/i18n";
import {
  buildThemeTemplate,
  getActiveCustomThemeId,
  getThemePreference,
  importCustomTheme,
  listCustomThemes,
  removeCustomTheme,
  setActiveCustomTheme,
  setThemePreference,
  type CustomTheme,
  type ThemePreference,
} from "../lib/theme";
import { useChatStore } from "../stores/chat";
import { useUiPrefs } from "../stores/uiPrefs";

/**
 * 设置信息架构(r9 重做):两个分区。
 * 「模型与服务商」= 当前模型只读 + 供应商 master-detail(连接/模型管理);
 * 「通用」= 外观 + 语言 + 能力入口 + 沙箱。切换模型的动作归 composer。
 */
type SectionId =
  | "models"
  | "general"
  | "security"
  | "data"
  | "shortcuts"
  | "about";

const SECTION_LABELS: Record<SectionId, TranslationKey> = {
  models: "settings.nav.modelsProviders",
  general: "settings.nav.general",
  security: "settings.nav.security",
  data: "settings.nav.data",
  shortcuts: "settings.nav.shortcuts",
  about: "settings.nav.about",
};

/**
 * 缺 key 的后果取决于选了哪档,所以文案不能一概而论:auto 会退回 Tavily,
 * 显式选服务端搜索则是直接失败(那正是"显式"的意义),而选了 Tavily 时
 * 有没有 key 根本不相干。
 */
function webSearchHint(state: WebSearchSettings | null): TranslationKey | null {
  // 只在会出问题时说话:缺密钥仍选服务端=会失败;缺密钥自动档=已回退。
  if (!state || state.hosted_configured) return null;
  if (state.web_search_backend === "tavily") return null;
  return state.web_search_backend === "hosted"
    ? "settings.webSearch.needsKeyStrict"
    : "settings.webSearch.needsKey";
}

/** The dropdown is one list; these two are not providers. */
const WEB_SEARCH_AUTO = "__auto__";
const WEB_SEARCH_TAVILY = "__tavily__";

/**
 * 下拉的当前值:auto 与 tavily 是伪供应商项,其余是真实供应商 id。
 * hosted 但没记住供应商时落回 auto——显示一个空选项等于让用户猜。
 */
function webSearchSelectValue(state: WebSearchSettings | null): string {
  if (!state) return WEB_SEARCH_AUTO;
  if (state.web_search_backend === "tavily") return WEB_SEARCH_TAVILY;
  if (state.web_search_backend === "auto") return WEB_SEARCH_AUTO;
  return state.web_search_provider_id || WEB_SEARCH_AUTO;
}

/**
 * 换供应商时给一个能直接用的模型名,省得用户对着空框猜。只对内置的
 * DeepSeek 有把握;别家网关代理什么模型只有用户知道,所以沿用他上次填的。
 */
function defaultSearchModel(providerId: string, current: string): string {
  if (providerId.startsWith("deepseek")) return "deepseek-v4-flash";
  return current;
}

const APPROVAL_LABELS: Record<string, TranslationKey> = {
  AUTO: "composer.approval.auto",
  SMART: "composer.approval.smart",
  STRICT: "composer.approval.strict",
  OFF: "composer.approval.off",
};

/** 「模型与服务商」分区的三个视图:列表 / 供应商详情 / 新建自定义供应商。 */
type ProviderView =
  | { kind: "list" }
  | { kind: "detail"; providerId: string }
  | { kind: "create" };

type TestState =
  | { phase: "idle" }
  | { phase: "busy" }
  | { phase: "ok" }
  | { phase: "fail"; message: string };

export function SettingsView() {
  const { language, setLanguage, t } = useTranslation();
  const navigate = useNavigate();
  const showContextUsage = useUiPrefs((state) => state.showContextUsage);
  const setShowContextUsage = useUiPrefs((state) => state.setShowContextUsage);
  const activeModel = useChatStore((state) => state.activeModel);
  const loadActiveModel = useChatStore((state) => state.loadActiveModel);
  const [section, setSection] = useState<SectionId>("models");
  const [webSearch, setWebSearch] = useState<WebSearchSettings | null>(null);
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [providerView, setProviderView] = useState<ProviderView>({
    kind: "list",
  });
  const [keyDraft, setKeyDraft] = useState("");
  const [urlDraft, setUrlDraft] = useState("");
  const [protocolDraft, setProtocolDraft] =
    useState<ChatModelName>("OpenAIChatModel");
  const [testState, setTestState] = useState<TestState>({ phase: "idle" });
  const [savingProvider, setSavingProvider] = useState(false);
  const [clearingKey, setClearingKey] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [addingModel, setAddingModel] = useState(false);
  const [newModelId, setNewModelId] = useState("");
  const [newModelName, setNewModelName] = useState("");
  const [removingModel, setRemovingModel] = useState<string | null>(null);
  const [providerToRemove, setProviderToRemove] = useState<ProviderInfo | null>(
    null,
  );
  const [removingProvider, setRemovingProvider] = useState<string | null>(null);

  const [creating, setCreating] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createUrl, setCreateUrl] = useState("");
  const [createKey, setCreateKey] = useState("");
  const [createProtocol, setCreateProtocol] =
    useState<ChatModelName>("OpenAIChatModel");

  const [theme, setTheme] = useState<ThemePreference>(getThemePreference());
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>(() =>
    listCustomThemes(),
  );
  const [activeThemeId, setActiveThemeId] = useState<string | null>(() =>
    getActiveCustomThemeId(),
  );
  const themeFileRef = useRef<HTMLInputElement>(null);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [pluginCount, setPluginCount] = useState(0);
  const [sandbox, setSandbox] = useState<SandboxStatus | null>(null);
  const [savingSandbox, setSavingSandbox] = useState(false);
  const approvalLevel = useChatStore((state) => state.approvalLevel);
  const [uploadLimitMb, setUploadLimitMb] = useState<
    number | "unlimited" | "unknown"
  >("unknown");
  const [transcriptionType, setTranscriptionType] = useState<
    "disabled" | "whisper_api" | "local_whisper" | "doubao_asr" | "unknown"
  >("unknown");
  const [doubaoKeyReady, setDoubaoKeyReady] = useState(false);
  const [savingTranscription, setSavingTranscription] = useState(false);
  const [desktopWindowReady] = useState(() => hasDesktopHostBridge());
  const [rememberWindow, setRememberWindow] = useState(true);
  const [savingWindowPref, setSavingWindowPref] = useState(false);
  const [resettingWindow, setResettingWindow] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [backendHealth, setBackendHealth] = useState<
    { uptimeSeconds: number; agents: number } | "offline" | null
  >(null);
  // 面板是弹层：关闭时优先退回来路，直达 /settings 时回首页。
  const [canGoBack] = useState(() => {
    const state = window.history.state as { idx?: number } | null;
    return typeof state?.idx === "number" && state.idx > 0;
  });

  useEffect(() => {
    let active = true;
    void loadActiveModel();
    void modelApi
      .list()
      .then((providerList) => {
        if (!active) return;
        setProviders(providerList);
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setError(t("settings.loadFailed", { message: readableError(reason) }));
        setLoading(false);
      });
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t]);

  useEffect(() => {
    let active = true;
    void webSearchApi
      .get()
      .then((state) => {
        if (active) setWebSearch(state);
      })
      .catch(() => {
        // An older backend has no such endpoint; the row then just shows
        // the default rather than blocking the whole settings page.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void Promise.all([skillApi.list(), pluginApi.list()])
      .then(([skillList, plugins]) => {
        if (!active) return;
        setSkills(skillList);
        setPluginCount(plugins.length);
      })
      .catch(() => {
        // Settings remain usable when capability services are unavailable.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void sttApi
      .speechStatus()
      .then((status) => {
        if (!active) return;
        setTranscriptionType(status.transcription_provider_type);
        setDoubaoKeyReady(status.doubao_credentials_configured);
      })
      .catch(() => {
        // Older backends may not expose speech-status yet.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!desktopWindowReady) return;
    let active = true;
    void getDesktopWindowStatePreference()
      .then((pref) => {
        if (!active || !pref) return;
        setRememberWindow(pref.remember);
      })
      .catch(() => {
        // Older desktop shells may not expose window-state commands yet.
      });
    return () => {
      active = false;
    };
  }, [desktopWindowReady]);

  const toggleRememberWindow = async (next: boolean) => {
    setSavingWindowPref(true);
    setError(null);
    try {
      const ok = await setDesktopWindowStatePreference(next);
      if (!ok) {
        setError(t("settings.window.resetFailed"));
        return;
      }
      setRememberWindow(next);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingWindowPref(false);
    }
  };

  const resetWindow = async () => {
    setResettingWindow(true);
    setError(null);
    try {
      const ok = await resetDesktopWindowState();
      if (!ok) {
        setError(t("settings.window.resetFailed"));
        return;
      }
      setNotice(t("settings.window.resetDone"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setResettingWindow(false);
    }
  };

  const setVoiceTranscription = async (enabled: boolean) => {
    setSavingTranscription(true);
    setError(null);
    try {
      const next: TranscriptionProviderType = enabled
        ? "doubao_asr"
        : "disabled";
      const result = await sttApi.setProviderType(next);
      setTranscriptionType(result.transcription_provider_type);
      setNotice(
        enabled
          ? t("settings.voice.enabledNotice")
          : t("settings.voice.disabledNotice"),
      );
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingTranscription(false);
    }
  };

  /** 常用的技能启停就地完成,不把用户甩出设置;安装等高级操作走「管理」。 */
  const toggleSkill = async (name: string, enabled: boolean) => {
    setError(null);
    try {
      await runOptimisticSkillToggle({
        skills,
        name,
        enabled,
        onUpdate: setSkills,
        mutate: () => skillApi.setEnabled(name, enabled),
      });
    } catch (reason) {
      setError(readableError(reason));
    }
  };

  useEffect(() => {
    let active = true;
    void settingsApi
      .sandboxStatus()
      .then((status) => {
        if (active) setSandbox(status);
      })
      .catch(() => {
        // Sandbox row is hidden when the endpoint is unavailable.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void settingsApi
      .uploadLimit()
      .then((limit) => {
        if (!active) return;
        setUploadLimitMb(
          limit.upload_max_size_mb === null
            ? "unlimited"
            : limit.upload_max_size_mb,
        );
      })
      .catch(() => {
        // 行保持「—」,不阻塞其他设置。
      });
    void apiJson<{
      status: string;
      uptime_seconds: number;
      agents_loaded: string[];
    }>("/api/healthz")
      .then((health) => {
        if (!active) return;
        setBackendHealth({
          uptimeSeconds: health.uptime_seconds,
          agents: health.agents_loaded.length,
        });
      })
      .catch(() => {
        if (active) setBackendHealth("offline");
      });
    return () => {
      active = false;
    };
  }, []);

  const exportWorkspace = async () => {
    setExporting(true);
    clearBanners();
    try {
      const response = await apiFetch("/api/workspace/download");
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "potato-workspace.zip";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setNotice(t("settings.data.exported"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setExporting(false);
    }
  };

  const closePanel = () => {
    if (canGoBack) navigate(-1);
    else navigate("/");
  };

  const clearBanners = () => {
    setError(null);
    setNotice(null);
  };

  const toggleSandbox = async () => {
    if (!sandbox || savingSandbox) return;
    setSavingSandbox(true);
    clearBanners();
    try {
      const next = await settingsApi.setSandbox(!sandbox.enabled);
      setSandbox(next);
      setNotice(
        next.enabled
          ? t("settings.sandbox.enabledNotice")
          : t("settings.sandbox.disabledNotice"),
      );
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingSandbox(false);
    }
  };

  const refreshProviders = async () => {
    const providerList = await modelApi.list();
    setProviders(providerList);
    return providerList;
  };

  const applyUpdatedProvider = (updated: ProviderInfo) => {
    setProviders((items) =>
      items.map((item) => (item.id === updated.id ? updated : item)),
    );
  };

  const detailProvider =
    providerView.kind === "detail"
      ? providers.find((item) => item.id === providerView.providerId) ?? null
      : null;

  const openDetail = (provider: ProviderInfo) => {
    setProviderView({ kind: "detail", providerId: provider.id });
    setKeyDraft("");
    setUrlDraft(provider.base_url);
    setProtocolDraft(provider.chat_model as ChatModelName);
    setTestState({ phase: "idle" });
    setNewModelId("");
    setNewModelName("");
    clearBanners();
  };

  const backToList = () => {
    setProviderView({ kind: "list" });
    setTestState({ phase: "idle" });
    clearBanners();
  };

  /** 仅供删除模型/供应商后的兜底回切;设置 UI 不再提供主动切换入口。 */
  const activateModel = async (pid: string, mid: string) => {
    try {
      await modelApi.setActive(pid, mid);
      await loadActiveModel();
    } catch (reason) {
      setError(readableError(reason));
    }
  };

  const saveDetail = async () => {
    if (!detailProvider) return;
    const trimmedKey = keyDraft.trim();
    const nextBaseUrl = urlDraft.trim();
    // 不允许把 URL 存成空:空草稿视为"未修改",避免误清空端点。
    const baseUrlChanged =
      !detailProvider.freeze_url &&
      nextBaseUrl !== "" &&
      nextBaseUrl !== detailProvider.base_url;
    // 协议只对自定义供应商可改,内置的由后端固定。
    const protocolChanged =
      detailProvider.is_custom && protocolDraft !== detailProvider.chat_model;
    if (!trimmedKey && !baseUrlChanged && !protocolChanged) return;
    setSavingProvider(true);
    clearBanners();
    try {
      const updated = await modelApi.configure(detailProvider.id, {
        ...(trimmedKey ? { api_key: trimmedKey } : {}),
        ...(baseUrlChanged ? { base_url: nextBaseUrl } : {}),
        ...(protocolChanged ? { chat_model: protocolDraft } : {}),
      });
      applyUpdatedProvider(updated);
      setKeyDraft("");
      setUrlDraft(updated.base_url);
      setProtocolDraft(updated.chat_model as ChatModelName);
      setNotice(t("settings.provider.saved"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingProvider(false);
    }
  };

  const clearKey = async () => {
    if (!detailProvider) return;
    setClearingKey(true);
    clearBanners();
    try {
      const updated = await modelApi.configure(detailProvider.id, {
        api_key: "",
      });
      applyUpdatedProvider(updated);
      setKeyDraft("");
      setNotice(t("settings.provider.keyCleared"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setClearingKey(false);
    }
  };

  const testConnection = async () => {
    if (!detailProvider) return;
    setTestState({ phase: "busy" });
    setError(null);
    try {
      const trimmedKey = keyDraft.trim();
      const trimmedUrl = urlDraft.trim();
      const result = await modelApi.testProvider(detailProvider.id, {
        ...(trimmedKey ? { api_key: trimmedKey } : {}),
        ...(trimmedUrl && trimmedUrl !== detailProvider.base_url
          ? { base_url: trimmedUrl }
          : {}),
        ...(detailProvider.is_custom &&
        protocolDraft !== detailProvider.chat_model
          ? { chat_model: protocolDraft }
          : {}),
      });
      setTestState(
        result.success
          ? { phase: "ok" }
          : { phase: "fail", message: result.message },
      );
    } catch (reason) {
      setTestState({ phase: "fail", message: readableError(reason) });
    }
  };

  const discoverModels = async () => {
    if (!detailProvider) return;
    setDiscovering(true);
    clearBanners();
    try {
      const result = await modelApi.discover(detailProvider.id);
      await refreshProviders();
      setNotice(
        result.added_count > 0
          ? t("settings.models.discovered", { count: result.added_count })
          : t("settings.models.discoveredNone"),
      );
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setDiscovering(false);
    }
  };

  const addCustomModel = async () => {
    const id = newModelId.trim();
    if (!detailProvider || !id) return;
    setAddingModel(true);
    clearBanners();
    try {
      await modelApi.addModel(detailProvider.id, {
        id,
        name: newModelName.trim() || id,
      });
      await refreshProviders();
      setNewModelId("");
      setNewModelName("");
      setNotice(t("settings.models.added", { name: id }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setAddingModel(false);
    }
  };

  const removeModel = async (modelIdToRemove: string) => {
    if (!detailProvider) return;
    const providerId = detailProvider.id;
    setRemovingModel(modelIdToRemove);
    setError(null);
    try {
      await modelApi.removeModel(providerId, modelIdToRemove);
      const providerList = await refreshProviders();
      // 删掉的是当前激活模型时兜底回切,agent 不能悬空。
      const activePair = activeModel?.active_llm;
      if (
        activePair?.provider_id === providerId &&
        activePair.model === modelIdToRemove
      ) {
        const current = providerList.find((item) => item.id === providerId);
        const fallback = current ? providerModels(current)[0]?.id : undefined;
        if (fallback) await activateModel(providerId, fallback);
        else setNotice(t("settings.models.activeGone"));
      }
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setRemovingModel(null);
    }
  };

  const createProvider = async () => {
    const name = createName.trim();
    const id = slugifyProviderId(name);
    if (!name || !id) {
      setError(t("settings.create.invalidName"));
      return;
    }
    if (!createUrl.trim()) {
      setError(t("settings.create.urlRequired"));
      return;
    }
    setCreating(true);
    clearBanners();
    try {
      // 后端可能为去重改写 id(同名/与内置冲突),后续一律以返回值为准。
      const createdInfo = await modelApi.createCustomProvider({
        id,
        name,
        default_base_url: createUrl.trim(),
        chat_model: createProtocol,
      });
      const trimmedKey = createKey.trim();
      if (trimmedKey) {
        await modelApi.configure(createdInfo.id, { api_key: trimmedKey });
      }
      const providerList = await refreshProviders();
      const created = providerList.find((item) => item.id === createdInfo.id);
      setCreateName("");
      setCreateUrl("");
      setCreateKey("");
      setCreateProtocol("OpenAIChatModel");
      if (created) openDetail(created);
      else setProviderView({ kind: "list" });
      setNotice(t("settings.create.created", { name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setCreating(false);
    }
  };

  const removeProvider = async () => {
    const target = providerToRemove;
    if (!target || !target.is_custom) return;
    setRemovingProvider(target.id);
    clearBanners();
    try {
      const providerList = await modelApi.removeProvider(target.id);
      setProviders(providerList);
      setProviderView({ kind: "list" });

      // 激活模型属于被删供应商时兜底回切到首个可用项。
      const activePair = activeModel?.active_llm;
      if (activePair?.provider_id === target.id) {
        const remainingProvider =
          providerList.find(
            (provider) =>
              providerReady(provider) && providerModels(provider).length > 0,
          ) ?? null;
        const fallback = remainingProvider
          ? providerModels(remainingProvider)[0]?.id
          : undefined;
        if (remainingProvider && fallback) {
          await activateModel(remainingProvider.id, fallback);
        } else {
          setNotice(t("settings.models.activeGone"));
        }
      }

      setProviderToRemove(null);
      setNotice(t("settings.provider.deleted", { name: target.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setRemovingProvider(null);
    }
  };

  const chooseTheme = (preference: ThemePreference) => {
    setTheme(preference);
    setActiveThemeId(null);
    setThemePreference(preference);
  };

  const saveWebSearch = (update: {
    web_search_backend: WebSearchBackend;
    web_search_provider_id?: string;
    web_search_model?: string;
  }) => {
    const previous = webSearch;
    // Optimistic: snapping back on the next render would read as the click
    // not registering. hosted_configured is the server's to know, so after
    // the write lands we re-read rather than keep a guess.
    setWebSearch((state) =>
      state
        ? {
            ...state,
            ...update,
            web_search_provider_id:
              update.web_search_provider_id ?? state.web_search_provider_id,
            web_search_model: update.web_search_model ?? state.web_search_model,
          }
        : null,
    );
    void webSearchApi
      .set(update)
      .then(() => webSearchApi.get().then(setWebSearch))
      .catch((reason: unknown) => {
        setWebSearch(previous);
        setError(readableError(reason));
      });
  };

  const chooseWebSearchSource = (value: string) => {
    if (value === WEB_SEARCH_TAVILY) {
      saveWebSearch({ web_search_backend: "tavily" });
      return;
    }
    if (value === WEB_SEARCH_AUTO) {
      saveWebSearch({ web_search_backend: "auto" });
      return;
    }
    saveWebSearch({
      web_search_backend: "hosted",
      web_search_provider_id: value,
      web_search_model: defaultSearchModel(
        value,
        webSearch?.web_search_model ?? "",
      ),
    });
  };

  const importThemeFile = async (file: File) => {
    clearBanners();
    try {
      const imported = importCustomTheme(await file.text());
      setCustomThemes(listCustomThemes());
      setActiveCustomTheme(imported.id);
      setActiveThemeId(imported.id);
      setTheme(getThemePreference());
      setNotice(t("settings.theme.imported", { name: imported.name }));
    } catch (reason) {
      setError(
        t("settings.theme.importInvalid", {
          message: readableError(reason),
        }),
      );
    }
  };

  const selectCustomTheme = (id: string) => {
    clearBanners();
    setActiveCustomTheme(id);
    setActiveThemeId(id);
    setTheme(getThemePreference());
  };

  const deleteCustomTheme = (id: string) => {
    removeCustomTheme(id);
    setCustomThemes(listCustomThemes());
    if (activeThemeId === id) {
      setActiveThemeId(null);
      setTheme(getThemePreference());
    }
  };

  const downloadThemeTemplate = () => {
    const blob = new Blob([buildThemeTemplate()], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "potato-theme.json";
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  };

  const navItems: { id: SectionId; icon: ReactNode }[] = [
    { id: "models", icon: <Bot size={16} /> },
    { id: "general", icon: <SlidersHorizontal size={16} /> },
    { id: "security", icon: <ShieldCheck size={16} /> },
    { id: "data", icon: <HardDrive size={16} /> },
    { id: "shortcuts", icon: <Keyboard size={16} /> },
    { id: "about", icon: <Info size={16} /> },
  ];
  const activeSection: SectionId = navItems.some((item) => item.id === section)
    ? section
    : "models";

  const activePair = activeModel?.active_llm;
  const activeProvider = activePair
    ? providers.find((item) => item.id === activePair.provider_id) ?? null
    : null;

  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) closePanel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay backdrop-blur-[1px]" />
        <Dialog.Content
          // 打开时不聚焦首个导航项,避免一进来就带 focus 环(键盘 Tab 仍可达)
          onOpenAutoFocus={(event) => event.preventDefault()}
          className={cn(
            "qp-pop fixed inset-3 z-50 flex flex-col overflow-hidden outline-none",
            "rounded-[var(--radius-lg)] border border-line bg-surface shadow-[var(--shadow-lg)]",
            "sm:bottom-auto sm:right-auto sm:left-1/2 sm:top-1/2",
            // 随内容收缩：固定 85vh 会让短分区留下大片空腔
            "sm:h-auto sm:max-h-[85vh] sm:min-h-[30rem] sm:w-[min(56rem,calc(100vw-3rem))]",
            "sm:-translate-x-1/2 sm:-translate-y-1/2 sm:flex-row",
          )}
        >
          <Dialog.Description className="sr-only">
            {t("settings.subtitle")}
          </Dialog.Description>

          <nav
            aria-label={t("settings.title")}
            className={cn(
              // 导航面用 bg-bg，与右侧 bg-surface 形成表面分层（不再只靠 1px 竖线）
              "flex shrink-0 gap-1 overflow-x-auto border-b border-line bg-bg p-2",
              "sm:w-48 sm:flex-col sm:overflow-x-visible sm:overflow-y-auto",
              "sm:border-b-0 sm:border-r sm:p-3",
            )}
          >
            {navItems.map((item) => {
              const selected = item.id === activeSection;
              return (
                <button
                  key={item.id}
                  type="button"
                  aria-current={selected ? "page" : undefined}
                  onClick={() => setSection(item.id)}
                  className={cn(
                    "flex shrink-0 items-center gap-2 rounded-[var(--radius-md)] px-3 py-2 text-[13px]",
                    "transition-colors duration-[var(--dur-fast)]",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    selected
                      ? "bg-fill-active font-medium text-ink"
                      : "text-ink-secondary hover:bg-fill-hover hover:text-ink",
                  )}
                >
                  <span className={selected ? "text-ink" : "text-icon"}>
                    {item.icon}
                  </span>
                  {t(SECTION_LABELS[item.id])}
                </button>
              );
            })}
          </nav>

          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <header
              data-tauri-drag-region
              className="flex shrink-0 items-center justify-between gap-3 px-6 pb-3 pt-5"
            >
              <Dialog.Title className="text-[15px] font-semibold text-ink">
                {t(SECTION_LABELS[activeSection])}
              </Dialog.Title>
              <IconButton
                size="sm"
                aria-label={t("settings.close")}
                title={t("settings.close")}
                onClick={closePanel}
              >
                <X size={16} />
              </IconButton>
            </header>

            <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6 pt-0">
              {(error || notice) && (
                <div
                  className={`mb-4 rounded-[var(--radius-md)] px-3 py-2 text-xs ${
                    error
                      ? "bg-danger-soft text-danger"
                      : "bg-fill-active text-ok"
                  }`}
                >
                  {error || notice}
                </div>
              )}

              {loading ? (
                <SettingsGroup className="p-4">
                  <SkeletonRows rows={5} />
                </SettingsGroup>
              ) : activeSection === "models" ? (
                providerView.kind === "detail" && detailProvider ? (
                  <ProviderDetail
                    provider={detailProvider}
                    activeModelId={
                      activePair?.provider_id === detailProvider.id
                        ? activePair.model
                        : null
                    }
                    keyDraft={keyDraft}
                    urlDraft={urlDraft}
                    protocolDraft={protocolDraft}
                    testState={testState}
                    saving={savingProvider}
                    clearingKey={clearingKey}
                    discovering={discovering}
                    addingModel={addingModel}
                    // 统一 mutation 锁:任一写操作进行中,其余写操作全部禁用,
                    // 防止发现/增删/保存并发互踩(sol review P1)。
                    busy={
                      savingProvider ||
                      clearingKey ||
                      discovering ||
                      addingModel ||
                      removingModel !== null ||
                      removingProvider !== null
                    }
                    newModelId={newModelId}
                    newModelName={newModelName}
                    removingModel={removingModel}
                    onBack={backToList}
                    onKeyDraft={(value) => {
                      setKeyDraft(value);
                      setTestState({ phase: "idle" });
                    }}
                    onUrlDraft={(value) => {
                      setUrlDraft(value);
                      setTestState({ phase: "idle" });
                    }}
                    onProtocolDraft={(value) => {
                      setProtocolDraft(value);
                      setTestState({ phase: "idle" });
                    }}
                    onSave={() => void saveDetail()}
                    onClearKey={() => void clearKey()}
                    onTest={() => void testConnection()}
                    onDiscover={() => void discoverModels()}
                    onNewModelId={setNewModelId}
                    onNewModelName={setNewModelName}
                    onAddModel={() => void addCustomModel()}
                    onRemoveModel={(id) => void removeModel(id)}
                    onRemoveProvider={() => setProviderToRemove(detailProvider)}
                  />
                ) : providerView.kind === "create" ? (
                  <ProviderCreate
                    name={createName}
                    baseUrl={createUrl}
                    apiKey={createKey}
                    protocol={createProtocol}
                    creating={creating}
                    onName={setCreateName}
                    onBaseUrl={setCreateUrl}
                    onApiKey={setCreateKey}
                    onProtocol={setCreateProtocol}
                    onBack={backToList}
                    onSubmit={() => void createProvider()}
                  />
                ) : (
                  <div className="space-y-3">
                    <SettingsGroup>
                      <SettingRow
                        title={t("settings.models.current")}
                      >
                        <div className="text-right">
                          <div className="text-[13px] text-ink">
                            {activePair?.model || t("settings.models.noActive")}
                          </div>
                          {activePair && (
                            <div className="mt-0.5 text-xs text-ink-tertiary">
                              {providerDisplayName(
                                activeProvider?.name ?? activePair.provider_id,
                              )}
                              {activeModel?.effective_max_input_length
                                ? ` · ${t("settings.models.contextWindow", {
                                    count:
                                      activeModel.effective_max_input_length.toLocaleString(),
                                  })}`
                                : ""}
                            </div>
                          )}
                        </div>
                      </SettingRow>
                    </SettingsGroup>

                    <SettingsGroup className="p-2">
                      <div className="px-2 pb-2 pt-1">
                        <div className="text-[13px] font-medium text-ink">
                          {t("settings.provider.listTitle")}
                        </div>
                      </div>
                      <div className="space-y-1">
                        {sortProviders(providers).map((item) => (
                          <ProviderListRow
                            key={item.id}
                            provider={item}
                            onOpen={() => openDetail(item)}
                          />
                        ))}
                      </div>
                      <div className="mt-1 border-t border-line pt-1.5">
                        <button
                          type="button"
                          onClick={() => {
                            setProviderView({ kind: "create" });
                            clearBanners();
                          }}
                          className="flex w-full items-center gap-2 rounded-[var(--radius-sm)] px-3 py-2 text-[13px] text-ink-secondary transition-colors hover:bg-fill-hover hover:text-ink"
                        >
                          <Plus size={14} />
                          {t("settings.provider.addCustom")}
                        </button>
                      </div>
                    </SettingsGroup>
                  </div>
                )
              ) : activeSection === "general" ? (
                <div className="space-y-3">
                  <SettingsGroup>
                    <SettingRow
                      title={t("settings.appearance.theme")}
                    >
                      <SegmentedControl
                        variant="track"
                        value={theme}
                        options={[
                          { value: "light", label: t("settings.theme.light") },
                          { value: "dark", label: t("settings.theme.dark") },
                          {
                            value: "system",
                            label: t("settings.theme.system"),
                          },
                        ]}
                        onChange={chooseTheme}
                      />
                    </SettingRow>
                    <SettingRow
                      title={t("settings.webSearch.title")}
                      description={(() => {
                        const hint = webSearchHint(webSearch);
                        return hint ? t(hint) : undefined;
                      })()}
                    >
                      <Select
                        className="w-56"
                        aria-label={t("settings.webSearch.title")}
                        value={webSearchSelectValue(webSearch)}
                        onChange={(event) =>
                          chooseWebSearchSource(event.target.value)
                        }
                      >
                        <option value={WEB_SEARCH_AUTO}>
                          {t("settings.webSearch.auto")}
                        </option>
                        <option value={WEB_SEARCH_TAVILY}>
                          {t("settings.webSearch.tavily")}
                        </option>
                        {(webSearch?.providers ?? []).map((provider) => (
                          <option key={provider.id} value={provider.id}>
                            {provider.name}
                          </option>
                        ))}
                      </Select>
                    </SettingRow>
                    {webSearch?.web_search_backend === "hosted" && (
                      <SettingRow
                        title={t("settings.webSearch.model")}
                        description={t("settings.webSearch.modelHint")}
                      >
                        <Input
                          className="w-56"
                          defaultValue={webSearch.web_search_model}
                          placeholder="deepseek-v4-flash"
                          aria-label={t("settings.webSearch.model")}
                          // Commit on blur, like the provider key field:
                          // saving per keystroke would write a config file
                          // for every character.
                          onBlur={(event) => {
                            const next = event.target.value.trim();
                            if (!next || next === webSearch.web_search_model) {
                              return;
                            }
                            saveWebSearch({
                              web_search_backend: "hosted",
                              web_search_model: next,
                            });
                          }}
                        />
                      </SettingRow>
                    )}
                    <SettingRow
                      title={t("settings.contextUsage.title")}
                    >
                      <Switch
                        checked={showContextUsage}
                        onChange={() => setShowContextUsage(!showContextUsage)}
                        aria-label={t("settings.contextUsage.title")}
                      />
                    </SettingRow>
                    <SettingRow
                      title={t("settings.theme.custom")}
                    >
                      <div className="flex items-center gap-2">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={downloadThemeTemplate}
                        >
                          {t("settings.theme.template")}
                        </Button>
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={() => themeFileRef.current?.click()}
                        >
                          <Upload size={13} />
                          {t("settings.theme.import")}
                        </Button>
                        <input
                          ref={themeFileRef}
                          type="file"
                          accept=".json,application/json"
                          className="hidden"
                          onChange={(event) => {
                            const file = event.target.files?.[0];
                            event.target.value = "";
                            if (file) void importThemeFile(file);
                          }}
                        />
                      </div>
                    </SettingRow>
                    {customThemes.map((customTheme) => (
                      <div
                        key={customTheme.id}
                        className="flex items-center gap-3 border-t border-line px-4 py-2.5"
                      >
                        <button
                          type="button"
                          onClick={() => selectCustomTheme(customTheme.id)}
                          className="flex min-w-0 flex-1 items-center gap-2 text-left"
                        >
                          <span className="truncate text-[13px] text-ink">
                            {customTheme.name}
                          </span>
                          <span className="shrink-0 text-[11px] text-ink-tertiary">
                            {t(
                              customTheme.base === "dark"
                                ? "settings.theme.baseDark"
                                : "settings.theme.baseLight",
                            )}
                          </span>
                          {activeThemeId === customTheme.id && (
                            <Badge tone="ok">{t("settings.theme.inUse")}</Badge>
                          )}
                        </button>
                        <IconButton
                          size="sm"
                          tone="danger"
                          title={t("settings.theme.deleteTheme")}
                          aria-label={t("settings.theme.deleteTheme")}
                          onClick={() => deleteCustomTheme(customTheme.id)}
                        >
                          <Trash2 size={14} />
                        </IconButton>
                      </div>
                    ))}
                    <SettingRow
                      title={t("settings.language.title")}
                    >
                      <SegmentedControl
                        variant="track"
                        value={language}
                        options={[
                          { value: "zh", label: t("settings.language.zh") },
                          { value: "en", label: t("settings.language.en") },
                        ]}
                        onChange={setLanguage}
                      />
                    </SettingRow>
                    {desktopWindowReady && (
                      <>
                        <SettingRow
                          title={t("settings.window.remember")}
                        >
                          <Switch
                            checked={rememberWindow}
                            disabled={savingWindowPref}
                            onChange={() => {
                              void toggleRememberWindow(!rememberWindow);
                            }}
                            aria-label={t("settings.window.remember")}
                          />
                        </SettingRow>
                        <SettingRow
                          title={t("settings.window.reset")}
                        >
                          <Button
                            variant="secondary"
                            size="sm"
                            disabled={resettingWindow}
                            onClick={() => {
                              void resetWindow();
                            }}
                          >
                            {resettingWindow ? (
                              <LoaderCircle
                                size={13}
                                className="animate-spin"
                              />
                            ) : null}
                            {t("settings.window.reset")}
                          </Button>
                        </SettingRow>
                      </>
                    )}
                    <SettingRow
                      title={t("settings.voice.title")}
                      description={
                        doubaoKeyReady
                          ? undefined
                          : t("settings.voice.descriptionMissingKey")
                      }
                    >
                      <Switch
                        checked={transcriptionType === "doubao_asr"}
                        disabled={
                          savingTranscription ||
                          transcriptionType === "unknown" ||
                          (!doubaoKeyReady &&
                            transcriptionType !== "doubao_asr")
                        }
                        onChange={() => {
                          void setVoiceTranscription(
                            transcriptionType !== "doubao_asr",
                          );
                        }}
                        aria-label={t("settings.voice.title")}
                      />
                    </SettingRow>
                  </SettingsGroup>

                  <SettingsGroup>
                    <SettingRow
                      title={t("settings.capabilities.title")}
                      description={t("settings.capabilities.summary", {
                        enabled: skills.filter((skill) => skill.enabled).length,
                        skills: skills.length,
                        plugins: pluginCount,
                      })}
                    >
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => navigate("/skills")}
                      >
                        {t("settings.capabilities.manage")}
                      </Button>
                    </SettingRow>
                    {skills.length > 0 && (
                      <div className="max-h-72 overflow-y-auto border-t border-line">
                        {skills.map((skill) => (
                          <div
                            key={skill.name}
                            className="flex items-center gap-3 border-t border-line px-4 py-2 first:border-t-0"
                          >
                            <span
                              title={skill.name}
                              className="min-w-0 flex-1 truncate text-[13px] text-ink-secondary"
                            >
                              {skill.emoji ? `${skill.emoji} ` : ""}
                              {skillDisplayName(skill.name, language)}
                            </span>
                            <Switch
                              checked={skill.enabled}
                              onChange={() =>
                                void toggleSkill(skill.name, !skill.enabled)
                              }
                              aria-label={skillDisplayName(skill.name, language)}
                            />
                          </div>
                        ))}
                      </div>
                    )}
                  </SettingsGroup>
                </div>
              ) : activeSection === "security" ? (
                <SettingsGroup>
                  {sandbox && (
                    <SettingRow
                      title={t("settings.sandbox.label")}
                      description={
                        <>
                          {sandbox.enabled
                            ? t("settings.sandbox.on")
                            : t("settings.sandbox.off")}
                          {sandbox.enabled && !sandbox.effective && (
                            <span className="mt-1 block text-warn">
                              {t(
                                sandbox.reason === "unsupported"
                                  ? "settings.sandbox.unsupported"
                                  : "settings.sandbox.notAdmin",
                              )}
                            </span>
                          )}
                        </>
                      }
                    >
                      <Switch
                        checked={sandbox.enabled}
                        disabled={savingSandbox}
                        onChange={() => void toggleSandbox()}
                        aria-label={t("settings.sandbox.label")}
                      />
                    </SettingRow>
                  )}
                  <SettingRow
                    title={t("settings.security.approval")}
                  >
                    <span className="text-[13px] text-ink-secondary">
                      {t(
                        APPROVAL_LABELS[approvalLevel] ??
                          "composer.approval.auto",
                      )}
                    </span>
                  </SettingRow>
                </SettingsGroup>
              ) : activeSection === "data" ? (
                <SettingsGroup>
                  <SettingRow
                    title={t("settings.data.uploadLimit")}
                  >
                    <span className="text-[13px] tabular-nums text-ink-secondary">
                      {uploadLimitMb === "unknown"
                        ? "—"
                        : uploadLimitMb === "unlimited"
                        ? t("settings.data.uploadUnlimited")
                        : `${uploadLimitMb} MB`}
                    </span>
                  </SettingRow>
                  <SettingRow
                    title={t("settings.data.export")}
                  >
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={exporting}
                      onClick={() => void exportWorkspace()}
                    >
                      {exporting ? (
                        <LoaderCircle size={13} className="animate-spin" />
                      ) : (
                        <Download size={13} />
                      )}
                      {t("settings.data.export")}
                    </Button>
                  </SettingRow>
                </SettingsGroup>
              ) : activeSection === "shortcuts" ? (
                <SettingsGroup className="px-4 py-3">
                  <ShortcutList />
                </SettingsGroup>
              ) : (
                <SettingsGroup>
                  <SettingRow
                    title={APP_NAME}
                    description={t("settings.about.tagline")}
                  >
                    <span className="text-[13px] tabular-nums text-ink-secondary">
                      {t("settings.about.version", { version: APP_VERSION })}
                    </span>
                  </SettingRow>
                  <SettingRow
                    title={t("settings.about.backend")}
                    description={
                      backendHealth === "offline"
                        ? undefined
                        : backendHealth
                        ? t("settings.about.backendAgents", {
                            count: backendHealth.agents,
                          })
                        : undefined
                    }
                  >
                    {backendHealth === "offline" ? (
                      <span className="text-[13px] text-danger">
                        {t("settings.about.backendOffline")}
                      </span>
                    ) : backendHealth ? (
                      <span className="text-[13px] text-ink-secondary">
                        {t("settings.about.backendOnline", {
                          hours: (backendHealth.uptimeSeconds / 3600).toFixed(
                            1,
                          ),
                        })}
                      </span>
                    ) : (
                      <span className="text-[13px] text-ink-muted">—</span>
                    )}
                  </SettingRow>
                </SettingsGroup>
              )}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>

      <ConfirmDialog
        open={providerToRemove !== null}
        title={t("settings.provider.delete")}
        description={
          providerToRemove
            ? t("settings.provider.deleteConfirm", {
                name: providerToRemove.name,
              })
            : undefined
        }
        confirmLabel={t("settings.provider.delete")}
        tone="danger"
        busy={removingProvider !== null}
        onConfirm={() => void removeProvider()}
        onOpenChange={(open) => {
          if (!open && removingProvider === null) setProviderToRemove(null);
        }}
      />
    </Dialog.Root>
  );
}

/** 供应商列表行:名称 + 状态徽标 + 摘要,整行可点进详情。 */
function ProviderListRow({
  provider,
  onOpen,
}: {
  provider: ProviderInfo;
  onOpen: () => void;
}) {
  const { t } = useTranslation();
  const configured = providerConfigured(provider);
  const modelCount = providerModels(provider).length;
  const summary = provider.is_local
    ? t("settings.provider.localReady")
    : configured
    ? provider.api_key || provider.base_url
    : provider.require_api_key
    ? t("settings.provider.notConfigured")
    : t("settings.provider.keyNotRequired");
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex w-full items-center gap-3 rounded-[var(--radius-sm)] px-3 py-2 text-left transition-colors hover:bg-fill-hover"
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[13px] text-ink">
            {providerDisplayName(provider.name)}
          </span>
          {provider.is_local ? (
            <Badge tone="neutral">{t("settings.provider.local")}</Badge>
          ) : configured ? (
            <Badge tone="ok">{t("settings.provider.configured")}</Badge>
          ) : null}
          {provider.is_custom && (
            <Badge tone="neutral">{t("settings.provider.custom")}</Badge>
          )}
        </div>
        <div className="mt-0.5 truncate text-xs text-ink-tertiary">
          {summary}
        </div>
      </div>
      {modelCount > 0 && (
        <span className="shrink-0 text-xs tabular-nums text-ink-tertiary">
          {t("settings.provider.modelCount", { count: modelCount })}
        </span>
      )}
      <ChevronRight size={14} className="shrink-0 text-ink-tertiary" />
    </button>
  );
}

/** 端点说哪种协议:/chat/completions 还是 Codex 用的 /responses。 */
function ProtocolPicker({
  value,
  disabled,
  onChange,
}: {
  value: ChatModelName;
  disabled?: boolean;
  onChange: (value: ChatModelName) => void;
}) {
  const { t } = useTranslation();
  const labels: Record<(typeof CUSTOM_PROVIDER_PROTOCOLS)[number], string> = {
    OpenAIChatModel: t("settings.create.protocolChat"),
    OpenAIResponseModel: t("settings.create.protocolResponses"),
  };
  return (
    <div
      aria-disabled={disabled || undefined}
      className={cn(disabled && "pointer-events-none opacity-40")}
    >
      <SegmentedControl
        variant="track"
        value={value}
        options={CUSTOM_PROVIDER_PROTOCOLS.map((item) => ({
          value: item,
          label: labels[item],
        }))}
        onChange={onChange}
      />
    </div>
  );
}

/** 供应商详情:连接(key/URL/测试/保存/清除)+ 模型管理 inline。 */
function ProviderDetail({
  provider,
  activeModelId,
  keyDraft,
  urlDraft,
  protocolDraft,
  testState,
  saving,
  clearingKey,
  discovering,
  addingModel,
  busy,
  newModelId,
  newModelName,
  removingModel,
  onBack,
  onKeyDraft,
  onUrlDraft,
  onProtocolDraft,
  onSave,
  onClearKey,
  onTest,
  onDiscover,
  onNewModelId,
  onNewModelName,
  onAddModel,
  onRemoveModel,
  onRemoveProvider,
}: {
  provider: ProviderInfo;
  activeModelId: string | null;
  keyDraft: string;
  urlDraft: string;
  protocolDraft: ChatModelName;
  testState: TestState;
  saving: boolean;
  clearingKey: boolean;
  discovering: boolean;
  addingModel: boolean;
  busy: boolean;
  newModelId: string;
  newModelName: string;
  removingModel: string | null;
  onBack: () => void;
  onKeyDraft: (value: string) => void;
  onUrlDraft: (value: string) => void;
  onProtocolDraft: (value: ChatModelName) => void;
  onSave: () => void;
  onClearKey: () => void;
  onTest: () => void;
  onDiscover: () => void;
  onNewModelId: (value: string) => void;
  onNewModelName: (value: string) => void;
  onAddModel: () => void;
  onRemoveModel: (modelId: string) => void;
  onRemoveProvider: () => void;
}) {
  const { t } = useTranslation();
  const models = providerModels(provider);
  const hasKey = Boolean(provider.api_key);
  // 协议只在自定义供应商上可切,且只在它当前就是 OpenAI 家族协议时展示——
  // 免得把一个 Anthropic/Gemini 自定义供应商误改成两选一里的某个。
  const canPickProtocol =
    provider.is_custom &&
    (CUSTOM_PROVIDER_PROTOCOLS as readonly string[]).includes(
      provider.chat_model,
    );
  const protocolChanged =
    canPickProtocol && protocolDraft !== provider.chat_model;
  const dirty =
    Boolean(keyDraft.trim()) ||
    protocolChanged ||
    (!provider.freeze_url &&
      urlDraft.trim() !== "" &&
      urlDraft.trim() !== provider.base_url);
  // 没有任何可用端点(草稿和已存 URL 都空)时测试无意义。
  const canTest = Boolean(urlDraft.trim() || provider.base_url);

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ChevronLeft size={14} />
          {t("settings.provider.backToList")}
        </Button>
        <div className="min-w-0 flex-1" />
        {provider.is_custom && (
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            onClick={onRemoveProvider}
          >
            <Trash2 size={13} className="text-danger" />
            <span className="text-danger">{t("settings.provider.delete")}</span>
          </Button>
        )}
      </div>

      <SettingsGroup>
        <SettingRow
          title={providerDisplayName(provider.name)}
          description={
            provider.is_local
              ? t("settings.provider.localReady")
              : provider.base_url || undefined
          }
        >
          {provider.is_local ? (
            <Badge tone="neutral">{t("settings.provider.local")}</Badge>
          ) : providerConfigured(provider) ? (
            <Badge tone="ok">{t("settings.provider.configured")}</Badge>
          ) : (
            <span className="text-xs text-ink-tertiary">
              {t("settings.provider.notConfigured")}
            </span>
          )}
        </SettingRow>
      </SettingsGroup>

      {!provider.is_local && (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (dirty && !busy) onSave();
          }}
        >
          <SettingsGroup>
            <SettingRow
              title={t("settings.provider.apiKey")}
              description={
                hasKey
                  ? t("settings.provider.keySaved")
                  : !provider.require_api_key
                  ? t("settings.provider.keyOptional")
                  : undefined
              }
            >
              <div className="flex w-64 max-w-full items-center gap-1.5">
                <Input
                  type="password"
                  value={keyDraft}
                  disabled={busy}
                  onChange={(event) => onKeyDraft(event.target.value)}
                  placeholder={
                    hasKey
                      ? provider.api_key
                      : t("settings.provider.apiKeyPlaceholder")
                  }
                  aria-label={t("settings.provider.apiKey")}
                  autoComplete="off"
                  className="w-full"
                />
                {hasKey && !keyDraft && (
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={busy}
                    title={t("settings.provider.clearKeyTitle")}
                    onClick={onClearKey}
                  >
                    {clearingKey ? (
                      <LoaderCircle size={13} className="animate-spin" />
                    ) : (
                      t("settings.provider.clearKey")
                    )}
                  </Button>
                )}
              </div>
            </SettingRow>
            <SettingRow
              title={t("settings.provider.baseUrl")}
              description={
                provider.freeze_url
                  ? t("settings.provider.baseUrlFrozen")
                  : undefined
              }
            >
              <Input
                type="url"
                value={urlDraft}
                disabled={provider.freeze_url || busy}
                aria-label={t("settings.provider.baseUrl")}
                onChange={(event) => onUrlDraft(event.target.value)}
                className="w-64 max-w-full"
              />
            </SettingRow>
            {canPickProtocol && (
              <SettingRow
                title={t("settings.create.protocol")}
                description={t("settings.create.protocolDescription")}
              >
                <ProtocolPicker
                  value={protocolDraft}
                  disabled={busy}
                  onChange={onProtocolDraft}
                />
              </SettingRow>
            )}
            <SettingRow
              title={t("settings.provider.test")}
              description={
                testState.phase === "ok" ? (
                  <span className="text-ok">
                    {t("settings.provider.testOk")}
                  </span>
                ) : testState.phase === "fail" ? (
                  <span className="text-danger">{testState.message}</span>
                ) : undefined
              }
            >
              <div className="flex items-center gap-2">
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!canTest || testState.phase === "busy" || busy}
                  onClick={onTest}
                >
                  {testState.phase === "busy" ? (
                    <LoaderCircle size={13} className="animate-spin" />
                  ) : (
                    <PlugZap size={13} />
                  )}
                  {t("settings.provider.test")}
                </Button>
                <Button
                  type="submit"
                  variant="secondary"
                  size="sm"
                  disabled={!dirty || busy}
                >
                  {saving ? (
                    <LoaderCircle size={13} className="animate-spin" />
                  ) : null}
                  {t("settings.provider.save")}
                </Button>
              </div>
            </SettingRow>
          </SettingsGroup>
        </form>
      )}

      <SettingsGroup className="p-2">
        <div className="flex flex-wrap items-center justify-between gap-2 px-2 pb-2 pt-1">
          <div>
            <div className="text-[13px] font-medium text-ink">
              {t("settings.provider.modelsTitle")}
              {models.length > 0 && (
                <span className="ml-1.5 text-xs font-normal text-ink-tertiary">
                  {models.length}
                </span>
              )}
            </div>
          </div>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy}
            onClick={onDiscover}
          >
            {discovering ? (
              <LoaderCircle size={13} className="animate-spin" />
            ) : (
              <Radar size={13} />
            )}
            {t("settings.models.discover")}
          </Button>
        </div>
        <div className="max-h-72 overflow-y-auto rounded-[var(--radius-sm)] border border-line bg-surface">
          {models.length === 0 ? (
            <div className="px-3 py-3 text-xs text-ink-tertiary">
              {t("settings.models.noModels")}
            </div>
          ) : (
            [
              ...provider.models.map((item) => ({ item, builtin: true })),
              ...provider.extra_models.map((item) => ({
                item,
                builtin: false,
              })),
            ].map(({ item, builtin }) => (
              <div
                key={item.id}
                className="flex items-center gap-2 border-b border-line px-3 py-2 last:border-b-0"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] text-ink">
                    {item.name || item.id}
                  </div>
                  {item.name && item.name !== item.id && (
                    <div className="truncate font-mono text-[11px] text-ink-tertiary">
                      {item.id}
                    </div>
                  )}
                </div>
                {item.id === activeModelId && (
                  <Badge tone="neutral">
                    {t("settings.models.currentBadge")}
                  </Badge>
                )}
                {builtin ? (
                  <span className="text-[11px] text-ink-tertiary">
                    {t("settings.models.builtinBadge")}
                  </span>
                ) : (
                  <IconButton
                    size="sm"
                    tone="danger"
                    disabled={busy}
                    title={t("settings.models.removeModel")}
                    onClick={() => onRemoveModel(item.id)}
                  >
                    {removingModel === item.id ? (
                      <LoaderCircle size={14} className="animate-spin" />
                    ) : (
                      <Trash2 size={14} />
                    )}
                  </IconButton>
                )}
              </div>
            ))
          )}
        </div>
        <form
          className="mt-2 flex flex-wrap items-center gap-2 px-1 pb-1"
          onSubmit={(event) => {
            event.preventDefault();
            onAddModel();
          }}
        >
          <Input
            value={newModelId}
            disabled={busy}
            placeholder={t("settings.models.addModelIdPlaceholder")}
            aria-label={t("settings.models.addModelId")}
            onChange={(event) => onNewModelId(event.target.value)}
            className="min-w-44 flex-1"
          />
          <Input
            value={newModelName}
            disabled={busy}
            placeholder={t("settings.models.addModelNamePlaceholder")}
            aria-label={t("settings.models.addModelName")}
            onChange={(event) => onNewModelName(event.target.value)}
            className="min-w-44 flex-1"
          />
          <Button
            type="submit"
            variant="secondary"
            size="sm"
            disabled={busy || !newModelId.trim()}
          >
            {addingModel ? (
              <LoaderCircle size={13} className="animate-spin" />
            ) : (
              <Plus size={13} />
            )}
            {t("settings.models.addModelConfirm")}
          </Button>
        </form>
      </SettingsGroup>
    </div>
  );
}

/** 新建自定义供应商(OpenAI 兼容):名称 + Base URL + 可选 key。 */
function ProviderCreate({
  name,
  baseUrl,
  apiKey,
  protocol,
  creating,
  onName,
  onBaseUrl,
  onApiKey,
  onProtocol,
  onBack,
  onSubmit,
}: {
  name: string;
  baseUrl: string;
  apiKey: string;
  protocol: ChatModelName;
  creating: boolean;
  onName: (value: string) => void;
  onBaseUrl: (value: string) => void;
  onApiKey: (value: string) => void;
  onProtocol: (value: ChatModelName) => void;
  onBack: () => void;
  onSubmit: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ChevronLeft size={14} />
          {t("settings.provider.backToList")}
        </Button>
      </div>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
      >
        <SettingsGroup>
          <SettingRow
            title={t("settings.create.name")}
          >
            <Input
              autoFocus
              value={name}
              disabled={creating}
              placeholder={t("settings.create.namePlaceholder")}
              aria-label={t("settings.create.name")}
              onChange={(event) => onName(event.target.value)}
              className="w-64 max-w-full"
            />
          </SettingRow>
          <SettingRow title={t("settings.provider.baseUrl")}>
            <Input
              type="url"
              value={baseUrl}
              disabled={creating}
              placeholder={t("settings.create.baseUrlPlaceholder")}
              aria-label={t("settings.provider.baseUrl")}
              onChange={(event) => onBaseUrl(event.target.value)}
              className="w-64 max-w-full"
            />
          </SettingRow>
          <SettingRow
            title={t("settings.create.protocol")}
            description={t("settings.create.protocolDescription")}
          >
            <ProtocolPicker
              value={protocol}
              disabled={creating}
              onChange={onProtocol}
            />
          </SettingRow>
          <SettingRow title={t("settings.create.apiKeyOptional")}>
            <Input
              type="password"
              value={apiKey}
              disabled={creating}
              autoComplete="off"
              placeholder={t("settings.provider.apiKeyPlaceholder")}
              aria-label={t("settings.create.apiKeyOptional")}
              onChange={(event) => onApiKey(event.target.value)}
              className="w-64 max-w-full"
            />
          </SettingRow>
          <div className="flex items-center justify-end gap-2 border-t border-line px-4 py-3">
            <Button
              variant="ghost"
              size="sm"
              disabled={creating}
              onClick={onBack}
            >
              {t("common.cancel")}
            </Button>
            <Button
              type="submit"
              variant="secondary"
              size="sm"
              disabled={creating || !name.trim() || !baseUrl.trim()}
            >
              {creating ? (
                <LoaderCircle size={13} className="animate-spin" />
              ) : (
                <Plus size={13} />
              )}
              {t("settings.create.submit")}
            </Button>
          </div>
        </SettingsGroup>
      </form>
    </div>
  );
}

/** WorkBuddy 式分组卡：比面板略暗的底色，内部行以 border-t 分隔。 */
function SettingsGroup({
  className,
  children,
}: {
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-[var(--radius-md)] bg-bg",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** 设置行：左侧「项名 + 一行说明」，右侧控件。 */
function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t border-line px-4 py-3 first:border-t-0">
      <div className="min-w-0">
        <div className="text-[13px] text-ink">{title}</div>
        {description && (
          <div className="mt-0.5 text-xs leading-5 text-ink-tertiary">
            {description}
          </div>
        )}
      </div>
      <div className="ml-auto shrink-0">{children}</div>
    </div>
  );
}

/** 列表排序:已配置 > 本地 > 其余预置;组内保持后端顺序。 */
function sortProviders(providers: ProviderInfo[]): ProviderInfo[] {
  const rank = (provider: ProviderInfo) =>
    providerConfigured(provider) ? 0 : provider.is_local ? 1 : 2;
  return [...providers].sort((a, b) => rank(a) - rank(b));
}

function providerModels(provider: ProviderInfo): ModelInfo[] {
  const seen = new Set<string>();
  return [...provider.models, ...provider.extra_models].filter((model) => {
    if (seen.has(model.id)) return false;
    seen.add(model.id);
    return true;
  });
}

/** 内置 provider 的协议名保持不变，只在用户可见层切换产品品牌。 */
function providerDisplayName(name: string): string {
  return name === "QwenPaw Local" ? "Potato Local" : name;
}

/** 后端 id 约束:字母数字开头,只允许 [A-Za-z0-9._-],最长 64。 */
function slugifyProviderId(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^[._-]+/, "")
    .slice(0, 64)
    .replace(/-+$/, "");
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
