import * as Dialog from "@radix-ui/react-dialog";
import {
  Blocks,
  ChevronRight,
  Download,
  LoaderCircle,
  PackageOpen,
  Plus,
  Puzzle,
  Search,
  Tags,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Banner } from "../components/ui/Banner";
import {
  Badge,
  Button,
  Card,
  ConfirmDialog,
  EmptyState,
  IconButton,
  Input,
  PageContainer,
  PageHeader,
  SegmentedControl,
  SkeletonRows,
  Switch,
} from "../components/ui";
import {
  mergeCatalogInstalled,
  pluginApi,
  pluginToolCount,
  pollHubInstall,
  runOptimisticSkillToggle,
  skillApi,
  type CatalogPlugin,
  type HubInstallTask,
  type HubSkillInfo,
  type PluginInfo,
  type PoolSkillInfo,
  type SkillInfo,
  type WorkspaceSkillSummary,
} from "../lib/capabilities";
import { presentError, type ErrorPresentation } from "../lib/errorPresentation";
import { useTranslation } from "../lib/i18n";
import {
  skillDescription,
  skillDisplayName,
  skillSearchHaystack,
} from "../lib/skillPresentation";

type MainTab = "skills" | "plugins";
type SkillSourceTab = "pool" | "hub" | "upload";
type PluginSourceTab = "catalog" | "url" | "upload";
type ActiveHubTask = HubInstallTask & { skillName: string };

