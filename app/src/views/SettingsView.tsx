import {
  Bot,
  Check,
  KeyRound,
  Languages,
  Palette,
  Puzzle,
  ShieldCheck,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
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
      <div className="flex h-full items-center justify-center text-sm text-ink-muted">
        {t("settings.loading")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto bg-surface">
      <div className="mx-auto max-w-3xl px-6 py-10 sm:px-10">
        <header className="mb-8">
          <h1 className="text-2xl font-medium tracking-tight text-ink">
            {t("settings.title")}
          </h1>
          <p className="mt-1 text-sm text-ink-muted">
            {t("settings.subtitle")}
          </p>
        </header>

        {(error || notice) && (
          <div
            className={`mb-5 rounded-md px-3 py-2 text-xs ${
              error
                ? "bg-danger-soft text-danger"
                : "bg-accent-soft text-accent"
            }`}
          >
            {error || notice}
          </div>
        )}

        <div className="divide-y divide-line rounded-lg border border-line bg-surface">
          <SettingsSection
            icon={<Bot size={17} />}
            title={t("settings.models.title")}
            description={t("settings.models.description")}
          >
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label={t("settings.models.provider")}>
                <select
                  value={providerId}
                  onChange={(event) => chooseProvider(event.target.value)}
                  className={inputClassName}
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
                </select>
              </Field>
              <Field label={t("settings.models.model")}>
                <select
                  value={modelId}
                  disabled={models.length === 0}
                  onChange={(event) => setModelId(event.target.value)}
                  className={inputClassName}
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
                </select>
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
              <button
                type="button"
                disabled={!providerId || !modelId || savingModel}
                onClick={() => void activateModel()}
                className="rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                {savingModel
                  ? t("settings.models.applying")
                  : t("settings.models.apply")}
              </button>
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
                <input
                  type="password"
                  value={apiKey}
                  disabled={!provider}
                  onChange={(event) => setApiKey(event.target.value)}
                  placeholder={t("settings.provider.apiKeyPlaceholder")}
                  autoComplete="off"
                  className={inputClassName}
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
                <input
                  type="url"
                  value={baseUrl}
                  disabled={!provider || provider.freeze_url}
                  onChange={(event) => setBaseUrl(event.target.value)}
                  className={inputClassName}
                />
              </Field>
            </div>
            <div className="mt-4 flex justify-end">
              <button
                type="button"
                disabled={!provider || savingProvider}
                onClick={() => void saveProvider()}
                className="rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink disabled:cursor-not-allowed disabled:opacity-40"
              >
                {savingProvider
                  ? t("settings.provider.saving")
                  : t("settings.provider.save")}
              </button>
            </div>
          </SettingsSection>

          <SettingsSection
            icon={<Palette size={17} />}
            title={t("settings.appearance.title")}
            description={t("settings.appearance.description")}
          >
            <ChoiceGroup
              value={theme}
              choices={[
                { value: "light", label: t("settings.theme.light") },
                { value: "dark", label: t("settings.theme.dark") },
                { value: "system", label: t("settings.theme.system") },
              ]}
              onChange={(value) => chooseTheme(value as ThemePreference)}
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
              <button
                type="button"
                onClick={() => navigate("/skills")}
                className="rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink"
              >
                {t("settings.capabilities.manage")}
              </button>
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
                <button
                  type="button"
                  role="switch"
                  aria-checked={sandbox.enabled}
                  disabled={savingSandbox}
                  onClick={() => void toggleSandbox()}
                  className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40 ${
                    sandbox.enabled ? "bg-accent" : "bg-line-strong"
                  }`}
                >
                  <span
                    className={`absolute left-0 top-0.5 h-4 w-4 rounded-full bg-surface shadow-sm transition-transform ${
                      sandbox.enabled
                        ? "translate-x-[1.125rem]"
                        : "translate-x-0.5"
                    }`}
                  />
                </button>
              </div>
            </SettingsSection>
          )}

          <SettingsSection
            icon={<Languages size={17} />}
            title={t("settings.language.title")}
            description={t("settings.language.description")}
          >
            <ChoiceGroup
              value={language}
              choices={[
                { value: "zh", label: t("settings.language.zh") },
                { value: "en", label: t("settings.language.en") },
              ]}
              onChange={(value) => setLanguage(value as "zh" | "en")}
            />
          </SettingsSection>
        </div>
      </div>
    </div>
  );
}

const inputClassName =
  "mt-1.5 block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none transition-colors focus:border-line-strong disabled:cursor-not-allowed disabled:bg-bubble-tool disabled:text-ink-muted";

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
          <span className="text-accent">{icon}</span>
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

function ChoiceGroup({
  value,
  choices,
  onChange,
}: {
  value: string;
  choices: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid auto-cols-fr grid-flow-col gap-2">
      {choices.map((choice) => {
        const selected = choice.value === value;
        return (
          <button
            key={choice.value}
            type="button"
            onClick={() => onChange(choice.value)}
            className={`flex min-w-0 items-center justify-center gap-1.5 rounded-md border px-3 py-2 text-xs font-medium transition-colors ${
              selected
                ? "border-accent bg-accent-soft text-accent"
                : "border-line text-ink-secondary hover:border-line-strong hover:text-ink"
            }`}
          >
            {selected && <Check size={13} />}
            {choice.label}
          </button>
        );
      })}
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

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
