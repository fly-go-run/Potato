import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  Check,
  ChevronDown,
  CirclePlus,
  LoaderCircle,
  Settings2,
} from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  modelApi,
  providerReady,
  type ActiveModel,
  type ProviderInfo,
} from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import { useChatStore } from "../../stores/chat";

/**
 * composer 内联模型选择(对标 Codex 的 composer 模型菜单):
 * 点模型名直接切换,不再跳设置页。列表懒加载,按 provider 分组,
 * 选择走 agent 作用域的 setActive,成功后刷新 store 的 activeModel。
 */
export function ModelPicker() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { activeModel, modelLoading, loadActiveModel } = useChatStore();
  const [providers, setProviders] = useState<ProviderInfo[] | null>(null);
  const [listLoading, setListLoading] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const [effortSaving, setEffortSaving] = useState<string | null>(null);
  const model = activeModel?.active_llm;

  const ensureList = () => {
    if (listLoading) return;
    // 每次打开都刷新:设置页新添加/发现的模型立即可见,列表很小不心疼
    setListLoading(true);
    modelApi
      .list()
      // 只列用户主动配置过的 provider(已存 key 的 + 本地运行的),
      // 未配置的聚合/云端 provider 列出来也调不通,只会添乱
      .then((items) =>
        setProviders(
          items.filter(providerReady),
        ),
      )
      .catch(() => setProviders([]))
      .finally(() => setListLoading(false));
  };

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
              ? `${model.provider_id} / ${model.model}`
              : t("composer.selectModel")
          }
          className="flex h-8 max-w-52 items-center gap-1 truncate rounded-full px-2.5 text-[13px] text-ink-secondary transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover data-[state=open]:bg-fill-hover"
        >
          <CirclePlus size={16} className="shrink-0 text-ink" strokeWidth={1.8} />
          <span className="truncate">
            {modelLoading
              ? t("composer.loadingModel")
              : model?.model || t("composer.noModel")}
          </span>
          <ChevronDown size={13} className="shrink-0 text-ink-muted" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          sideOffset={6}
          align="start"
          className="qp-pop z-50 max-h-80 min-w-56 overflow-y-auto rounded-[var(--radius-md)] border border-line bg-raised p-1 shadow-[var(--shadow-md)]"
        >
          {listLoading || providers === null ? (
            <div className="flex items-center gap-2 px-2 py-2 text-xs text-ink-muted">
              <LoaderCircle size={13} className="animate-spin" />
              {t("composer.modelListLoading")}
            </div>
          ) : providers.length === 0 ? (
            <div className="px-2 py-2 text-xs text-ink-muted">
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
                  <DropdownMenu.Label className="px-2 pb-1 pt-2 text-[11px] text-ink-muted">
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
                        className="flex cursor-default items-center justify-between gap-3 rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active data-[disabled]:opacity-60"
                      >
                        <span className="truncate">
                          {item.name || item.id}
                        </span>
                        {switching === key ? (
                          <LoaderCircle
                            size={13}
                            className="shrink-0 animate-spin text-ink-muted"
                          />
                        ) : active ? (
                          <Check size={13} className="shrink-0 text-accent" />
                        ) : null}
                      </DropdownMenu.Item>
                    );
                  })}
                </DropdownMenu.Group>
              );
            })
          )}
          <EffortSection
            providers={providers}
            active={model ?? null}
            saving={effortSaving}
            onPick={(effort) => void chooseEffort(effort)}
          />
          <DropdownMenu.Separator className="my-1 h-px bg-line" />
          <DropdownMenu.Item
            onSelect={() => navigate("/settings")}
            className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active"
          >
            <Settings2 size={13} />
            {t("composer.manageModels")}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

const EFFORT_LABELS: Record<string, string> = {
  none: "composer.effort.none",
  minimal: "composer.effort.minimal",
  low: "composer.effort.low",
  medium: "composer.effort.medium",
  high: "composer.effort.high",
  xhigh: "composer.effort.xhigh",
};

/** 当前模型支持 effort 风格思考配置时,在菜单尾部渲染档位单选。 */
function EffortSection({
  providers,
  active,
  saving,
  onPick,
}: {
  providers: ProviderInfo[] | null;
  active: ActiveModel | null;
  saving: string | null;
  onPick: (effort: string | null) => void;
}) {
  const { t } = useTranslation();
  if (!providers || !active) return null;
  const provider = providers.find((item) => item.id === active.provider_id);
  const info = provider
    ? [...provider.models, ...provider.extra_models].find(
        (item) => item.id === active.model,
      )
    : undefined;
  if (!provider || !info) return null;
  const style = info.thinking_param_style ?? provider.thinking_param_style;
  // 过渡策略:旧版打包后端不返回 style 元数据,但已持久化过 effort 的模型
  // 显然支持该参数,同样放行;届时档位表用 provider 默认并剔除 minimal
  // (探测确认 5.6 系不支持,且缺权威元数据时宁可少列不误列)。
  const qualifies = style === "effort" || info.reasoning_effort != null;
  if (!qualifies) return null;
  let options =
    info.reasoning_effort_options ??
    provider.reasoning_effort_options ??
    [];
  if (info.reasoning_effort_options == null && style !== "effort") {
    options = options.filter((item) => item !== "minimal");
  }
  if (options.length === 0) return null;
  const current = info.reasoning_effort ?? null;

  return (
    <>
      <DropdownMenu.Separator className="my-1 h-px bg-line" />
      <DropdownMenu.Label className="px-2 pb-1 pt-2 text-[11px] text-ink-muted">
        {t("composer.effort.title")}
      </DropdownMenu.Label>
      {options.map((option) => {
        const labelKey = EFFORT_LABELS[option];
        const selected = current === option;
        return (
          <DropdownMenu.Item
            key={option}
            disabled={saving !== null}
            onSelect={(event) => {
              event.preventDefault();
              if (!selected) onPick(option);
            }}
            className="flex cursor-default items-center justify-between gap-3 rounded-sm px-2 py-1.5 text-xs text-ink-secondary outline-none hover:bg-fill-hover focus:bg-fill-active data-[disabled]:opacity-60"
          >
            <span>{labelKey ? t(labelKey as never) : option}</span>
            {saving === option ? (
              <LoaderCircle
                size={13}
                className="shrink-0 animate-spin text-ink-muted"
              />
            ) : selected ? (
              <Check size={13} className="shrink-0 text-accent" />
            ) : null}
          </DropdownMenu.Item>
        );
      })}
    </>
  );
}
