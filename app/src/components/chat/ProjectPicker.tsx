import * as Dialog from "@radix-ui/react-dialog";
import {
  ArrowLeft,
  Check,
  ChevronRight,
  Folder,
  FolderGit2,
  FolderOpen,
  GitBranch,
  LoaderCircle,
  Plus,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { projectApi } from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import {
  loadRecentProjects,
  mergeProjects,
  projectNameFromPath,
  rememberRecentProject,
  type CodingProject,
  type CodingProjectInfo,
  type DirectoryListing,
  type ProjectBinding,
} from "../../lib/projects";
import { useChatStore } from "../../stores/chat";

type PickerMode = "list" | "browse" | "create";

export function ProjectPicker() {
  const { t } = useTranslation();
  const project = useChatStore((state) => state.project);
  const setProject = useChatStore((state) => state.setProject);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<PickerMode>("list");
  const [managed, setManaged] = useState<CodingProject[]>([]);
  const [current, setCurrent] = useState<CodingProjectInfo | null>(null);
  const [listing, setListing] = useState<DirectoryListing | null>(null);
  const [projectName, setProjectName] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const projects = useMemo(
    () => mergeProjects(managed, loadRecentProjects()),
    [managed, open],
  );

  useEffect(() => {
    if (!open) return;
    setMode("list");
    setError(null);
    setLoading(true);
    void Promise.all([projectApi.current(), projectApi.list()])
      .then(([projectInfo, projectList]) => {
        setCurrent(projectInfo);
        setManaged(projectList);
      })
      .catch((reason: unknown) => setError(readableError(reason)))
      .finally(() => setLoading(false));
  }, [open]);

  const selectProject = (selection: ProjectBinding | null) => {
    setProject(selection);
    setOpen(false);
  };

  const browse = async (path = "~") => {
    setMode("browse");
    setLoading(true);
    setError(null);
    try {
      setListing(await projectApi.browse(path));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setLoading(false);
    }
  };

  const selectBrowsedDirectory = () => {
    if (!listing || listing.selectable === false) return;
    const selection = {
      path: listing.current,
      name: projectNameFromPath(listing.current),
    };
    rememberRecentProject(selection);
    selectProject(selection);
  };

  const createProject = async () => {
    const name = projectName.trim();
    if (!name) return;
    setSaving(true);
    setError(null);
    try {
      const created = await projectApi.create(name);
      const selection = { ...created, isGit: true };
      setManaged((items) => [
        ...items.filter((item) => item.path !== created.path),
        {
          path: created.path,
          name: created.name,
          is_git: true,
          is_active: true,
        },
      ]);
      setProjectName("");
      selectProject(selection);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          title={project?.path ?? t("projects.defaultWorkspace")}
          className="flex max-w-40 items-center gap-1.5 rounded-md px-2 py-1.5 text-xs text-ink-secondary transition-colors hover:bg-line/50"
        >
          <Folder size={14} className="shrink-0" />
          <span className="truncate">
            {project?.name ?? t("projects.defaultWorkspace")}
          </span>
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 flex max-h-[min(42rem,calc(100%-2rem))] w-[calc(100%-2rem)] max-w-2xl -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-lg border border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {mode === "browse"
                  ? t("projects.browseTitle")
                  : mode === "create"
                    ? t("projects.createTitle")
                    : t("projects.title")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-muted">
                {mode === "browse"
                  ? t("projects.browseDescription")
                  : mode === "create"
                    ? t("projects.createDescription")
                    : t("projects.description")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                title={t("projects.close")}
                className="rounded-md p-1 text-ink-muted hover:bg-line/50 hover:text-ink"
              >
                <X size={16} />
              </button>
            </Dialog.Close>
          </header>

          {error && (
            <div className="mx-5 mt-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
              {error}
            </div>
          )}

          {mode === "list" && (
            <>
              <div className="min-h-0 flex-1 overflow-y-auto p-3">
                {loading ? (
                  <Loading label={t("projects.loading")} />
                ) : (
                  <div className="space-y-1">
                    <ProjectRow
                      name={t("projects.defaultWorkspace")}
                      path={current?.workspace_dir ?? ""}
                      selected={!project}
                      onClick={() => selectProject(null)}
                    />
                    {projects.map((item) => (
                      <ProjectRow
                        key={item.path}
                        name={item.name}
                        path={item.path}
                        isGit={item.isGit}
                        selected={project?.path === item.path}
                        onClick={() => selectProject(item)}
                      />
                    ))}
                  </div>
                )}
              </div>
              <footer className="flex flex-wrap items-center gap-2 border-t border-line px-4 py-3">
                <button
                  type="button"
                  onClick={() => void browse()}
                  className="flex items-center gap-1.5 rounded-md border border-line px-3 py-2 text-xs font-medium text-ink-secondary hover:border-line-strong hover:text-ink"
                >
                  <FolderOpen size={14} />
                  {t("projects.browse")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setMode("create");
                    setError(null);
                  }}
                  className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface hover:bg-accent-hover"
                >
                  <Plus size={14} />
                  {t("projects.create")}
                </button>
              </footer>
            </>
          )}

          {mode === "browse" && (
            <>
              <div className="flex items-center gap-2 border-b border-line px-4 py-2">
                <button
                  type="button"
                  disabled={!listing?.parent || loading}
                  onClick={() => {
                    if (listing?.parent) void browse(listing.parent);
                  }}
                  title={t("projects.parent")}
                  className="rounded-md p-1.5 text-ink-secondary hover:bg-line/50 disabled:opacity-30"
                >
                  <ArrowLeft size={15} />
                </button>
                <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink-secondary">
                  {listing?.current ?? t("projects.loading")}
                </span>
              </div>
              <div className="min-h-0 flex-1 overflow-y-auto p-3">
                {loading ? (
                  <Loading label={t("projects.loadingDirectories")} />
                ) : listing?.dirs.length ? (
                  <div className="space-y-0.5">
                    {listing.dirs.map((directory) => (
                      <button
                        key={directory.path}
                        type="button"
                        onClick={() => void browse(directory.path)}
                        className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-ink-secondary hover:bg-line/50 hover:text-ink"
                      >
                        <Folder size={15} className="shrink-0 text-ink-muted" />
                        <span className="min-w-0 flex-1 truncate">
                          {directory.name}
                        </span>
                        <ChevronRight size={14} className="text-ink-muted" />
                      </button>
                    ))}
                  </div>
                ) : (
                  <div className="px-3 py-8 text-center text-sm text-ink-muted">
                    {t("projects.noSubdirectories")}
                  </div>
                )}
              </div>
              <footer className="flex items-center justify-between gap-3 border-t border-line px-4 py-3">
                <button
                  type="button"
                  onClick={() => setMode("list")}
                  className="rounded-md px-3 py-2 text-xs font-medium text-ink-secondary hover:bg-line/50"
                >
                  {t("projects.back")}
                </button>
                <button
                  type="button"
                  disabled={!listing || listing.selectable === false || loading}
                  onClick={selectBrowsedDirectory}
                  className="rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {t("projects.selectCurrent")}
                </button>
              </footer>
            </>
          )}

          {mode === "create" && (
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void createProject();
              }}
              className="p-5"
            >
              <label className="block text-xs font-medium text-ink-secondary">
                {t("projects.name")}
                <input
                  autoFocus
                  value={projectName}
                  onChange={(event) => setProjectName(event.target.value)}
                  placeholder={t("projects.namePlaceholder")}
                  className="mt-1.5 block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none focus:border-line-strong"
                />
              </label>
              <div className="mt-5 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setMode("list")}
                  className="rounded-md px-3 py-2 text-xs font-medium text-ink-secondary hover:bg-line/50"
                >
                  {t("projects.back")}
                </button>
                <button
                  type="submit"
                  disabled={!projectName.trim() || saving}
                  className="rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {saving ? t("projects.creating") : t("projects.create")}
                </button>
              </div>
            </form>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ProjectRow({
  name,
  path,
  isGit,
  selected,
  onClick,
}: {
  name: string;
  path: string;
  isGit?: boolean;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left transition-colors ${
        selected ? "bg-accent-soft" : "hover:bg-line/50"
      }`}
    >
      {isGit ? (
        <FolderGit2 size={17} className="shrink-0 text-accent" />
      ) : (
        <Folder size={17} className="shrink-0 text-ink-muted" />
      )}
      <span className="min-w-0 flex-1">
        <span
          className={`flex items-center gap-1.5 text-sm font-medium ${
            selected ? "text-accent" : "text-ink"
          }`}
        >
          <span className="truncate">{name}</span>
          {isGit && <GitBranch size={12} className="shrink-0 text-ink-muted" />}
        </span>
        {path && (
          <span className="mt-0.5 block truncate font-mono text-[11px] text-ink-muted">
            {path}
          </span>
        )}
      </span>
      {selected && <Check size={15} className="shrink-0 text-accent" />}
    </button>
  );
}

function Loading({ label }: { label: string }) {
  return (
    <div className="flex items-center justify-center gap-2 px-3 py-10 text-sm text-ink-muted">
      <LoaderCircle size={16} className="animate-spin" />
      {label}
    </div>
  );
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
