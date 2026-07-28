import {
  Bot,
  KeyRound,
  Languages,
  Palette,
  Puzzle,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Button,
  Card,
  Input,
  PageContainer,
  PageHeader,
  SegmentedControl,
  Select,
  SkeletonRows,
  Switch,
} from "../components/ui";
import {
  modelApi,
  settingsApi,
  type ModelInfo,
  type ProviderInfo,
  type SandboxStatus,
} from "../lib/api";
import { pluginApi, skillApi } from "../lib/capabilities";
import { useTranslation } from "../lib/i18n";
import {
  getThemePreference,
  setThemePreference,
  type ThemePreference,
} from "../lib/theme";
import { useChatStore } from "../stores/chat";

export function SettingsView() {
  const { language, setLanguage, t } = useTranslation();
  const navigate = useNavigate();
  const { activeModel, loadActiveModel } = useChatStore();
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [providerId, setProviderId] = useState("");
  const [modelId, setModelId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [theme, setTheme] = useState<ThemePreference>(
    getThemePreference(),
  );
  const [loading, setLoading] = useState(true);
  const [savingModel, setSavingModel] = useState(false);
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
  const models = useMemo(
    () => (provider ? providerModels(provider) : []),
    [provider],
  );

  const chooseProvider = (nextProviderId: string) => {
    const nextProvider =
      providers.find((item) => item.id === nextProviderId) ?? null;
    const nextModels = nextProvider ? providerModels(nextProvider) : [];
    setProviderId(nextProviderId);
    setModelId(
      activeModel?.active_llm?.provider_id === nextProviderId
        ? activeModel.active_llm.model
        : nextModels[0]?.id ?? "",
    );
    setApiKey("");
    setBaseUrl(nextProvider?.base_url ?? "");
    setNotice(null);
    setError(null);
  };

  const activateModel = async () => {
    if (!providerId || !modelId) return;
    setSavingModel(true);
    setError(null);
    setNotice(null);
    try {
      await modelApi.setActive(providerId, modelId);
      await loadActiveModel();
      setNotice(t("settings.models.saved"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSavingModel(false);
    }
  };

  const saveProvider = async () => {
    if (!provider) return;
    setSavingProvider(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await modelApi.configure(provider.id, {
        ...(apiKey ? { api_key: apiKey } : {}),
        ...(!provider.freeze_url ? { base_url: baseUrl.trim() } : {}),
      });
      setProviders((items) =>
        items.map((item) => (item.id === updated.id ? updated : item)),
      );
      setApiKey("");
      setBaseUrl(updated.base_url);
      await loadActiveModel();
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

  if (loading) {
    return (
      <PageContainer width="reading">
        <PageHeader
          title={t("settings.title")}
          subtitle={t("settings.subtitle")}
        />
        <Card className="p-5">
          <SkeletonRows rows={5} />
        </Card>
      </PageContainer>
    );
  }

  return (
    <PageContainer width="reading">
        <PageHeader
          title={t("settings.title")}
          subtitle={t("settings.subtitle")}
        />

        {(error || notice) && (
          <div
            className={`mb-5 rounded-md px-3 py-2 text-xs ${
              error
                ? "bg-danger-soft text-danger"
                : "bg-fill-active text-ok"
            }`}
          >
            {error || notice}
          </div>
        )}

        <Card className="divide-y divide-line">
          <SettingsSection
            icon={<Bot size={17} />}
            title={t("settings.models.title")}
            description={t("settings.models.description")}
          >
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label={t("settings.models.provider")}>
                <Select
                  value={providerId}
                  onChange={(event) => chooseProvider(event.target.value)}
                  className="mt-1.5"
                >
                  {!providerId && (
                    <option value="">
                      {t("settings.models.chooseProvider")}
                    </option>
                  )}
                  {providers.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={t("settings.models.model")}>
                <Select
                  value={modelId}
                  disabled={models.length === 0}
                  onChange={(event) => setModelId(event.target.value)}
                  className="mt-1.5"
                >
                  {models.length === 0 ? (
                    <option value="">{t("settings.models.noModels")}</option>
                  ) : (
                    models.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.name || model.id}
                      </option>
                    ))
                  )}
                </Select>
              </Field>
            </div>

            <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
              <div className="text-xs text-ink-muted">
                <span>{t("settings.models.active")}: </span>
                <span className="font-medium text-ink-secondary">
                  {activeModel?.active_llm
                    ? `${activeModel.active_llm.provider_id} / ${activeModel.active_llm.model}`
                    : t("settings.models.notConfigured")}
                </span>
                {activeModel?.effective_max_input_length && (
                  <span className="ml-2">
                    ·{" "}
                    {t("settings.models.contextWindow", {
                      count:
                        activeModel.effective_max_input_length.toLocaleString(),
                    })}
                  </span>
                )}
              </div>
              <Button
                variant="primary"
                size="sm"
                disabled={!providerId || !modelId || savingModel}
                onClick={() => void activateModel()}
              >
                {savingModel
                  ? t("settings.models.applying")
                  : t("settings.models.apply")}
              </Button>
            </div>
          </SettingsSection>

          <SettingsSection
            icon={<KeyRound size={17} />}
            title={t("settings.provider.title")}
            description={t("settings.provider.description")}
          >
            <div className="grid gap-4 sm:grid-cols-2">
              <Field
                label={t("settings.provider.apiKey")}
                hint={
                  provider && !provider.require_api_key
                    ? t("settings.provider.keyOptional")
                    : undefined
                }
              >
                <Input
                  type="password"
                  value={apiKey}
                  disabled={!provider}
                  onChange={(event) => setApiKey(event.target.value)}
                  placeholder={t("settings.provider.apiKeyPlaceholder")}
                  autoComplete="off"
                  className="mt-1.5"
                />
              </Field>
              <Field
                label={t("settings.provider.baseUrl")}
                hint={
                  provider?.freeze_url
                    ? t("settings.provider.baseUrlFrozen")
                    : undefined
                }
              >
                <Input
                  type="url"
                  value={baseUrl}
                  disabled={!provider || provider.freeze_url}
                  onChange={(event) => setBaseUrl(event.target.value)}
                  className="mt-1.5"
                />
              </Field>
            </div>
            <div className="mt-4 flex justify-end">
              <Button
                variant="secondary"
                size="sm"
                disabled={!provider || savingProvider}
                onClick={() => void saveProvider()}
              >
                {savingProvider
                  ? t("settings.provider.saving")
                  : t("settings.provider.save")}
              </Button>
            </div>
          </SettingsSection>

          <SettingsSection
            icon={<Palette size={17} />}
            title={t("settings.appearance.title")}
            description={t("settings.appearance.description")}
          >
            <SegmentedControl
              value={theme}
              options={[
                { value: "light", label: t("settings.theme.light") },
                { value: "dark", label: t("settings.theme.dark") },
                { value: "system", label: t("settings.theme.system") },
              ]}
              onChange={chooseTheme}
            />
          </SettingsSection>

          <SettingsSection
            icon={<Puzzle size={17} />}
            title={t("settings.capabilities.title")}
            description={t("settings.capabilities.description")}
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <p className="text-sm text-ink-secondary">
                {t("settings.capabilities.summary", {
                  enabled: capabilitySummary.enabled,
                  skills: capabilitySummary.skills,
                  plugins: capabilitySummary.plugins,
                })}
              </p>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => navigate("/skills")}
              >
                {t("settings.capabilities.manage")}
              </Button>
            </div>
          </SettingsSection>

          {sandbox && (
            <SettingsSection
              icon={<ShieldCheck size={17} />}
              title={t("settings.sandbox.title")}
              description={t("settings.sandbox.description")}
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <p className="text-sm text-ink-secondary">
                    {t("settings.sandbox.label")}
                  </p>
                  <p className="mt-1 text-xs leading-5 text-ink-muted">
                    {sandbox.enabled && !sandbox.effective
                      ? t(
                          sandbox.reason === "unsupported"
                            ? "settings.sandbox.unsupported"
                            : "settings.sandbox.notAdmin",
                        )
                      : sandbox.enabled
                        ? t("settings.sandbox.on")
                        : t("settings.sandbox.off")}
                  </p>
                </div>
                <Switch
                  checked={sandbox.enabled}
                  disabled={savingSandbox}
                  onChange={() => void toggleSandbox()}
                  aria-label={t("settings.sandbox.label")}
                />
              </div>
            </SettingsSection>
          )}

          <SettingsSection
            icon={<Languages size={17} />}
            title={t("settings.language.title")}
            description={t("settings.language.description")}
          >
            <SegmentedControl
              value={language}
              options={[
                { value: "zh", label: t("settings.language.zh") },
                { value: "en", label: t("settings.language.en") },
              ]}
              onChange={setLanguage}
            />
          </SettingsSection>
        </Card>
    </PageContainer>
  );
}

function SettingsSection({
  icon,
  title,
  description,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="grid gap-5 px-5 py-6 sm:grid-cols-[11rem_minmax(0,1fr)] sm:px-6">
      <div>
        <div className="flex items-center gap-2 font-medium text-ink">
          <span className="text-ink-muted">{icon}</span>
          <h2>{title}</h2>
        </div>
        <p className="mt-1.5 text-xs leading-5 text-ink-muted">
          {description}
        </p>
      </div>
      <div className="min-w-0">{children}</div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block text-xs font-medium text-ink-secondary">
      {label}
      {children}
      {hint && (
        <span className="mt-1 block font-normal text-ink-muted">{hint}</span>
      )}
    </label>
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

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
