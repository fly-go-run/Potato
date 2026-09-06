import * as Dialog from "@radix-ui/react-dialog";
import {
  Check,
  ChevronDown,
  Search,
  X,
  LoaderCircle,
  Settings,
} from "lucide-react";
import { useEffect, useState, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  modelApi,
  providerConfigured,
  type ActiveModel,
  type ProviderInfo,
} from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import { prettyModelName } from "../../lib/modelPresentation";
import { useChatStore } from "../../stores/chat";

const DEFAULT_REASONING_EFFORTS = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
];

const EFFORT_LABELS: Record<string, string> = {
  none: "composer.effort.none",
  minimal: "composer.effort.minimal",
  low: "composer.effort.low",
  medium: "composer.effort.medium",
  high: "composer.effort.high",
  max: "composer.effort.max",
  xhigh: "composer.effort.xhigh",
};

function findActiveModelInfo(
  providers: ProviderInfo[] | null,
  active: ActiveModel | null | undefined,
) {
  if (!providers || !active) return null;
  const provider = providers.find((item) => item.id === active.provider_id);
  if (!provider) return null;
  const info = [...provider.models, ...provider.extra_models].find(
    (item) => item.id === active.model,
  );
  return info ? { provider, info } : null;
}

function looksLikeReasoningModel(modelId: string): boolean {
  const normalized = modelId.toLowerCase();
  return (
    /\bgpt-5(?:[.\-:]|$)/.test(normalized) ||
    /(?:^|[/._:-])o(?:1|3|4)(?:[.\-:]|$)/.test(normalized) ||
    /(?:reasoner|reasoning|thinking)/.test(normalized) ||
    /deepseek-v4-.*pro/.test(normalized)
  );
}

function getReasoningEffortOptions(
  provider: ProviderInfo,
  info: ProviderInfo["models"][number],
): string[] {
  const style = info.thinking_param_style ?? provider.thinking_param_style;
  const supportsEffort =
    style === "effort" ||
    info.reasoning_effort != null ||
    (style == null && looksLikeReasoningModel(info.id));
  if (!supportsEffort) return [];

  const configuredOptions =
    info.reasoning_effort_options ?? provider.reasoning_effort_options;
  const options = configuredOptions?.length
    ? configuredOptions
    : DEFAULT_REASONING_EFFORTS;
  const normalized = [...new Set(options.filter(Boolean))];

  // For models detected by their ID, avoid offering the legacy "minimal"
  // value unless the backend explicitly advertised it for that model.
  if (style !== "effort" && info.reasoning_effort_options == null) {
    return normalized.filter((item) => item !== "minimal");
  }
  return normalized;
}

/**
 * Composer model and reasoning preferences:
 * 搜索面板按服务商平铺模型,底部编辑当前模型的思考深度。
 * 列表挂载即拉取(pill 需要显示当前思考深度),打开菜单时再刷新一次。
 */
