import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  Check,
  ChevronDown,
  ChevronRight,
  LoaderCircle,
  RotateCcw,
  Settings2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  modelApi,
  providerConfigured,
  type ActiveModel,
  type ProviderInfo,
} from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
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

const MENU_PANEL_CLASS =
  "qp-pop z-50 rounded-[var(--radius-lg)] border border-line bg-raised p-1.5 shadow-[var(--shadow-lg)]";

const TOP_ROW_CLASS =
  "flex w-full cursor-default select-none items-center justify-between gap-10 rounded-[var(--radius-sm)] px-3 py-2 text-sm text-ink outline-none hover:bg-fill-hover focus:bg-fill-hover data-[state=open]:bg-fill-hover data-[disabled]:opacity-50";

const SUB_ITEM_CLASS =
  "flex cursor-default items-center justify-between gap-4 rounded-[var(--radius-sm)] px-3 py-2 text-sm text-ink outline-none hover:bg-fill-hover focus:bg-fill-active data-[disabled]:opacity-60";

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
 * composer 内联模型选择(对标 Codex 的 composer 模型菜单):
 * 顶层是「模型 / 思考深度 / 恢复默认」的紧凑行,子菜单里做具体选择。
 * 列表挂载即拉取(pill 需要显示当前思考深度),打开菜单时再刷新一次。
 */
