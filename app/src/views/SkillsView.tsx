import * as Dialog from "@radix-ui/react-dialog";
import {
  Box,
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
} from "../lib/capabilities";
import { useTranslation } from "../lib/i18n";

type MainTab = "skills" | "plugins";
type SkillSourceTab = "pool" | "hub" | "upload";
type PluginSourceTab = "catalog" | "url" | "upload";

export function SkillsView() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<MainTab>("skills");
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [busySkill, setBusySkill] = useState<string | null>(null);
  const [busyPlugin, setBusyPlugin] = useState<string | null>(null);
  const [selectedSkill, setSelectedSkill] = useState<SkillInfo | null>(null);
  const [addOpen, setAddOpen] = useState(false);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [skillItems, pluginItems] = await Promise.all([
        skillApi.list(),
        pluginApi.list(),
      ]);
      setSkills(skillItems);
      setPlugins(pluginItems);
    } catch (reason) {
      setError(t("skills.loadFailed", { message: readableError(reason) }));
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
      `${skill.name} ${skill.description} ${(skill.tags ?? []).join(" ")}`
        .toLocaleLowerCase()
        .includes(needle),
    );
  }, [query, skills]);

  const toggleSkill = async (skill: SkillInfo) => {
    const enabled = !skill.enabled;
    setBusySkill(skill.name);
    setError(null);
    setNotice(null);
    try {
      await runOptimisticSkillToggle({
        skills,
        name: skill.name,
        enabled,
        onUpdate: setSkills,
        mutate: () => skillApi.setEnabled(skill.name, enabled),
      });
      setSelectedSkill((current) =>
        current?.name === skill.name ? { ...current, enabled } : current,
      );
    } catch (reason) {
      setError(
        t("skills.toggleFailed", {
          name: skill.name,
          message: readableError(reason),
        }),
      );
    } finally {
      setBusySkill(null);
    }
  };

  const deleteSkill = async (skill: SkillInfo) => {
    if (!window.confirm(t("skills.deleteConfirm", { name: skill.name }))) {
      return;
    }
    setBusySkill(skill.name);
    setError(null);
    try {
      await skillApi.delete(skill.name);
      setSkills((items) => items.filter((item) => item.name !== skill.name));
      setSelectedSkill(null);
      setNotice(t("skills.deleted", { name: skill.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusySkill(null);
    }
  };

  const deletePlugin = async (plugin: PluginInfo) => {
    if (!window.confirm(t("plugins.deleteConfirm", { name: plugin.name }))) {
      return;
    }
    setBusyPlugin(plugin.id);
    setError(null);
    try {
      await pluginApi.delete(plugin.id);
      setPlugins((items) => items.filter((item) => item.id !== plugin.id));
      setNotice(t("plugins.deleted", { name: plugin.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyPlugin(null);
    }
  };

  return (
    <div className="h-full overflow-y-auto bg-surface">
      <div className="mx-auto max-w-5xl px-6 py-8 sm:px-10">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-medium tracking-tight text-ink">
              {t("skills.title")}
            </h1>
            <p className="mt-1 text-sm text-ink-muted">
              {t("skills.subtitle")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setAddOpen(true)}
            className="flex items-center gap-1.5 rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink"
          >
            <Plus size={15} />
            {t("skills.add")}
          </button>
        </header>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-1">
            {(["skills", "plugins"] as const).map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => {
                  setTab(value);
                  setQuery("");
                  setError(null);
                  setNotice(null);
                }}
                className={`rounded-full px-3 py-1.5 text-sm font-medium transition-colors ${
                  tab === value
                    ? "bg-line/60 text-ink"
                    : "text-ink-secondary hover:bg-line/30 hover:text-ink"
                }`}
              >
                {value === "skills"
                  ? `${t("skills.tab.skills")} ${skills.length}`
                  : `${t("skills.tab.plugins")} ${plugins.length}`}
              </button>
            ))}
          </div>
          {tab === "skills" && (
            <SearchField value={query} onChange={setQuery} />
          )}
        </div>

        {error && (
          <Banner tone="danger" onDismiss={() => setError(null)}>
            {error}
          </Banner>
        )}
        {notice && (
          <div
            role="status"
            className="mt-4 rounded-md bg-accent-soft px-3 py-2 text-xs text-accent"
          >
            {notice}
          </div>
        )}

        {loading ? (
          <LoadingBlock label={t("skills.loading")} />
        ) : tab === "skills" ? (
          <section className="mt-5">
            {filteredSkills.length === 0 ? (
              <EmptyState
                icon={<PackageOpen size={28} />}
                title={t(query ? "skills.noResults" : "skills.empty")}
                description={t(
                  query
                    ? "skills.noResultsDescription"
                    : "skills.emptyDescription",
                )}
              />
            ) : (
              <div className="divide-y divide-line overflow-hidden rounded-lg border border-line bg-surface">
                {filteredSkills.map((skill) => (
                  <SkillRow
                    key={skill.name}
                    skill={skill}
                    busy={busySkill === skill.name}
                    onOpen={() => setSelectedSkill(skill)}
                    onToggle={() => void toggleSkill(skill)}
                  />
                ))}
              </div>
            )}
          </section>
        ) : plugins.length === 0 ? (
          <EmptyState
            icon={<Puzzle size={28} />}
            title={t("plugins.empty")}
            description={t("plugins.emptyDescription")}
          />
        ) : (
          <div className="mt-5 divide-y divide-line overflow-hidden rounded-lg border border-line bg-surface">
            {plugins.map((plugin) => (
              <PluginRow
                key={plugin.id}
                plugin={plugin}
                busy={busyPlugin === plugin.id}
                onDelete={() => void deletePlugin(plugin)}
              />
            ))}
          </div>
        )}
      </div>

      <SkillDetails
        skill={selectedSkill}
        busy={selectedSkill?.name === busySkill}
        onOpenChange={(open) => !open && setSelectedSkill(null)}
        onDelete={(skill) => void deleteSkill(skill)}
      />
      <AddCapabilityDialog
        open={addOpen}
        mode={tab}
        installedSkills={skills}
        installedPlugins={plugins}
        onOpenChange={setAddOpen}
        onChanged={async (message) => {
          setNotice(message);
          await load();
        }}
      />
    </div>
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
  const { t } = useTranslation();
  const source = skill.installed_from || skill.source;
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
      className="group flex cursor-pointer items-center gap-3 px-4 py-3 outline-none transition-colors hover:bg-line/30 focus-visible:bg-accent-soft"
    >
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-bubble-tool text-lg">
        {skill.emoji || "✦"}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="truncate text-sm font-medium text-ink">
            {skill.name}
          </span>
          {skill.version_text && (
            <span className="shrink-0 text-[11px] text-ink-muted">
              {skill.version_text}
            </span>
          )}
          {source && (
            <span className="truncate text-xs text-ink-muted">{source}</span>
          )}
        </div>
        <p className="line-clamp-1 text-xs text-ink-muted">
          {skill.description || t("skills.noDescription")}
        </p>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={skill.enabled}
        aria-label={t("skills.toggleLabel", { name: skill.name })}
        disabled={busy}
        onClick={(event) => {
          event.stopPropagation();
          onToggle();
        }}
        className="flex shrink-0 items-center gap-2 disabled:opacity-40"
      >
        {busy && <LoaderCircle size={14} className="animate-spin text-accent" />}
        <SwitchTrack checked={skill.enabled} />
      </button>
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
    <div className="flex items-center gap-3 px-4 py-3 transition-colors hover:bg-line/30">
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-bubble-tool text-accent">
        <Puzzle size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
          <span className="truncate text-sm font-medium text-ink">
            {plugin.name}
          </span>
          {plugin.version && (
            <span className="shrink-0 text-[11px] text-ink-muted">
              {plugin.version}
            </span>
          )}
          {source && (
            <span className="truncate text-xs text-ink-muted">{source}</span>
          )}
          {toolCount !== null && (
            <span className="text-[11px] text-ink-muted">
              · {t("plugins.tools", { count: toolCount })}
            </span>
          )}
        </div>
        <p className="line-clamp-1 text-xs text-ink-muted">
          {plugin.description || t("skills.noDescription")}
        </p>
      </div>
      <button
        type="button"
        disabled={busy}
        onClick={onDelete}
        className="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs text-danger transition-colors hover:bg-danger-soft disabled:opacity-40"
      >
        {busy ? (
          <LoaderCircle size={14} className="animate-spin" />
        ) : (
          <Trash2 size={14} />
        )}
        {t("plugins.uninstall")}
      </button>
    </div>
  );
}

function SkillDetails({
  skill,
  busy,
  onOpenChange,
  onDelete,
}: {
  skill: SkillInfo | null;
  busy: boolean;
  onOpenChange: (open: boolean) => void;
  onDelete: (skill: SkillInfo) => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog.Root open={skill !== null} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed inset-y-0 right-0 z-50 flex w-[min(29rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <span className="text-xl">{skill?.emoji || "✦"}</span>
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {skill?.name}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-muted">
                {t("skills.detailsDescription")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton label={t("skills.close")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            <Detail label={t("skills.description")}>
              <p className="whitespace-pre-wrap text-sm leading-6 text-ink-secondary">
                {skill?.description || t("skills.noDescription")}
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
                      <span
                        key={tag}
                        className="flex items-center gap-1 rounded-md bg-accent-soft px-2 py-1 text-xs text-accent"
                      >
                        <Tags size={11} />
                        {tag}
                      </span>
                    ))}
                  </div>
                ) : (
                  <span className="text-sm text-ink-muted">
                    {t("skills.noTags")}
                  </span>
                )}
              </Detail>
            </div>
          </div>
          <footer className="border-t border-line p-4">
            <button
              type="button"
              disabled={!skill || busy}
              onClick={() => skill && onDelete(skill)}
              className="flex items-center gap-1.5 rounded-md px-3 py-2 text-xs font-medium text-danger transition-colors hover:bg-danger-soft disabled:opacity-40"
            >
              {busy ? (
                <LoaderCircle size={14} className="animate-spin" />
              ) : (
                <Trash2 size={14} />
              )}
              {t("skills.delete")}
            </button>
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
  onChanged,
}: {
  open: boolean;
  mode: MainTab;
  installedSkills: SkillInfo[];
  installedPlugins: PluginInfo[];
  onOpenChange: (open: boolean) => void;
  onChanged: (message: string) => Promise<void>;
}) {
  const { language, t } = useTranslation();
  const [skillTab, setSkillTab] = useState<SkillSourceTab>("pool");
  const [pluginTab, setPluginTab] = useState<PluginSourceTab>("catalog");
  const [pool, setPool] = useState<PoolSkillInfo[]>([]);
  const [catalog, setCatalog] = useState<CatalogPlugin[]>([]);
  const [workspaceId, setWorkspaceId] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hubQuery, setHubQuery] = useState("");
  const [hubResults, setHubResults] = useState<HubSkillInfo[]>([]);
  const [hubTask, setHubTask] = useState<
    (HubInstallTask & { skillName: string }) | null
  >(null);
  const [url, setUrl] = useState("");
  const skillFileRef = useRef<HTMLInputElement>(null);
  const pluginFileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setBusy(null);
    setHubTask(null);
    setLoading(true);
    const request =
      mode === "skills"
        ? Promise.all([skillApi.pool(), skillApi.workspaces()]).then(
            ([poolItems, workspaces]) => {
              setPool(poolItems);
              setWorkspaceId(workspaces[0]?.agent_id ?? "");
            },
          )
        : pluginApi.catalog().then((response) => {
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
    try {
      const started = await skillApi.startHubInstall(skill);
      setHubTask({ ...started, skillName: skill.name });
      const result = await pollHubInstall(async (taskId) => {
        const status = await skillApi.hubInstallStatus(taskId);
        setHubTask({ ...status, skillName: skill.name });
        return status;
      }, started.task_id);
      if (result.status === "completed") {
        await onChanged(t("skills.add.installed", { name: skill.name }));
      } else if (result.status === "failed") {
        setError(
          t("skills.add.installFailed", {
            message: result.error || t("skills.unknown"),
          }),
        );
      }
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(null);
    }
  };

  const cancelHub = async () => {
    if (!hubTask) return;
    try {
      await skillApi.cancelHubInstall(hubTask.task_id);
      setHubTask({ ...hubTask, status: "cancelled" });
      setBusy(null);
    } catch (reason) {
      setError(readableError(reason));
    }
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
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 flex max-h-[min(44rem,calc(100%-2rem))] w-[calc(100%-2rem)] max-w-2xl -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-lg border border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {mode === "skills"
                  ? t("skills.add.title")
                  : t("plugins.add.title")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-muted">
                {mode === "skills"
                  ? t("skills.add.description")
                  : t("plugins.add.description")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton label={t("skills.close")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <div className="border-b border-line px-5 pt-3">
            <div className="flex gap-5">
              {tabs.map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  onClick={() =>
                    mode === "skills"
                      ? setSkillTab(value as SkillSourceTab)
                      : setPluginTab(value as PluginSourceTab)
                  }
                  className={`border-b-2 px-0.5 pb-2 text-xs font-medium transition-colors ${
                    activeTab === value
                      ? "border-accent text-accent"
                      : "border-transparent text-ink-muted hover:text-ink"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            {error && (
              <div className="mb-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
                {error}
              </div>
            )}
            {loading ? (
              <LoadingBlock label={t("skills.loading")} compact />
            ) : mode === "skills" && skillTab === "pool" ? (
              <CapabilitySourceList
                items={pool.map((skill) => ({
                  key: skill.name,
                  name: skill.name,
                  description: skill.description,
                  version: skill.version_text,
                  installed: installedSkills.some(
                    (installed) => installed.name === skill.name,
                  ),
                  icon: skill.emoji,
                }))}
                busy={busy}
                onInstall={(key) => {
                  const skill = pool.find((item) => item.name === key);
                  if (skill) void importPoolSkill(skill);
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
                  <input
                    value={hubQuery}
                    onChange={(event) => setHubQuery(event.target.value)}
                    placeholder={t("skills.add.hubPlaceholder")}
                    className={inputClassName}
                  />
                  <button
                    type="submit"
                    className="rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary hover:border-line-strong hover:text-ink"
                  >
                    {t("skills.search")}
                  </button>
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
                      <button
                        type="button"
                        onClick={() => void cancelHub()}
                        className="font-medium hover:underline"
                      >
                        {t("skills.add.cancel")}
                      </button>
                    </div>
                  )}
                {hubResults.length === 0 ? (
                  <p className="py-12 text-center text-sm text-ink-muted">
                    {t("skills.add.hubHint")}
                  </p>
                ) : (
                  <div className="mt-4">
                    <CapabilitySourceList
                      items={hubResults.map((skill) => ({
                        key: skill.slug,
                        name: skill.name,
                        description: skill.description,
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
                  const plugin = catalog.find(
                    (item) => item.plugin_id === key,
                  );
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
                  <input
                    type="url"
                    value={url}
                    onChange={(event) => setUrl(event.target.value)}
                    placeholder={t("plugins.add.urlPlaceholder")}
                    className={`${inputClassName} mt-1.5`}
                  />
                </label>
                <div className="mt-4 flex justify-end">
                  <button
                    type="submit"
                    disabled={!url.trim() || busy !== null}
                    className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface hover:bg-accent-hover disabled:opacity-40"
                  >
                    {busy === url.trim() && (
                      <LoaderCircle size={14} className="animate-spin" />
                    )}
                    {t("plugins.add.install")}
                  </button>
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
  busy,
  onInstall,
}: {
  items: Array<{
    key: string;
    name: string;
    description?: string;
    version?: string;
    installed: boolean;
    icon?: string;
  }>;
  busy: string | null;
  onInstall: (key: string) => void;
}) {
  const { t } = useTranslation();
  if (items.length === 0) {
    return (
      <p className="py-12 text-center text-sm text-ink-muted">
        {t("skills.add.none")}
      </p>
    );
  }
  return (
    <div className="divide-y divide-line rounded-md border border-line">
      {items.map((item) => (
        <div key={item.key} className="flex items-center gap-3 px-3 py-3">
          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-bubble-tool">
            {item.icon || <Box size={16} className="text-accent" />}
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="truncate text-sm font-medium text-ink">
                {item.name}
              </span>
              {item.version && (
                <span className="shrink-0 text-[11px] text-ink-muted">
                  {item.version}
                </span>
              )}
            </div>
            <p className="line-clamp-1 text-xs text-ink-muted">
              {item.description || t("skills.noDescription")}
            </p>
          </div>
          {item.installed ? (
            <span className="shrink-0 rounded-md bg-accent-soft px-2 py-1 text-[11px] font-medium text-accent">
              {t("skills.add.installedMark")}
            </span>
          ) : (
            <button
              type="button"
              disabled={busy !== null}
              onClick={() => onInstall(item.key)}
              className="flex shrink-0 items-center gap-1.5 rounded-md border border-line px-2.5 py-1.5 text-xs font-medium text-ink-secondary hover:border-line-strong hover:text-ink disabled:opacity-40"
            >
              {busy === item.key ? (
                <LoaderCircle size={13} className="animate-spin" />
              ) : (
                <Download size={13} />
              )}
              {t("skills.add.import")}
            </button>
          )}
        </div>
      ))}
    </div>
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
    <label className="flex cursor-pointer flex-col items-center rounded-lg border border-dashed border-line px-6 py-12 text-center transition-colors hover:border-line-strong">
      {busy ? (
        <LoaderCircle size={24} className="animate-spin text-accent" />
      ) : (
        <Upload size={24} className="text-accent" />
      )}
      <span className="mt-3 text-sm font-medium text-ink">
        {busy ? t("skills.add.uploading") : t("skills.add.chooseZip")}
      </span>
      <span className="mt-1 text-xs text-ink-muted">
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
        className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted"
      />
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("skills.searchPlaceholder")}
        className={`${inputClassName} pl-9`}
      />
    </label>
  );
}

function SwitchTrack({ checked }: { checked: boolean }) {
  return (
    <span
      className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${
        checked ? "bg-accent" : "bg-line-strong"
      }`}
    >
      <span
        className={`absolute left-0 top-0.5 h-4 w-4 rounded-full bg-surface shadow-sm transition-transform ${
          checked ? "translate-x-[1.125rem]" : "translate-x-0.5"
        }`}
      />
    </span>
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
      <h3 className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-ink-muted">
        {label}
      </h3>
      {children}
    </section>
  );
}

function IconButton({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      className="rounded-md p-1 text-ink-muted hover:bg-line/50 hover:text-ink"
    >
      {children}
    </button>
  );
}

function LoadingBlock({
  label,
  compact = false,
}: {
  label: string;
  compact?: boolean;
}) {
  return (
    <div
      className={`flex items-center justify-center gap-2 text-sm text-ink-muted ${
        compact ? "py-12" : "mt-5 rounded-lg border border-line py-16"
      }`}
    >
      <LoaderCircle size={16} className="animate-spin" />
      {label}
    </div>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="mt-5 flex flex-col items-center rounded-lg border border-dashed border-line px-6 py-16 text-center text-ink-muted">
      {icon}
      <h2 className="mt-4 font-medium text-ink">{title}</h2>
      <p className="mt-1 max-w-sm text-sm">{description}</p>
    </div>
  );
}

const inputClassName =
  "block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none transition-colors placeholder:text-ink-muted focus:border-line-strong disabled:cursor-not-allowed disabled:bg-bubble-tool disabled:text-ink-muted";

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