export function ModelPicker() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const location = useLocation();
  const activeModel = useChatStore((state) => state.activeModel);
  const modelLoading = useChatStore((state) => state.modelLoading);
  const loadActiveModel = useChatStore((state) => state.loadActiveModel);
  const [providers, setProviders] = useState<ProviderInfo[] | null>(null);
  const [listLoading, setListLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [switching, setSwitching] = useState<string | null>(null);
  const [effortSaving, setEffortSaving] = useState<string | null>(null);
  const model = activeModel?.active_llm;
  const activeModelInfo = findActiveModelInfo(providers, model);
  const activeEffort = activeModelInfo?.info.reasoning_effort ?? null;
  const activeEffortOptions = activeModelInfo
    ? getReasoningEffortOptions(activeModelInfo.provider, activeModelInfo.info)
    : [];
  const effortText = (effort: string | null) => {
    if (effort === null) return t("composer.effort.defaultValue");
    const labelKey = EFFORT_LABELS[effort];
    return labelKey ? t(labelKey as never) : effort;
  };
  const activeEffortLabel =
    activeEffort && activeEffortOptions.includes(activeEffort)
      ? effortText(activeEffort)
      : null;

  const ensureList = () => {
    if (listLoading) return;
    // 每次打开都刷新:设置页新添加/发现的模型立即可见,列表很小不心疼
    setListLoading(true);
    setError(null);
    const activeProviderId = model?.provider_id;
    modelApi
      .list()
      // 只列设置里真正配置过的 provider。require_api_key=false 的
      // 聚合/免费 provider 即使有内置模型，也不代表用户选择过它。
      // 例外:本地 provider(Ollama 等)有模型即可用,没有 key 概念;
      // 当前激活的 provider 无论如何都要保留,否则 pill 显示不了状态。
      .then((items) =>
        setProviders(
          items.filter(
            (item) =>
              providerConfigured(item) ||
              (item.is_local &&
                item.models.length + item.extra_models.length > 0) ||
              item.id === activeProviderId,
          ),
        ),
      )
      .catch(() => setError(t("composer.modelListFailed")))
      .finally(() => setListLoading(false));
  };

  useEffect(() => {
    // 挂载即拉取:pill 上的思考深度依赖模型列表里的配置
    ensureList();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const choose = async (providerId: string, modelId: string) => {
    const key = `${providerId}/${modelId}`;
    setSwitching(key);
    setError(null);
    try {
      await modelApi.setActive(providerId, modelId);
      await loadActiveModel();
      setOpen(false);
    } catch {
      setError(t("composer.modelSwitchFailed"));
      await loadActiveModel().catch(() => undefined);
    } finally {
      setSwitching(null);
    }
  };

  const chooseEffort = async (effort: string | null) => {
    if (!model) return;
    setEffortSaving(effort ?? "__default__");
    setError(null);
    try {
      const updated = await modelApi.configureModel(
        model.provider_id,
        model.model,
        { reasoning_effort: effort },
      );
      setProviders((current) =>
        (current ?? []).map((item) =>
          item.id === updated.id ? updated : item,
        ),
      );
    } catch {
      setError(t("composer.effortSaveFailed"));
    } finally {
      setEffortSaving(null);
    }
  };

  const groups = (providers ?? []).map((provider) => ({
    provider,
    models: [...new Map([...provider.models, ...provider.extra_models].map((item) => [item.id, item])).values()]
      .filter((item) => `${provider.name} ${item.name} ${item.id}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))
      .sort((a, b) => Number(b.id === model?.model && provider.id === model.provider_id) - Number(a.id === model?.model && provider.id === model.provider_id)),
  })).filter((group) => group.models.length > 0)
    .sort((a, b) => Number(b.provider.id === model?.provider_id) - Number(a.provider.id === model?.provider_id));
  return (
    <Dialog.Root open={open} onOpenChange={(next) => {
      if (switching || effortSaving) return;
      setOpen(next);
      if (next) { setQuery(""); ensureList(); }
    }}>
      <Dialog.Trigger asChild>
        <button type="button" title={model ? `${model.provider_id} / ${model.model}` : t("composer.selectModel")} className="flex h-8 max-w-56 items-center gap-1.5 rounded-full px-2 text-[13px] text-ink hover:bg-fill-hover">
          <span className="truncate">{modelLoading ? t("composer.loadingModel") : model?.model ? prettyModelName(model.model) : t("composer.noModel")}</span>
          {activeEffortLabel && <span className="text-ink-tertiary">{activeEffortLabel}</span>}
          <ChevronDown size={14} className="shrink-0 text-ink-tertiary" />
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content onOpenAutoFocus={(event) => { event.preventDefault(); searchRef.current?.focus(); }} className="qp-pop fixed left-1/2 top-1/2 z-50 flex max-h-[min(38rem,85vh)] w-[min(28rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[var(--radius-lg)] border border-line bg-raised shadow-lg outline-none">
          <div className="flex items-center justify-between px-5 pt-4 pb-3">
            <Dialog.Title className="text-base font-semibold">{t("composer.selectModel")}</Dialog.Title>
            <Dialog.Description className="sr-only">{t("models.search")}</Dialog.Description>
            <Dialog.Close disabled={Boolean(switching || effortSaving)} aria-label={t("common.cancel")} className="rounded-md p-1 text-ink-secondary hover:bg-fill-hover"><X size={16} /></Dialog.Close>
          </div>
          <div className="mx-4 mb-3 flex items-center gap-2 rounded-lg border border-line px-3 focus-within:border-tint">
            <Search size={15} className="text-ink-tertiary" />
            <input ref={searchRef} value={query} onChange={(event) => setQuery(event.target.value)} aria-label={t("models.search")} placeholder={t("models.search")} className="h-10 min-w-0 flex-1 bg-transparent text-sm outline-none" onKeyDown={(event) => {
              if (event.key === "ArrowDown") { event.preventDefault(); event.currentTarget.closest('[role="dialog"]')?.querySelector<HTMLButtonElement>('[data-model-option]')?.focus(); }
            }} />
          </div>
          {error && <div role="alert" className="mx-4 mb-2 rounded-lg bg-danger-soft px-3 py-2 text-xs text-danger">{error}<button onClick={ensureList} className="ml-2 underline">{t("common.retry")}</button></div>}
          <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-2 pb-2" onKeyDown={(event) => {
            if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
            const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[data-model-option]:not(:disabled)'));
            const index = buttons.indexOf(event.target as HTMLButtonElement);
            if (index < 0) return;
            event.preventDefault();
            if (event.key === "ArrowUp" && index === 0) searchRef.current?.focus();
            else buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
          }}>
            {listLoading && providers === null ? <p className="p-4 text-sm text-ink-secondary">{t("composer.modelListLoading")}</p> : groups.length === 0 && !error ? <p className="p-4 text-sm text-ink-secondary">{t(query ? "models.noMatch" : "composer.modelListEmpty")}</p> : null}
            {groups.map(({provider, models}) => <section key={provider.id}>
              <h3 className="px-3 pt-3 pb-1 text-xs text-ink-tertiary">{provider.name}</h3>
              {models.map((item) => {
                const active = model?.provider_id === provider.id && model.model === item.id;
                const label = item.name && item.name !== item.id ? item.name : prettyModelName(item.id);
                const duplicate = models.some((other) => other.id !== item.id && (other.name && other.name !== other.id ? other.name : prettyModelName(other.id)) === label);
                return <button data-model-option key={item.id} type="button" title={item.id} aria-pressed={active} disabled={Boolean(switching || effortSaving)} onClick={() => active ? setOpen(false) : void choose(provider.id, item.id)} className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm disabled:opacity-50 ${active ? "bg-accent-soft text-accent" : "hover:bg-fill-hover"}`}>
                  <span className="min-w-0 flex-1 truncate">{duplicate ? item.id : label}</span>
                  {switching === `${provider.id}/${item.id}` ? <LoaderCircle size={14} className="animate-spin" /> : active ? <Check size={14} /> : null}
                </button>;
              })}
            </section>)}
          </div>
          {activeEffortOptions.length > 0 && <label className="flex items-center justify-between gap-3 border-t border-line px-5 py-3 text-sm">
            {t("composer.effort.title")}
            <select value={activeEffort ?? "__default__"} disabled={Boolean(effortSaving || switching)} onChange={(event) => void chooseEffort(event.target.value === "__default__" ? null : event.target.value)} className="rounded-md border border-line bg-surface px-2 py-1">
              <option value="__default__">{t("composer.effort.defaultValue")}</option>
              {activeEffortOptions.map((option) => <option key={option} value={option}>{effortText(option)}</option>)}
            </select>
          </label>}
          <button disabled={Boolean(switching || effortSaving)} onClick={() => { setOpen(false); navigate("/settings", {state:{background:location}}); }} className="flex items-center gap-2 border-t border-line px-5 py-3 text-sm text-ink-secondary hover:bg-fill-hover"><Settings size={14} />{t("composer.manageModels")}</button>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