export function ModelPicker() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const activeModel = useChatStore((state) => state.activeModel);
  const modelLoading = useChatStore((state) => state.modelLoading);
  const loadActiveModel = useChatStore((state) => state.loadActiveModel);
  const [providers, setProviders] = useState<ProviderInfo[] | null>(null);
  const [listLoading, setListLoading] = useState(false);
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
      .catch(() => setProviders([]))
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
    try {
      await modelApi.setActive(providerId, modelId);
      await loadActiveModel();
    } catch {
      await loadActiveModel();
    } finally {
      setSwitching(null);
    }
  };

  const chooseEffort = async (effort: string | null) => {
    if (!model) return;
    setEffortSaving(effort ?? "__default__");
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
      /* 失败保持原样,下次打开菜单会重新拉取真实状态 */
    } finally {
      setEffortSaving(null);
    }
  };

  return (
    <DropdownMenu.Root onOpenChange={(open) => open && ensureList()}>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          title={
            model
              ? `${model.provider_id} / ${model.model}${
                  activeEffortLabel ? ` · ${activeEffortLabel}` : ""
                }`
              : t("composer.selectModel")
          }
          // 静止态不铺底色:输入框那一行里它是「当前状态」而不是主操作,
          // 常驻的灰色药丸会和右边的发送键抢视觉重量。悬停和展开时才上底,
          // 此时底色是在回应用户的动作,而不是一直在喊自己。
          className="flex h-8 max-w-56 items-center gap-1.5 truncate rounded-full px-3.5 text-[13px] text-ink transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover data-[state=open]:bg-fill-hover"
        >
          <span className="truncate">
            {modelLoading
              ? t("composer.loadingModel")
              : model?.model || t("composer.noModel")}
          </span>
          {activeEffortLabel ? (
            <span className="shrink-0 text-[13px] text-ink-muted">
              {activeEffortLabel}
            </span>
          ) : null}
          <ChevronDown size={14} className="shrink-0 text-ink-muted" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          sideOffset={6}
          align="start"
          className={`${MENU_PANEL_CLASS} min-w-64`}
        >
          {/* 模型 → 子菜单:按 provider 分组的模型列表 */}
          <DropdownMenu.Sub>
            <DropdownMenu.SubTrigger className={TOP_ROW_CLASS}>
              <span>{t("composer.menu.model")}</span>
              <span className="flex min-w-0 items-center gap-1 text-ink-muted">
                <span className="max-w-36 truncate">
                  {modelLoading
                    ? t("composer.loadingModel")
                    : model?.model || t("composer.noModel")}
                </span>
                <ChevronRight size={14} className="shrink-0" />
              </span>
            </DropdownMenu.SubTrigger>
            <DropdownMenu.Portal>
              <DropdownMenu.SubContent
                sideOffset={4}
                alignOffset={-4}
                className={`${MENU_PANEL_CLASS} max-h-96 min-w-56 overflow-y-auto`}
              >
                {listLoading && providers === null ? (
                  <div className="flex items-center gap-2 px-3 py-2 text-sm text-ink-muted">
                    <LoaderCircle size={14} className="animate-spin" />
                    {t("composer.modelListLoading")}
                  </div>
                ) : !providers || providers.length === 0 ? (
                  <div className="px-3 py-2 text-sm text-ink-muted">
                    {t("composer.modelListEmpty")}
                  </div>
                ) : (
                  providers.map((provider) => {
                    const models = [
                      ...provider.models,
                      ...provider.extra_models,
                    ];
                    if (models.length === 0) return null;
                    return (
                      <DropdownMenu.Group key={provider.id}>
                        <DropdownMenu.Label className="px-3 pb-1 pt-2 text-xs text-ink-muted">
                          {provider.name || provider.id}
                        </DropdownMenu.Label>
                        {models.map((item) => {
                          const key = `${provider.id}/${item.id}`;
                          const active =
                            model?.provider_id === provider.id &&
                            model?.model === item.id;
                          return (
                            <DropdownMenu.Item
                              key={key}
                              disabled={switching !== null}
                              onSelect={(event) => {
                                event.preventDefault();
                                if (!active) void choose(provider.id, item.id);
                              }}
                              className={SUB_ITEM_CLASS}
                            >
                              <span className="truncate">
                                {item.name || item.id}
                              </span>
                              {switching === key ? (
                                <LoaderCircle
                                  size={14}
                                  className="shrink-0 animate-spin text-ink-muted"
                                />
                              ) : active ? (
                                <Check
                                  size={13}
                                  className="shrink-0 text-accent"
                                />
                              ) : null}
                            </DropdownMenu.Item>
                          );
                        })}
                      </DropdownMenu.Group>
                    );
                  })
                )}
              </DropdownMenu.SubContent>
            </DropdownMenu.Portal>
          </DropdownMenu.Sub>

          {/* 思考深度 → 子菜单:档位单选,仅当前模型支持时显示 */}
          {activeEffortOptions.length > 0 ? (
            <DropdownMenu.Sub>
              <DropdownMenu.SubTrigger className={TOP_ROW_CLASS}>
                <span>{t("composer.effort.title")}</span>
                <span className="flex items-center gap-1 text-ink-muted">
                  <span>{effortText(activeEffort)}</span>
                  <ChevronRight size={14} className="shrink-0" />
                </span>
              </DropdownMenu.SubTrigger>
              <DropdownMenu.Portal>
                <DropdownMenu.SubContent
                  sideOffset={4}
                  alignOffset={-4}
                  className={`${MENU_PANEL_CLASS} min-w-44`}
                >
                  <DropdownMenu.Label className="px-3 pb-1 pt-1.5 text-xs text-ink-muted">
                    {t("composer.effort.title")}
                  </DropdownMenu.Label>
                  {activeEffortOptions.map((option) => {
                    const selected = activeEffort === option;
                    return (
                      <DropdownMenu.Item
                        key={option}
                        disabled={effortSaving !== null}
                        onSelect={(event) => {
                          event.preventDefault();
                          if (!selected) void chooseEffort(option);
                        }}
                        className={SUB_ITEM_CLASS}
                      >
                        <span>{effortText(option)}</span>
                        {effortSaving === option ? (
                          <LoaderCircle
                            size={14}
                            className="shrink-0 animate-spin text-ink-muted"
                          />
                        ) : selected ? (
                          <Check size={15} className="shrink-0 text-ink" />
                        ) : null}
                      </DropdownMenu.Item>
                    );
                  })}
                </DropdownMenu.SubContent>
              </DropdownMenu.Portal>
            </DropdownMenu.Sub>
          ) : null}

          {activeEffortOptions.length > 0 ? (
            <>
              <DropdownMenu.Separator className="mx-3 my-1.5 h-px bg-line" />
              {/* 恢复默认:清掉本模型的思考深度覆盖,回到后端默认档 */}
              <DropdownMenu.Item
                disabled={effortSaving !== null || activeEffort === null}
                onSelect={(event) => {
                  event.preventDefault();
                  if (activeEffort !== null) void chooseEffort(null);
                }}
                className={`${TOP_ROW_CLASS} text-ink-secondary`}
              >
                <span>{t("composer.effort.reset")}</span>
                {effortSaving === "__default__" ? (
                  <LoaderCircle
                    size={13}
                    className="shrink-0 animate-spin text-ink-muted"
                  />
                ) : (
                  <RotateCcw size={14} className="shrink-0 text-ink-muted" />
                )}
              </DropdownMenu.Item>
            </>
          ) : null}

          <DropdownMenu.Separator className="mx-3 my-1.5 h-px bg-line" />
          <DropdownMenu.Item
            onSelect={() =>
              navigate("/settings", { state: { background: location } })
            }
            className="flex cursor-default items-center gap-2 rounded-[var(--radius-sm)] px-3 py-2 text-sm text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active"
          >
            <Settings2 size={14} />
            {t("composer.manageModels")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
