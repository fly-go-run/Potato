import * as Dialog from "@radix-ui/react-dialog";
import {
  Bot,
  KeyRound,
  LoaderCircle,
  Palette,
  Plus,
  Puzzle,
  Radar,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import {
  Badge,
  Button,
  IconButton,
  Input,
  SegmentedControl,
  Select,
  SkeletonRows,
  Switch,
} from "../components/ui";
import { cn } from "../lib/cn";
import {
  modelApi,
  providerReady,
  settingsApi,
  type ModelInfo,
  type ProviderInfo,
  type SandboxStatus,
} from "../lib/api";
import { pluginApi, skillApi } from "../lib/capabilities";
import { useTranslation, type TranslationKey } from "../lib/i18n";
import {
  getThemePreference,
  setThemePreference,
  type ThemePreference,
} from "../lib/theme";
import { useChatStore } from "../stores/chat";

type SectionId =
  | "models"
  | "provider"
  | "appearance"
  | "capabilities"
  | "security";

const SECTION_LABELS: Record<SectionId, TranslationKey> = {
  models: "settings.nav.models",
  provider: "settings.nav.provider",
  appearance: "settings.nav.appearance",
  capabilities: "settings.nav.capabilities",
  security: "settings.nav.security",
};

export function SettingsView() {
  const { language, setLanguage, t } = useTranslation();
  const navigate = useNavigate();
  const { activeModel, loadActiveModel } = useChatStore();
  const [section, setSection] = useState<SectionId>("models");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [providerId, setProviderId] = useState("");
  // 凭证配置与当前活动模型解耦：可以先配置任意供应商，再去模型页选择。
  const [credentialProviderId, setCredentialProviderId] = useState("");
  const [modelId, setModelId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [theme, setTheme] = useState<ThemePreference>(
    getThemePreference(),
  );
  const [loading, setLoading] = useState(true);
  const [savingModel, setSavingModel] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [addModelOpen, setAddModelOpen] = useState(false);
  const [addingModel, setAddingModel] = useState(false);
  const [newModelId, setNewModelId] = useState("");
  const [newModelName, setNewModelName] = useState("");
  const [removingModel, setRemovingModel] = useState<string | null>(null);
  const [savingProvider, setSavingProvider] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [capabilitySummary, setCapabilitySummary] = useState({
    enabled: 0,
    skills: 0,
    plugins: 0,
  });
  const [sandbox, setSandbox] = useState<SandboxStatus | null>(null);
  const [savingSandbox, setSavingSandbox] = useState(false);
  // 面板是弹层：关闭时优先退回来路，直达 /settings 时回首页。
  const [canGoBack] = useState(() => {
    const state = window.history.state as { idx?: number } | null;
    return typeof state?.idx === "number" && state.idx > 0;
  });

  useEffect(() => {
    let active = true;
    void Promise.all([modelApi.list(), modelApi.active()])
      .then(([providerList, activeInfo]) => {
        if (!active) return;
        setProviders(providerList);
        const initialProvider =
          providerList.find(
            (provider) =>
              provider.id === activeInfo.active_llm?.provider_id,
          ) ?? providerList[0];
        const models = initialProvider
          ? providerModels(initialProvider)
          : [];
        setProviderId(initialProvider?.id ?? "");
        setCredentialProviderId(initialProvider?.id ?? "");
        setModelId(
          activeInfo.active_llm?.provider_id === initialProvider?.id
            ? activeInfo.active_llm?.model ?? models[0]?.id ?? ""
            : models[0]?.id ?? "",
        );
        setBaseUrl(initialProvider?.base_url ?? "");
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setError(
          t("settings.loadFailed", { message: readableError(reason) }),
        );
        setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [t]);

  useEffect(() => {
    let active = true;
    void Promise.all([skillApi.list(), pluginApi.list()])
      .then(([skills, plugins]) => {
        if (!active) return;
        setCapabilitySummary({
          enabled: skills.filter((skill) => skill.enabled).length,
          skills: skills.length,
          plugins: plugins.length,
        });
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
    void settingsApi
      .sandboxStatus()
      .then((status) => {
        if (active) setSandbox(status);
      })
      .catch(() => {
        // Sandbox section is hidden when the endpoint is unavailable.
      });
    return () => {
      active = false;
    };
  }, []);

  const closePanel = () => {
    if (canGoBack) navigate(-1);
    else navigate("/");
  };

  const toggleSandbox = async () => {
    if (!sandbox || savingSandbox) return;
    setSavingSandbox(true);
    setError(null);
    setNotice(null);
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

  const provider = providers.find((item) => item.id === providerId) ?? null;
  const credentialProvider =
    providers.find((item) => item.id === credentialProviderId) ?? null;
  const models = useMemo(
    () => (provider ? providerModels(provider) : []),
    [provider],
  );

  const chooseProvider = (nextProviderId: string) => {
    setProviderId(nextProviderId);
    setModelId(
      activeModel?.active_llm?.provider_id === nextProviderId
        ? activeModel.active_llm.model
        : "",
    );
    setNotice(null);
    setError(null);
  };

  const chooseCredentialProvider = (nextProviderId: string) => {
    const nextProvider =
      providers.find((item) => item.id === nextProviderId) ?? null;
    setCredentialProviderId(nextProviderId);
    setApiKey("");
    setBaseUrl(nextProvider?.base_url ?? "");
    setNotice(null);
    setError(null);
  };

  const activateModel = async (pid: string, mid: string) => {
    if (!pid || !mid) return;
    const previous = activeModel?.active_llm;
    setSavingModel(true);
    setError(null);
    setNotice(null);
    try {
      await modelApi.setActive(pid, mid);
      await loadActiveModel();
      setProviderId(pid);
      setModelId(mid);
      setNotice(t("settings.models.saved"));
    } catch (reason) {
      // Keep the controls truthful when the server rejects the switch.
      // The draft provider/model must never masquerade as the active model.
      if (previous) {
        setProviderId(previous.provider_id);
        setModelId(previous.model);
      } else {
        setModelId("");
      }
      setError(readableError(reason));
    } finally {
      setSavingModel(false);
    }
  };

  const refreshProviders = async () => {
    const providerList = await modelApi.list();
    setProviders(providerList);
    return providerList;
  };

  const discoverModels = async () => {
    if (!providerId) return;
    setDiscovering(true);
    setError(null);
    setNotice(null);
    try {
      const result = await modelApi.discover(providerId);
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
    if (!providerId || !id) return;
    setAddingModel(true);
    setError(null);
    setNotice(null);
    try {
      await modelApi.addModel(providerId, {
        id,
        name: newModelName.trim() || id,
      });
      await refreshProviders();
      await activateModel(providerId, id);
      setAddModelOpen(false);
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
    if (!providerId) return;
    setRemovingModel(modelIdToRemove);
    setError(null);
    try {
      await modelApi.removeModel(providerId, modelIdToRemove);
      const providerList = await refreshProviders();
      // Only the real active pair needs a fallback; a draft selection must
      // not mutate the active model just because it was removed.
      const activePair = activeModel?.active_llm;
      if (
        activePair?.provider_id === providerId &&
        activePair.model === modelIdToRemove
      ) {
        const current = providerList.find((item) => item.id === providerId);
        const remaining = current ? providerModels(current) : [];
        const fallback = remaining[0]?.id ?? "";
        setModelId(fallback);
        if (fallback) await activateModel(providerId, fallback);
        else setModelId("");
      }
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setRemovingModel(null);
    }
  };

  const saveProvider = async () => {
    if (!credentialProvider) return;
    const trimmedKey = apiKey.trim();
    const nextBaseUrl = baseUrl.trim();
    const baseUrlChanged =
      !credentialProvider.freeze_url && nextBaseUrl !== credentialProvider.base_url;
    if (!trimmedKey && !baseUrlChanged) return;
    setSavingProvider(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await modelApi.configure(credentialProvider.id, {
        ...(trimmedKey ? { api_key: trimmedKey } : {}),
        ...(baseUrlChanged ? { base_url: nextBaseUrl } : {}),
      });
      setProviders((items) =>
        items.map((item) => (item.id === updated.id ? updated : item)),
      );
      setApiKey("");
      setBaseUrl(updated.base_url);
      setNotice(t("settings.provider.saved"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingProvider(false);
    }
  };

  const chooseTheme = (preference: ThemePreference) => {
    setTheme(preference);
    setThemePreference(preference);
  };

  const navItems: { id: SectionId; icon: ReactNode }[] = [
    { id: "models", icon: <Bot size={16} /> },
    { id: "provider", icon: <KeyRound size={16} /> },
    { id: "appearance", icon: <Palette size={16} /> },
    { id: "capabilities", icon: <Puzzle size={16} /> },
    // 沙箱端点不可用时（同旧版）整个安全分区不出现
    ...(sandbox ? [{ id: "security" as const, icon: <ShieldCheck size={16} /> }] : []),
  ];
  const activeSection: SectionId = navItems.some((item) => item.id === section)
    ? section
    : "models";

  const activePair = activeModel?.active_llm;
  const selectedIsActive = Boolean(
    activePair &&
      activePair.provider_id === providerId &&
      activePair.model === modelId,
  );
  const modelHint: ReactNode = savingModel ? (
    <span className="flex items-center gap-1.5">
      <LoaderCircle size={12} className="animate-spin" />
      {t("settings.models.applying")}
    </span>
  ) : selectedIsActive && activeModel?.effective_max_input_length ? (
    t("settings.models.contextWindow", {
      count: activeModel.effective_max_input_length.toLocaleString(),
    })
  ) : providerId && !selectedIsActive ? (
    t("settings.models.chooseModelToApply")
  ) : undefined;

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
            // 随内容收缩：固定 85vh 会让「模型/外观」这类短分区留下大片空腔
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
                  <span className={selected ? "text-ink" : "text-ink-muted"}>
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

            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5 pt-0">
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
                <SettingsGroup>
                  <SettingRow title={t("settings.models.provider")}>
                    <div className="w-56 max-w-full">
                      <Select
                        value={providerId}
                        onChange={(event) => chooseProvider(event.target.value)}
                        aria-label={t("settings.models.provider")}
                      >
                        {!providerId && (
                          <option value="">
                            {t("settings.models.chooseProvider")}
                          </option>
                        )}
                        {providers.map((item) => (
                          <option key={item.id} value={item.id}>
                            {providerDisplayName(item.name)}
                          </option>
                        ))}
                      </Select>
                    </div>
                  </SettingRow>

                  <SettingRow
                    title={t("settings.models.model")}
                    description={modelHint}
                  >
                    <div className="w-56 max-w-full">
                      <Select
                        value={modelId}
                        disabled={models.length === 0 || savingModel}
                        aria-label={t("settings.models.model")}
                        onChange={(event) => {
                          const next = event.target.value;
                          void activateModel(providerId, next);
                        }}
                      >
                        {!modelId && (
                          <option value="">
                            {t("settings.models.chooseModel")}
                          </option>
                        )}
                        {models.length === 0 ? (
                          <option value="">
                            {t("settings.models.noModels")}
                          </option>
                        ) : (
                          models.map((model) => (
                            <option key={model.id} value={model.id}>
                              {model.name || model.id}
                            </option>
                          ))
                        )}
                      </Select>
                    </div>
                  </SettingRow>

                  <SettingRow
                    title={t("settings.models.discover")}
                    description={t("settings.models.discoverDescription")}
                  >
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={!providerId || discovering}
                      onClick={() => void discoverModels()}
                    >
                      {discovering ? (
                        <LoaderCircle size={13} className="animate-spin" />
                      ) : (
                        <Radar size={13} />
                      )}
                      {t("settings.models.discover")}
                    </Button>
                  </SettingRow>

                  <SettingRow
                    title={t("settings.models.manageModels")}
                    description={t("settings.models.manageModelsDescription")}
                  >
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={!providerId}
                      onClick={() => setAddModelOpen(true)}
                    >
                      <SlidersHorizontal size={13} />
                      {t("settings.models.manageModels")}
                    </Button>
                  </SettingRow>
                </SettingsGroup>
              ) : activeSection === "provider" ? (
                <div className="space-y-3">
                  <SettingsGroup className="p-2">
                    <div className="px-2 pb-2 pt-1">
                      <div className="text-[13px] font-medium text-ink">
                        {t("settings.provider.configuredTitle")}
                      </div>
                      <div className="mt-0.5 text-xs leading-5 text-ink-tertiary">
                        {t("settings.provider.configuredDescription")}
                      </div>
                    </div>
                    {providers.filter(providerReady).length > 0 ? (
                      <div className="space-y-1">
                        {providers.filter(providerReady).map((item) => (
                          <div
                            key={item.id}
                            className={cn(
                              "flex items-center gap-3 rounded-[var(--radius-sm)] px-3 py-2",
                              item.id === credentialProviderId
                                ? "bg-fill-active"
                                : "hover:bg-fill-hover",
                            )}
                          >
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-2">
                                <span className="truncate text-[13px] text-ink">
                                  {item.name}
                                </span>
                                <Badge tone="ok">
                                  {t("settings.provider.configured")}
                                </Badge>
                              </div>
                              <div className="mt-0.5 truncate text-xs text-ink-tertiary">
                                {item.is_local
                                  ? t("settings.provider.localReady")
                                  : item.api_key ||
                                    t("settings.provider.keyNotRequired")}
                              </div>
                            </div>
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={savingProvider}
                              onClick={() => chooseCredentialProvider(item.id)}
                            >
                              {t("settings.provider.edit")}
                            </Button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="rounded-[var(--radius-sm)] bg-surface px-3 py-3 text-xs text-ink-tertiary">
                        {t("settings.provider.noneConfigured")}
                      </div>
                    )}
                  </SettingsGroup>

                  <SettingsGroup>
                    <SettingRow
                      title={t("settings.provider.add")}
                      description={t("settings.provider.providerDescription")}
                    >
                      <div className="w-56 max-w-full">
                        <Select
                          value={credentialProviderId}
                          disabled={savingProvider}
                          onChange={(event) =>
                            chooseCredentialProvider(event.target.value)
                          }
                          aria-label={t("settings.provider.add")}
                        >
                          <option value="">
                            {t("settings.provider.chooseProvider")}
                          </option>
                          {providers.map((item) => (
                            <option key={item.id} value={item.id}>
                              {providerDisplayName(item.name)}
                              {providerReady(item)
                                ? ` · ${t("settings.provider.configured")}`
                                : ""}
                            </option>
                          ))}
                        </Select>
                      </div>
                    </SettingRow>
                    <SettingRow
                      title={t("settings.provider.apiKey")}
                      description={
                        credentialProvider && !credentialProvider.require_api_key
                          ? t("settings.provider.keyOptional")
                          : undefined
                      }
                    >
                      <div className="flex w-56 max-w-full flex-col items-end gap-1">
                        <Input
                          type="password"
                          value={apiKey}
                          disabled={!credentialProvider || savingProvider}
                          onChange={(event) => setApiKey(event.target.value)}
                          onBlur={() => void saveProvider()}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") {
                              event.preventDefault();
                              void saveProvider();
                            }
                          }}
                          placeholder={t("settings.provider.apiKeyPlaceholder")}
                          aria-label={t("settings.provider.apiKey")}
                          autoComplete="off"
                          className="w-full"
                        />
                        <span className="text-[11px] text-ink-muted">
                          {savingProvider
                            ? t("settings.provider.saving")
                            : t("settings.provider.autoSaveHint")}
                        </span>
                      </div>
                    </SettingRow>
                    <SettingRow
                      title={t("settings.provider.baseUrl")}
                      description={
                        credentialProvider?.freeze_url
                          ? t("settings.provider.baseUrlFrozen")
                          : undefined
                      }
                    >
                      <Input
                        type="url"
                        value={baseUrl}
                        disabled={
                          !credentialProvider ||
                          credentialProvider.freeze_url ||
                          savingProvider
                        }
                        aria-label={t("settings.provider.baseUrl")}
                        onChange={(event) => setBaseUrl(event.target.value)}
                        onBlur={() => void saveProvider()}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            void saveProvider();
                          }
                        }}
                        className="w-56 max-w-full"
                      />
                    </SettingRow>
                  </SettingsGroup>
                </div>
              ) : activeSection === "appearance" ? (
                <SettingsGroup>
                  <SettingRow
                    title={t("settings.appearance.theme")}
                    description={t("settings.appearance.description")}
                  >
                    <SegmentedControl
                      variant="track"
                      value={theme}
                      options={[
                        { value: "light", label: t("settings.theme.light") },
                        { value: "dark", label: t("settings.theme.dark") },
                        { value: "system", label: t("settings.theme.system") },
                      ]}
                      onChange={chooseTheme}
                    />
                  </SettingRow>
                  <SettingRow
                    title={t("settings.language.title")}
                    description={t("settings.language.description")}
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
                </SettingsGroup>
              ) : activeSection === "capabilities" ? (
                <SettingsGroup>
                  <SettingRow
                    title={t("settings.capabilities.title")}
                    description={t("settings.capabilities.summary", {
                      enabled: capabilitySummary.enabled,
                      skills: capabilitySummary.skills,
                      plugins: capabilitySummary.plugins,
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
                </SettingsGroup>
              ) : sandbox ? (
                <SettingsGroup>
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
                </SettingsGroup>
              ) : null}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>

      <Dialog.Root open={addModelOpen} onOpenChange={setAddModelOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
          <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 w-[calc(100%-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-lg)] border border-line bg-raised p-5 shadow-[var(--shadow-lg)] outline-none">
            <Dialog.Title className="text-sm font-semibold text-ink">
              {t("settings.models.manageModelsTitle", {
                provider: provider?.name || providerId,
              })}
            </Dialog.Title>
            <Dialog.Description className="mt-1.5 text-sm text-ink-secondary">
              {t("settings.models.manageModelsDescription")}
            </Dialog.Description>

            <div className="mt-4 max-h-64 overflow-y-auto rounded-[var(--radius-md)] border border-line">
              {provider &&
                [
                  ...provider.models.map((item) => ({ item, builtin: true })),
                  ...provider.extra_models.map((item) => ({
                    item,
                    builtin: false,
                  })),
                ].map(({ item, builtin }) => (
                  <div
                    key={item.id}
                    className="group flex items-center gap-2 border-b border-line px-3 py-2 last:border-b-0"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] text-ink">
                        {item.name || item.id}
                      </div>
                      {item.name && item.name !== item.id && (
                        <div className="truncate font-mono text-[11px] text-ink-muted">
                          {item.id}
                        </div>
                      )}
                    </div>
                    {item.id === modelId && (
                      <Badge tone="neutral">
                        {t("settings.models.currentBadge")}
                      </Badge>
                    )}
                    {builtin ? (
                      <span className="text-[11px] text-ink-muted">
                        {t("settings.models.builtinBadge")}
                      </span>
                    ) : (
                      <IconButton
                        size="sm"
                        tone="danger"
                        disabled={removingModel !== null}
                        title={t("settings.models.removeModel")}
                        onClick={() => void removeModel(item.id)}
                      >
                        {removingModel === item.id ? (
                          <LoaderCircle
                            size={14}
                            className="animate-spin"
                          />
                        ) : (
                          <Trash2 size={14} />
                        )}
                      </IconButton>
                    )}
                  </div>
                ))}
            </div>

            <form
              className="mt-4 space-y-3"
              onSubmit={(event) => {
                event.preventDefault();
                void addCustomModel();
              }}
            >
              <Input
                autoFocus
                value={newModelId}
                disabled={addingModel}
                placeholder={t("settings.models.addModelIdPlaceholder")}
                aria-label={t("settings.models.addModelId")}
                onChange={(event) => setNewModelId(event.target.value)}
              />
              <Input
                value={newModelName}
                disabled={addingModel}
                placeholder={t("settings.models.addModelNamePlaceholder")}
                aria-label={t("settings.models.addModelName")}
                onChange={(event) => setNewModelName(event.target.value)}
              />
              <div className="flex items-center justify-between gap-2 pt-2">
                <Button
                  type="submit"
                  variant="secondary"
                  size="sm"
                  disabled={addingModel || !newModelId.trim()}
                >
                  {addingModel ? (
                    <LoaderCircle size={13} className="animate-spin" />
                  ) : (
                    <Plus size={13} />
                  )}
                  {t("settings.models.addModelConfirm")}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={addingModel}
                  onClick={() => setAddModelOpen(false)}
                >
                  {t("settings.models.done")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </Dialog.Root>
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

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