export function SkillsView() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<MainTab>("skills");
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<ErrorPresentation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [busySkills, setBusySkills] = useState<Set<string>>(() => new Set());
  const [busyPlugin, setBusyPlugin] = useState<string | null>(null);
  const [selectedSkill, setSelectedSkill] = useState<SkillInfo | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<
    | { type: "skill"; item: SkillInfo }
    | { type: "plugin"; item: PluginInfo }
    | null
  >(null);

  const load = async () => {
    setLoading(true);
    setLoadError(null);
    setError(null);
    try {
      const [skillItems, pluginItems] = await Promise.all([
        skillApi.list(),
        pluginApi.list(),
      ]);
      setSkills(skillItems);
      setPlugins(pluginItems);
    } catch (reason) {
      setLoadError(presentError(reason));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const filteredSkills = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return skills;
    return skills.filter((skill) =>
      `${skillSearchHaystack(skill.name)} ${(
        skill.tags ?? []
      ).join(" ")}`
        .toLocaleLowerCase()
        .includes(needle),
    );
  }, [query, skills]);

  const toggleSkill = async (skill: SkillInfo) => {
    const enabled = !skill.enabled;
    setBusySkills((current) => new Set(current).add(skill.name));
    setError(null);
    setNotice(null);
    setSelectedSkill((current) =>
      current?.name === skill.name ? { ...current, enabled } : current,
    );
    try {
      await runOptimisticSkillToggle({
        skills,
        name: skill.name,
        enabled,
        onUpdate: (update) => setSkills(update),
        mutate: () => skillApi.setEnabled(skill.name, enabled),
      });
    } catch (reason) {
      setSelectedSkill((current) =>
        current?.name === skill.name
          ? { ...current, enabled: skill.enabled }
          : current,
      );
      setError(
        t("skills.toggleFailed", {
          name: skill.name,
          message: readableError(reason),
        }),
      );
    } finally {
      setBusySkills((current) => {
        const next = new Set(current);
        next.delete(skill.name);
        return next;
      });
    }
  };

  const deleteSkill = async (skill: SkillInfo) => {
    setBusySkills((current) => new Set(current).add(skill.name));
    setError(null);
    try {
      await skillApi.delete(skill.name);
      setSkills((items) => items.filter((item) => item.name !== skill.name));
      setSelectedSkill(null);
      setNotice(t("skills.deleted", { name: skill.name }));
      setPendingDelete(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusySkills((current) => {
        const next = new Set(current);
        next.delete(skill.name);
        return next;
      });
    }
  };

  const deletePlugin = async (plugin: PluginInfo) => {
    setBusyPlugin(plugin.id);
    setError(null);
    try {
      await pluginApi.delete(plugin.id);
      setPlugins((items) => items.filter((item) => item.id !== plugin.id));
      setNotice(t("plugins.deleted", { name: plugin.name }));
      setPendingDelete(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyPlugin(null);
    }
  };

  return (
    <>
      <PageContainer width="wide">
        <PageHeader
          title={t("skills.title")}
          subtitle={t("skills.subtitle")}
          actions={
            <Button
              variant="primary"
              size="sm"
              onClick={() => setAddOpen(true)}
            >
              <Plus size={15} />
              {t("skills.add")}
            </Button>
          }
        />

        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <SegmentedControl
            value={tab}
            options={[
              {
                value: "skills",
                label: t("skills.tab.skills"),
                count: skills.length,
              },
              {
                value: "plugins",
                label: t("skills.tab.plugins"),
                count: plugins.length,
              },
            ]}
            onChange={(value) => {
              setTab(value);
              setQuery("");
              setError(null);
              setNotice(null);
            }}
          />
          {tab === "skills" && (
            <SearchField value={query} onChange={setQuery} />
          )}
        </div>

        {loadError && (
          <section
            role="alert"
            className="mt-4 flex items-start gap-3 rounded-md bg-danger-soft px-3 py-3 text-xs text-danger"
          >
            <div className="min-w-0 flex-1">
              <p className="font-medium">{t("skills.loadFailedTitle")}</p>
              <p className="mt-1">{t(loadError.summaryKey)}</p>
              {loadError.detail && (
                <details className="mt-2 text-danger/80">
                  <summary className="cursor-pointer">
                    {t("common.technicalDetail")}
                  </summary>
                  <pre className="mt-1 whitespace-pre-wrap break-words font-mono text-[11px]">
                    {loadError.detail}
                  </pre>
                </details>
              )}
            </div>
            <Button variant="secondary" size="sm" onClick={() => void load()}>
              {t("common.retry")}
            </Button>
          </section>
        )}
        {error && !loadError && (
          <Banner tone="danger" onDismiss={() => setError(null)}>
            {error}
          </Banner>
        )}
        {notice && (
          <div
            role="status"
            className="mt-4 rounded-md bg-fill-active px-3 py-2 text-xs text-ok"
          >
            {notice}
          </div>
        )}

        {loading ? (
          <Card className="mt-5 p-4">
            <SkeletonRows rows={6} />
          </Card>
        ) : loadError ? null : tab === "skills" ? (
          <section className="mt-5">
            {filteredSkills.length === 0 ? (
              <EmptyState
                icon={<PackageOpen size={20} />}
                title={t(query ? "skills.noResults" : "skills.empty")}
                description={t(
                  query
                    ? "skills.noResultsDescription"
                    : "skills.emptyDescription",
                )}
              />
            ) : (
              <div className="grid grid-cols-1 gap-x-6 gap-y-0.5 border-t border-line pt-2 sm:grid-cols-2">
                {filteredSkills.map((skill) => (
                  <SkillRow
                    key={skill.name}
                    skill={skill}
                    busy={busySkills.has(skill.name)}
                    onOpen={() => setSelectedSkill(skill)}
                    onToggle={() => void toggleSkill(skill)}
                  />
                ))}
              </div>
            )}
          </section>
        ) : plugins.length === 0 ? (
          <div className="mt-5">
            <EmptyState
              icon={<Puzzle size={20} />}
              title={t("plugins.empty")}
              description={t("plugins.emptyDescription")}
            />
          </div>
        ) : (
          <Card className="mt-5 divide-y divide-line overflow-hidden">
            {plugins.map((plugin) => (
              <PluginRow
                key={plugin.id}
                plugin={plugin}
                busy={busyPlugin === plugin.id}
                onDelete={() =>
                  setPendingDelete({ type: "plugin", item: plugin })
                }
              />
            ))}
          </Card>
        )}
      </PageContainer>

      <SkillDetails
        skill={selectedSkill}
        busy={selectedSkill ? busySkills.has(selectedSkill.name) : false}
        onOpenChange={(open) => !open && setSelectedSkill(null)}
        onDelete={(skill) => setPendingDelete({ type: "skill", item: skill })}
        onToggle={(skill) => void toggleSkill(skill)}
      />
      <AddCapabilityDialog
        open={addOpen}
        mode={tab}
        installedSkills={skills}
        installedPlugins={plugins}
        onOpenChange={setAddOpen}
        onNotice={setNotice}
        onChanged={async (message) => {
          setNotice(message);
          await load();
        }}
      />
      <ConfirmDialog
        open={pendingDelete !== null}
        title={
          pendingDelete?.type === "plugin"
            ? t("plugins.uninstall")
            : t("skills.delete")
        }
        description={
          pendingDelete?.type === "plugin"
            ? t("plugins.deleteConfirm", { name: pendingDelete.item.name })
            : pendingDelete?.type === "skill"
            ? t("skills.deleteConfirm", { name: pendingDelete.item.name })
            : undefined
        }
        tone="danger"
        busy={busySkills.size > 0 || busyPlugin !== null}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        onConfirm={() => {
          if (pendingDelete?.type === "skill")
            void deleteSkill(pendingDelete.item);
          if (pendingDelete?.type === "plugin")
            void deletePlugin(pendingDelete.item);
        }}
      />
    </>
  );
}

function SkillRow({
  skill,
  busy,
  onOpen,
  onToggle,
}: {
  skill: SkillInfo;
  busy: boolean;
  onOpen: () => void;
  onToggle: () => void;
}) {
  const { t, language } = useTranslation();
  return (
    // 行主体是原生 button；Switch 作为兄弟节点浮在其上（而非嵌套 button），
    // 既保住语义又避免 switch 的点击冒泡到行展开。
    <div className="group relative">
      <button
        type="button"
        onClick={onOpen}
        title={skill.name}
        className="flex w-full items-center gap-3.5 rounded-[var(--radius-md)] py-3 pl-4 pr-20 text-left outline-none transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover active:bg-fill-active focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span
          className={`grid h-9 w-9 shrink-0 place-items-center rounded-[10px] border border-line bg-bubble-tool ${
            skill.enabled ? "text-icon" : "text-ink-muted"
          }`}
        >
          {skill.emoji || <Blocks size={18} />}
        </span>
        <div className="min-w-0 flex-1">
          <span
            className={`block truncate text-sm font-medium ${
              skill.enabled ? "text-ink" : "text-ink-secondary"
            }`}
          >
            {skillDisplayName(skill.name, language)}
          </span>
          <p className="line-clamp-2 text-[13px] leading-5 text-ink-tertiary">
            {skillDescription(skill.name, language) ||
              t("skills.noDescription")}
          </p>
        </div>
      </button>
      <div className="pointer-events-none absolute inset-y-0 right-4 flex items-center gap-2.5">
        <span className="pointer-events-auto flex items-center">
          <Switch
            checked={skill.enabled}
            disabled={busy}
            onChange={onToggle}
            aria-label={t("skills.toggleLabel", {
              name: skillDisplayName(skill.name, language),
            })}
          />
        </span>
        <ChevronRight
          size={14}
          aria-hidden
          className="text-icon transition-colors duration-[var(--dur-fast)] group-hover:text-icon-strong"
        />
      </div>
    </div>
  );
}

function PluginRow({
  plugin,
  busy,
  onDelete,
}: {
  plugin: PluginInfo;
  busy: boolean;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const toolCount = pluginToolCount(plugin);
  const source = plugin.installed_from || plugin.source;
  return (
    <div className="flex items-center gap-3.5 px-4 py-3 transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover">
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[10px] border border-line bg-bubble-tool text-icon">
        <Puzzle size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
          <span className="truncate text-sm font-medium text-ink">
            {plugin.name}
          </span>
          {plugin.version && (
            <Badge tone="neutral" className="shrink-0">
              {plugin.version}
            </Badge>
          )}
          {source && (
            <Badge tone="neutral" className="max-w-32 truncate">
              {source}
            </Badge>
          )}
          {toolCount !== null && (
            <span className="text-[11px] text-ink-tertiary">
              · {t("plugins.tools", { count: toolCount })}
            </span>
          )}
        </div>
        <p className="line-clamp-2 text-[13px] leading-5 text-ink-tertiary">
          {plugin.description || t("skills.noDescription")}
        </p>
      </div>
      <Button variant="danger" size="sm" disabled={busy} onClick={onDelete}>
        {busy ? (
          <LoaderCircle size={14} className="animate-spin" />
        ) : (
          <Trash2 size={14} />
        )}
        {t("plugins.uninstall")}
      </Button>
    </div>
  );
}

function SkillDetails({
  skill,
  busy,
  onOpenChange,
  onDelete,
  onToggle,
}: {
  skill: SkillInfo | null;
  busy: boolean;
  onOpenChange: (open: boolean) => void;
  onDelete: (skill: SkillInfo) => void;
  onToggle: (skill: SkillInfo) => void;
}) {
  const { t, language } = useTranslation();
  return (
    <Dialog.Root open={skill !== null} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-drawer fixed inset-y-0 right-0 z-50 flex w-[min(29rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            {/* emoji 只在详情里出现；列表行统一线稿，保证整列图标一致。 */}
            <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[10px] border border-line bg-bubble-tool text-base text-icon">
              {skill?.emoji || <Blocks size={18} />}
            </span>
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {skill ? skillDisplayName(skill.name, language) : ""}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-tertiary">
                {t("skills.detailsDescription")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton size="sm" title={t("skills.close")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            <div className="mb-6 flex items-center justify-between rounded-[var(--radius-md)] border border-line bg-surface px-4 py-3">
              <span className="text-sm text-ink">
                {t("skills.enableLabel")}
              </span>
              <span className="flex items-center gap-2">
                {busy && (
                  <LoaderCircle
                    size={14}
                    className="animate-spin text-ink-tertiary"
                  />
                )}
                <Switch
                  checked={skill?.enabled ?? false}
                  disabled={!skill || busy}
                  onChange={() => skill && onToggle(skill)}
                  aria-label={
                    skill
                      ? t("skills.toggleLabel", {
                          name: skillDisplayName(skill.name, language),
                        })
                      : t("skills.enableLabel")
                  }
                />
              </span>
            </div>
            <Detail label={t("skills.description")}>
              <p className="whitespace-pre-wrap text-sm leading-6 text-ink-secondary">
                {skill
                  ? skillDescription(skill.name, language) ||
                    t("skills.noDescription")
                  : t("skills.noDescription")}
              </p>
            </Detail>
            <div className="mt-6 grid grid-cols-2 gap-4">
              <Detail label={t("skills.version")}>
                <span className="text-sm text-ink-secondary">
                  {skill?.version_text || t("skills.unknown")}
                </span>
              </Detail>
              <Detail label={t("skills.source")}>
                <span className="break-words text-sm text-ink-secondary">
                  {skill?.installed_from ||
                    skill?.source ||
                    t("skills.unknown")}
                </span>
              </Detail>
            </div>
            <div className="mt-6">
              <Detail label={t("skills.tags")}>
                {skill?.tags?.length ? (
                  <div className="flex flex-wrap gap-1.5">
                    {skill.tags.map((tag) => (
                      <Badge key={tag} tone="neutral">
                        <Tags size={11} />
                        {tag}
                      </Badge>
                    ))}
                  </div>
                ) : (
                  <span className="text-sm text-ink-tertiary">
                    {t("skills.noTags")}
                  </span>
                )}
              </Detail>
            </div>
            {/* 技术信息：内部标识不再出现在列表主标题上，只在这里保留。 */}
            <div className="mt-6 border-t border-line pt-5">
              <Detail label={t("skills.internalName")}>
                <p className="break-all font-mono text-xs leading-5 text-ink-tertiary">
                  {skill?.name}
                </p>
              </Detail>
            </div>
          </div>
          <footer className="border-t border-line p-4">
            <Button
              variant="danger"
              size="sm"
              disabled={!skill || busy}
              onClick={() => skill && onDelete(skill)}
            >
              {busy ? (
                <LoaderCircle size={14} className="animate-spin" />
              ) : (
                <Trash2 size={14} />
              )}
              {t("skills.delete")}
            </Button>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function AddCapabilityDialog({
  open,
  mode,
  installedSkills,
  installedPlugins,
  onOpenChange,
  onNotice,
  onChanged,
}: {
  open: boolean;
  mode: MainTab;
  installedSkills: SkillInfo[];
  installedPlugins: PluginInfo[];
  onOpenChange: (open: boolean) => void;
  onNotice: (message: string) => void;
  onChanged: (message: string) => Promise<void>;
}) {
  const { language, t } = useTranslation();
  const [skillTab, setSkillTab] = useState<SkillSourceTab>("pool");
  const [pluginTab, setPluginTab] = useState<PluginSourceTab>("catalog");
  const [pool, setPool] = useState<PoolSkillInfo[]>([]);
  const [catalog, setCatalog] = useState<CatalogPlugin[]>([]);
  const [workspaces, setWorkspaces] = useState<WorkspaceSkillSummary[]>([]);
  const [workspaceId, setWorkspaceId] = useState("");
  const [pendingPoolSkill, setPendingPoolSkill] =
    useState<PoolSkillInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hubQuery, setHubQuery] = useState("");
  const [hubResults, setHubResults] = useState<HubSkillInfo[]>([]);
  const [hubTask, setHubTask] = useState<ActiveHubTask | null>(null);
  const [url, setUrl] = useState("");
  const skillFileRef = useRef<HTMLInputElement>(null);
  const pluginFileRef = useRef<HTMLInputElement>(null);
  const hubTaskRef = useRef<ActiveHubTask | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setBusy(null);
    setHubTask(null);
    hubTaskRef.current = null;
    setPendingPoolSkill(null);
    setWorkspaces([]);
    setWorkspaceId("");
    setLoading(true);
    const request =
      mode === "skills"
        ? Promise.all([skillApi.pool(), skillApi.workspaces()]).then(
            ([poolItems, availableWorkspaces]) => {
              setPool(poolItems);
              setWorkspaces(availableWorkspaces);
              setWorkspaceId(
                availableWorkspaces.length === 1
                  ? availableWorkspaces[0]?.agent_id ?? ""
                  : "",
              );
            },
          )
        : pluginApi.catalog().then((response) => {
            setWorkspaces([]);
            setWorkspaceId("");
            setCatalog(
              mergeCatalogInstalled(response.plugins, installedPlugins),
            );
            if (response.error) setError(response.error);
          });
    void request
      .catch((reason: unknown) => setError(readableError(reason)))
      .finally(() => setLoading(false));
  }, [open, mode]);

  const importPoolSkill = async (skill: PoolSkillInfo) => {
    if (!workspaceId) {
      setError(t("skills.add.noWorkspace"));
      return;
    }
    setBusy(skill.name);
    setError(null);
    try {
      await skillApi.importFromPool(skill.name, workspaceId);
      setPendingPoolSkill(null);
      await onChanged(t("skills.add.imported", { name: skill.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(null);
    }
  };

  const searchHub = async () => {
    setLoading(true);
    setError(null);
    try {
      setHubResults(await skillApi.searchHub(hubQuery.trim()));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setLoading(false);
    }
  };

  const installHubSkill = async (skill: HubSkillInfo) => {
    setBusy(skill.slug);
    setError(null);
    let taskId: string | null = null;
    try {
      const started = await skillApi.startHubInstall(skill);
      taskId = started.task_id;
      const activeTask = { ...started, skillName: skill.name };
      hubTaskRef.current = activeTask;
      setHubTask(activeTask);
      const result = await pollHubInstall(async (taskId) => {
        const status = await skillApi.hubInstallStatus(taskId);
        const currentTask = hubTaskRef.current;
        if (
          currentTask?.task_id === started.task_id &&
          currentTask.status !== "cancelled"
        ) {
          const nextTask = { ...status, skillName: skill.name };
          hubTaskRef.current = nextTask;
          setHubTask(nextTask);
        }
        return status;
      }, started.task_id);
      const isCurrentTask = hubTaskRef.current?.task_id === started.task_id;
      const wasCancelled =
        isCurrentTask && hubTaskRef.current?.status === "cancelled";
      if (result.status === "completed" && !wasCancelled && isCurrentTask) {
        await onChanged(t("skills.add.installed", { name: skill.name }));
      } else if (result.status === "failed" && isCurrentTask) {
        setError(
          t("skills.add.installFailed", {
            message: result.error || t("skills.unknown"),
          }),
        );
      } else if (result.status === "cancelled" && isCurrentTask) {
        onNotice(t("skills.installCancelled", { name: skill.name }));
      }
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      if (!taskId || hubTaskRef.current?.task_id === taskId) {
        setBusy(null);
      }
    }
  };

  const cancelHub = async () => {
    const task = hubTask;
    if (!task) return;
    try {
      await skillApi.cancelHubInstall(task.task_id);
      const currentTask = hubTaskRef.current;
      if (currentTask?.task_id === task.task_id) {
        const cancelledTask = { ...currentTask, status: "cancelled" as const };
        hubTaskRef.current = cancelledTask;
        setHubTask(cancelledTask);
        setBusy(null);
        onNotice(t("skills.installCancelled", { name: task.skillName }));
      }
    } catch (reason) {
      setError(readableError(reason));
    }
  };

  const requestPoolImport = (skill: PoolSkillInfo) => {
    if (workspaces.length > 1) {
      setWorkspaceId("");
      setPendingPoolSkill(skill);
      return;
    }
    void importPoolSkill(skill);
  };

  const uploadSkill = async (file: File) => {
    setBusy("skill-upload");
    setError(null);
    try {
      await skillApi.upload(file);
      await onChanged(t("skills.add.uploaded"));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(null);
      if (skillFileRef.current) skillFileRef.current.value = "";
    }
  };

  const installPlugin = async (source: string, label: string) => {
    setBusy(source);
    setError(null);
    try {
      await pluginApi.install(source);
      await onChanged(t("plugins.add.installed", { name: label }));
      setCatalog((items) =>
        items.map((item) =>
          item.install_url === source ? { ...item, installed: true } : item,
        ),
      );
      setUrl("");
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(null);
    }
  };

  const uploadPlugin = async (file: File) => {
    setBusy("plugin-upload");
    setError(null);
    try {
      const plugin = await pluginApi.upload(file);
      await onChanged(t("plugins.add.installed", { name: plugin.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(null);
      if (pluginFileRef.current) pluginFileRef.current.value = "";
    }
  };

  const tabs =
    mode === "skills"
      ? ([
          ["pool", t("skills.add.pool")],
          ["hub", t("skills.add.hub")],
          ["upload", t("skills.add.upload")],
        ] as Array<[SkillSourceTab, string]>)
      : ([
          ["catalog", t("plugins.add.catalog")],
          ["url", t("plugins.add.url")],
          ["upload", t("plugins.add.upload")],
        ] as Array<[PluginSourceTab, string]>);
  const activeTab = mode === "skills" ? skillTab : pluginTab;

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 flex max-h-[min(44rem,calc(100%-2rem))] w-[calc(100%-2rem)] max-w-2xl -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[var(--radius-lg)] border border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {mode === "skills"
                  ? t("skills.add.title")
                  : t("plugins.add.title")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-tertiary">
                {mode === "skills"
                  ? t("skills.add.description")
                  : t("plugins.add.description")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton size="sm" title={t("skills.close")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <div className="border-b border-line px-5 py-3">
            <SegmentedControl
              value={activeTab}
              options={tabs.map(([value, label]) => ({ value, label }))}
              onChange={(value) =>
                mode === "skills"
                  ? setSkillTab(value as SkillSourceTab)
                  : setPluginTab(value as PluginSourceTab)
              }
            />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            {mode === "skills" &&
              skillTab === "pool" &&
              pendingPoolSkill &&
              workspaces.length > 1 && (
                <section className="mb-4 rounded-md border border-line bg-surface p-4">
                  <h3 className="text-sm font-medium text-ink">
                    {t("skills.import.workspace")}
                  </h3>
                  <p className="mt-1 text-xs leading-5 text-ink-tertiary">
                    {t("skills.import.workspaceHint")}
                  </p>
                  <p className="mt-3 text-xs text-ink-secondary">
                    {skillDisplayName(pendingPoolSkill.name, language)}
                  </p>
                  <div
                    role="radiogroup"
                    aria-label={t("skills.import.workspace")}
                    className="mt-2 space-y-1.5"
                  >
                    {workspaces.map((workspace) => {
                      const label = workspaceLabel(
                        workspace,
                        t("skills.import.workspace"),
                      );
                      return (
                        <label
                          key={workspace.agent_id}
                          className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-2 text-sm text-ink-secondary hover:bg-fill-hover"
                        >
                          <input
                            type="radio"
                            name="skill-import-workspace"
                            value={workspace.agent_id}
                            checked={workspaceId === workspace.agent_id}
                            onChange={() => setWorkspaceId(workspace.agent_id)}
                            className="accent-accent"
                          />
                          <span className="min-w-0 truncate">{label}</span>
                        </label>
                      );
                    })}
                  </div>
                  <div className="mt-4 flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={busy !== null}
                      onClick={() => {
                        setPendingPoolSkill(null);
                        setWorkspaceId("");
                      }}
                    >
                      {t("common.cancel")}
                    </Button>
                    <Button
                      variant="primary"
                      size="sm"
                      disabled={!workspaceId || busy !== null}
                      onClick={() =>
                        pendingPoolSkill &&
                        void importPoolSkill(pendingPoolSkill)
                      }
                    >
                      {t("skills.add.import")}
                    </Button>
                  </div>
                </section>
              )}
            {error && (
              <div className="mb-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
                {error}
              </div>
            )}
            {loading ? (
              <SkeletonRows rows={5} />
            ) : mode === "skills" && skillTab === "pool" ? (
              <CapabilitySourceList
                icon={<Blocks size={16} />}
                items={pool.map((skill) => ({
                  key: skill.name,
                  name: skillDisplayName(skill.name, language),
                  title: skill.name,
                  emoji: skill.emoji,
                  description: skillDescription(skill.name, language),
                  version: skill.version_text,
                  installed: installedSkills.some(
                    (installed) => installed.name === skill.name,
                  ),
                }))}
                busy={busy}
                onInstall={(key) => {
                  const skill = pool.find((item) => item.name === key);
                  if (skill) requestPoolImport(skill);
                }}
              />
            ) : mode === "skills" && skillTab === "hub" ? (
              <div>
                <form
                  className="flex gap-2"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void searchHub();
                  }}
                >
                  <Input
                    value={hubQuery}
                    onChange={(event) => setHubQuery(event.target.value)}
                    placeholder={t("skills.add.hubPlaceholder")}
                  />
                  <Button type="submit" variant="secondary" size="sm">
                    {t("skills.search")}
                  </Button>
                </form>
                {hubTask &&
                  (hubTask.status === "pending" ||
                    hubTask.status === "importing") && (
                    <div className="mt-3 flex items-center gap-2 rounded-md bg-accent-soft px-3 py-2 text-xs text-accent">
                      <LoaderCircle size={14} className="animate-spin" />
                      <span className="min-w-0 flex-1">
                        {t("skills.add.installing", {
                          name: hubTask.skillName,
                        })}
                      </span>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => void cancelHub()}
                      >
                        {t("skills.add.cancel")}
                      </Button>
                    </div>
                  )}
                {hubResults.length === 0 ? (
                  <p className="py-12 text-center text-sm text-ink-tertiary">
                    {t("skills.add.hubHint")}
                  </p>
                ) : (
                  <div className="mt-4">
                    <CapabilitySourceList
                      icon={<Blocks size={16} />}
                      items={hubResults.map((skill) => ({
                        key: skill.slug,
                        name: skill.name,
                        description: skillDescription(skill.name, language),
                        version: skill.version,
                        installed: installedSkills.some(
                          (installed) =>
                            installed.name === skill.slug ||
                            installed.name === skill.name,
                        ),
                      }))}
                      busy={busy}
                      onInstall={(key) => {
                        const skill = hubResults.find(
                          (item) => item.slug === key,
                        );
                        if (skill) void installHubSkill(skill);
                      }}
                    />
                  </div>
                )}
              </div>
            ) : mode === "skills" ? (
              <ZipUpload
                inputRef={skillFileRef}
                busy={busy === "skill-upload"}
                onFile={(file) => void uploadSkill(file)}
              />
            ) : pluginTab === "catalog" ? (
              <CapabilitySourceList
                icon={<Puzzle size={16} />}
                items={catalog.map((plugin) => ({
                  key: plugin.plugin_id,
                  name: plugin.name,
                  description:
                    plugin.description_i18n?.[
                      language === "zh" ? "zh-CN" : "en-US"
                    ] || plugin.description,
                  version: plugin.version,
                  installed: plugin.installed,
                }))}
                busy={busy}
                onInstall={(key) => {
                  const plugin = catalog.find((item) => item.plugin_id === key);
                  if (plugin) {
                    void installPlugin(plugin.install_url, plugin.name);
                  }
                }}
              />
            ) : pluginTab === "url" ? (
              <form
                onSubmit={(event) => {
                  event.preventDefault();
                  if (url.trim()) void installPlugin(url.trim(), url.trim());
                }}
              >
                <label className="text-xs font-medium text-ink-secondary">
                  {t("plugins.add.urlLabel")}
                  <Input
                    type="url"
                    value={url}
                    onChange={(event) => setUrl(event.target.value)}
                    placeholder={t("plugins.add.urlPlaceholder")}
                    className="mt-1.5"
                  />
                </label>
                <div className="mt-4 flex justify-end">
                  <Button
                    type="submit"
                    variant="primary"
                    size="sm"
                    disabled={!url.trim() || busy !== null}
                  >
                    {busy === url.trim() && (
                      <LoaderCircle size={14} className="animate-spin" />
                    )}
                    {t("plugins.add.install")}
                  </Button>
                </div>
              </form>
            ) : (
              <ZipUpload
                inputRef={pluginFileRef}
                busy={busy === "plugin-upload"}
                onFile={(file) => void uploadPlugin(file)}
              />
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function CapabilitySourceList({
  items,
  icon,
  busy,
  onInstall,
}: {
  items: Array<{
    key: string;
    name: string;
    title?: string;
    emoji?: string;
    description?: string;
    version?: string;
    installed: boolean;
  }>;
  icon: React.ReactNode;
  busy: string | null;
  onInstall: (key: string) => void;
}) {
  const { t } = useTranslation();
  if (items.length === 0) {
    return (
      <p className="py-12 text-center text-sm text-ink-tertiary">
        {t("skills.add.none")}
      </p>
    );
  }
  return (
    <Card className="divide-y divide-line overflow-hidden rounded-[var(--radius-md)]">
      {items.map((item) => (
        <div key={item.key} className="flex items-center gap-3 px-4 py-3">
          <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[10px] border border-line bg-bubble-tool text-icon">
            {item.emoji || icon}
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span
                title={item.title ?? item.name}
                className="truncate text-sm font-medium text-ink"
              >
                {item.name}
              </span>
              {item.version && (
                <Badge tone="neutral" className="shrink-0">
                  {item.version}
                </Badge>
              )}
            </div>
            <p className="line-clamp-2 text-[13px] leading-5 text-ink-tertiary">
              {item.description || t("skills.noDescription")}
            </p>
          </div>
          {item.installed ? (
            <Badge tone="ok">{t("skills.add.installedMark")}</Badge>
          ) : (
            <Button
              variant="secondary"
              size="sm"
              disabled={busy !== null}
              onClick={() => onInstall(item.key)}
            >
              {busy === item.key ? (
                <LoaderCircle size={13} className="animate-spin" />
              ) : (
                <Download size={13} />
              )}
              {t("skills.add.import")}
            </Button>
          )}
        </div>
      ))}
    </Card>
  );
}

function ZipUpload({
  inputRef,
  busy,
  onFile,
}: {
  inputRef: React.RefObject<HTMLInputElement>;
  busy: boolean;
  onFile: (file: File) => void;
}) {
  const { t } = useTranslation();
  return (
    <label className="flex cursor-pointer flex-col items-center rounded-lg border border-dashed border-line px-6 py-12 text-center transition-colors hover:border-line-strong focus-within:border-line-strong focus-within:shadow-[0_0_0_3px_var(--ring)]">
      {busy ? (
        <LoaderCircle size={24} className="animate-spin text-accent" />
      ) : (
        <Upload size={24} className="text-accent" />
      )}
      <span className="mt-3 text-sm font-medium text-ink">
        {busy ? t("skills.add.uploading") : t("skills.add.chooseZip")}
      </span>
      <span className="mt-1 text-xs text-ink-tertiary">
        {t("skills.add.zipHint")}
      </span>
      <input
        ref={inputRef}
        type="file"
        accept=".zip,application/zip"
        disabled={busy}
        className="sr-only"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) onFile(file);
        }}
      />
    </label>
  );
}

function SearchField({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <label className="relative block w-full max-w-xs">
      <Search
        size={15}
        className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-icon"
      />
      <Input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("skills.searchPlaceholder")}
        className="rounded-full pl-9"
      />
    </label>
  );
}

function Detail({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h3 className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-ink-tertiary">
        {label}
      </h3>
      {children}
    </section>
  );
}

/**
 * snake_case 内部标识 → 可读名称（与 ToolCard 的 humanToolName 同一套规则；
 * 该函数未导出，此处内联等价实现，避免为复用去改动工具卡片）。
 */

function workspaceLabel(workspace: WorkspaceSkillSummary, fallback: string) {
  const agentName = workspace.agent_name?.trim();
  if (agentName) return agentName;
  const segments = workspace.workspace_dir.split(/[\\/]/).filter(Boolean);
  return segments.at(-1) || fallback;
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
