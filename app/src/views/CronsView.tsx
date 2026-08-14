import * as Dialog from "@radix-ui/react-dialog";
import {
  Archive,
  BarChart3,
  BookOpen,
  CalendarClock,
  Clock3,
  FileText,
  History,
  ListChecks,
  Mail,
  Newspaper,
  Pencil,
  Play,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useChatStore } from "../stores/chat";
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
  Select,
  SkeletonRows,
  Switch,
  inputClasses,
} from "../components/ui";
import { cronApi } from "../lib/api";
import {
  buildCronSpec,
  cronExpression,
  CRON_PRESETS,
  findTarget,
  isCronJobEditable,
  promptFromSpec,
  targetKey,
  type CronDispatchTarget,
  type CronExecutionRecord,
  type CronFormValue,
  type CronJobSpec,
  type CronJobState,
} from "../lib/crons";
import { useTranslation } from "../lib/i18n";

interface JobRow {
  spec: CronJobSpec;
  state: CronJobState | null;
  /** 状态接口失败时为 true:不能把"读不到状态"伪装成"已启用"。 */
  stateFailed?: boolean;
}

interface GlobalRun {
  job: CronJobSpec;
  record: CronExecutionRecord;
  index: number;
}

const emptyForm: CronFormValue = {
  name: "",
  cron: "0 9 * * *",
  prompt: "",
  targetKey: "",
};

const CRON_TEMPLATES = [
  {
    icon: FileText,
    nameKey: "crons.templates.weeklyReport.name",
    cron: "0 17 * * 5",
    promptKey: "crons.templates.weeklyReport.prompt",
  },
  {
    icon: CalendarClock,
    nameKey: "crons.templates.meetingPrep.name",
    cron: "30 9 * * 1-5",
    promptKey: "crons.templates.meetingPrep.prompt",
  },
  {
    icon: Newspaper,
    nameKey: "crons.templates.dailyNews.name",
    cron: "0 9 * * 1-5",
    promptKey: "crons.templates.dailyNews.prompt",
  },
  {
    icon: ListChecks,
    nameKey: "crons.templates.fridayTodos.name",
    cron: "0 16 * * 5",
    promptKey: "crons.templates.fridayTodos.prompt",
  },
  {
    icon: Mail,
    nameKey: "crons.templates.emailReminder.name",
    cron: "0 18 * * 1-5",
    promptKey: "crons.templates.emailReminder.prompt",
  },
  {
    icon: BarChart3,
    nameKey: "crons.templates.monthlyReport.name",
    cron: "0 10 1 * *",
    promptKey: "crons.templates.monthlyReport.prompt",
  },
  {
    icon: BookOpen,
    nameKey: "crons.templates.dailyLearning.name",
    cron: "0 8 * * *",
    promptKey: "crons.templates.dailyLearning.prompt",
  },
  {
    icon: Archive,
    nameKey: "crons.templates.fileArchive.name",
    cron: "0 17 * * 5",
    promptKey: "crons.templates.fileArchive.prompt",
  },
] as const;

type CronTemplateDraft = Pick<CronFormValue, "name" | "cron" | "prompt">;

export function CronsView() {
  const { language, t } = useTranslation();
  const [jobs, setJobs] = useState<JobRow[]>([]);
  const [targets, setTargets] = useState<CronDispatchTarget[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editing, setEditing] = useState<CronJobSpec | "new" | null>(null);
  const [newDraft, setNewDraft] = useState<CronTemplateDraft | null>(null);
  const [historyJob, setHistoryJob] = useState<CronJobSpec | null>(null);
  const [busyJobId, setBusyJobId] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CronJobSpec | null>(null);
  const [view, setView] = useState<"tasks" | "runs">("tasks");
  const [globalRuns, setGlobalRuns] = useState<GlobalRun[]>([]);
  const [globalRunsLoading, setGlobalRunsLoading] = useState(false);
  const [globalRunsError, setGlobalRunsError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [specs, targetResponse] = await Promise.all([
        cronApi.list(),
        cronApi.dispatchTargets(),
      ]);
      const states = await Promise.all(
        specs.map(async (spec) => {
          if (!spec.id) return null;
          try {
            return await cronApi.state(spec.id);
          } catch {
            return "failed" as const;
          }
        }),
      );
      setJobs(
        specs.map((spec, index) => ({
          spec,
          state: states[index] === "failed" ? null : states[index],
          stateFailed: states[index] === "failed",
        })),
      );
      setTargets(targetResponse.items);
    } catch (reason) {
      setError(t("crons.loadFailed", { message: readableError(reason) }));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  useEffect(() => {
    if (view !== "runs" || loading) return;
    let active = true;
    setGlobalRunsLoading(true);
    setGlobalRunsError(null);
    void Promise.allSettled(
      jobs.map(async ({ spec }) => ({
        spec,
        records: spec.id ? await cronApi.history(spec.id) : [],
      })),
    )
      .then((results) => {
        if (!active) return;
        const runs = results
          .flatMap((result) =>
            result.status === "fulfilled"
              ? result.value.records.map((record, index) => ({
                  job: result.value.spec,
                  record,
                  index,
                }))
              : [],
          )
          .sort(
            (left, right) =>
              Date.parse(right.record.run_at) - Date.parse(left.record.run_at),
          );
        setGlobalRuns(runs);
        const failed = results.filter(
          (result) => result.status === "rejected",
        ).length;
        if (failed > 0) {
          setGlobalRunsError(t("crons.runs.partialFailed", { count: failed }));
        }
      })
      .finally(() => {
        if (active) setGlobalRunsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [jobs, loading, t, view]);

  const act = async (job: CronJobSpec, action: "pause" | "resume" | "run") => {
    if (!job.id) return;
    setBusyJobId(job.id);
    setError(null);
    setNotice(null);
    try {
      await cronApi.action(job.id, action);
      if (action === "run") {
        setNotice(t("crons.runStarted", { name: job.name }));
      } else {
        await cronApi.replace(job.id, {
          ...job,
          enabled: action === "resume",
        });
      }
      await load();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyJobId(null);
    }
  };

  const remove = async (job: CronJobSpec) => {
    if (!job.id) return;
    setBusyJobId(job.id);
    setError(null);
    setNotice(null);
    try {
      await cronApi.delete(job.id);
      setJobs((items) => items.filter((item) => item.spec.id !== job.id));
      setNotice(t("crons.deleted", { name: job.name }));
      setPendingDelete(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyJobId(null);
    }
  };

  const openNew = (draft: CronTemplateDraft | null = null) => {
    setNewDraft(draft);
    setEditing("new");
  };

  return (
    <>
      <PageContainer width="wide">
        <PageHeader
          title={t("crons.title")}
          subtitle={t("crons.subtitle")}
          // 一屏一个主操作：空态时中心已有「新建任务」，页头不再重复；
          // 副标题同理只在空态（首次访问）出现。
          showSubtitle={jobs.length === 0}
          actions={
            view === "runs" || jobs.length === 0 ? undefined : (
              <Button variant="primary" size="sm" onClick={() => openNew()}>
                <Plus size={15} />
                {t("crons.new")}
              </Button>
            )
          }
        />

        <SegmentedControl
          value={view}
          variant="track"
          options={[
            {
              value: "tasks",
              label: t("crons.tabs.tasks"),
              count: jobs.length,
            },
            {
              value: "runs",
              label: t("crons.tabs.runs"),
              count: globalRuns.length || undefined,
            },
          ]}
          onChange={setView}
          className="mb-5"
        />

        {(error || notice) && (
          <div
            className={`mb-5 rounded-md px-3 py-2 text-xs ${
              error ? "bg-danger-soft text-danger" : "bg-fill-active text-ok"
            }`}
          >
            {error || notice}
          </div>
        )}

        {view === "runs" ? (
          <GlobalRunHistory
            jobs={jobs}
            runs={globalRuns}
            loading={globalRunsLoading}
            error={globalRunsError}
            language={language}
          />
        ) : loading && jobs.length === 0 ? (
          <Card className="p-4">
            <SkeletonRows rows={6} />
          </Card>
        ) : error && jobs.length === 0 ? (
          // 加载失败不能伪装成"暂无任务"再递上新建按钮
          <Card className="flex items-center justify-between gap-3 p-4 text-sm text-ink-secondary">
            <span>{error}</span>
            <Button variant="secondary" size="sm" onClick={() => void load()}>
              {t("common.retry")}
            </Button>
          </Card>
        ) : jobs.length === 0 ? (
          <EmptyState
            icon={<CalendarClock size={20} />}
            title={t("crons.emptyTitle")}
            description={t("crons.emptyDescription")}
            action={
              <Button variant="primary" size="sm" onClick={() => openNew()}>
                <Plus size={15} />
                {t("crons.new")}
              </Button>
            }
          />
        ) : (
          <Card className="overflow-x-auto">
            <table className="w-full min-w-[56rem] border-collapse text-left">
              <thead className="bg-bubble-tool text-[11px] font-medium uppercase tracking-wide text-ink-tertiary">
                <tr>
                  <th className="px-4 py-3">{t("crons.nameColumn")}</th>
                  <th className="px-4 py-3">{t("crons.scheduleColumn")}</th>
                  <th className="px-4 py-3">{t("crons.statusColumn")}</th>
                  <th className="px-4 py-3">{t("crons.lastRunColumn")}</th>
                  <th className="px-4 py-3">{t("crons.nextRunColumn")}</th>
                  <th className="px-4 py-3 text-right">
                    {t("crons.actionsColumn")}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {jobs.map(({ spec, state, stateFailed }) => {
                  const busy = busyJobId === spec.id;
                  const active = isJobActive(spec, state);
                  return (
                    <tr key={spec.id} className="text-sm text-ink-secondary">
                      <td className="px-4 py-3">
                        <div className="font-medium text-ink">{spec.name}</div>
                        {state?.last_status && (
                          <div className="mt-1">
                            <StatusBadge status={state.last_status} />
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <div>{scheduleLabel(spec, t)}</div>
                        <div className="mt-0.5 font-mono text-[11px] text-ink-tertiary">
                          {cronExpression(spec) ??
                            (spec.schedule.type === "once"
                              ? spec.schedule.run_at ?? spec.schedule.at
                              : null) ??
                            "—"}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <Switch
                            checked={active}
                            disabled={busy}
                            onChange={() =>
                              void act(spec, active ? "pause" : "resume")
                            }
                            aria-label={t("crons.toggleLabel", {
                              name: spec.name,
                              status: active
                                ? t("crons.enabled")
                                : t("crons.paused"),
                            })}
                          />
                          <span
                            className={
                              stateFailed ? "text-xs text-warn" : "text-xs"
                            }
                          >
                            {stateFailed
                              ? t("crons.stateUnknown")
                              : active
                              ? t("crons.enabled")
                              : t("crons.paused")}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3 text-xs">
                        {state?.last_run_at
                          ? formatDate(state.last_run_at, language)
                          : t("crons.never")}
                      </td>
                      <td className="px-4 py-3 text-xs">
                        {state?.next_run_at
                          ? formatDate(state.next_run_at, language)
                          : t("crons.noNextRun")}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center justify-end gap-0.5">
                          <ActionButton
                            title={
                              isCronJobEditable(spec)
                                ? t("crons.edit")
                                : t("crons.editUnsupported")
                            }
                            disabled={busy || !isCronJobEditable(spec)}
                            onClick={() => setEditing(spec)}
                          >
                            <Pencil size={15} />
                          </ActionButton>
                          <ActionButton
                            title={t("crons.runNow")}
                            disabled={busy}
                            onClick={() => void act(spec, "run")}
                          >
                            <Play size={15} />
                          </ActionButton>
                          <ActionButton
                            title={t("crons.history")}
                            disabled={busy}
                            onClick={() => setHistoryJob(spec)}
                          >
                            <History size={15} />
                          </ActionButton>
                          <ActionButton
                            title={t("crons.delete")}
                            disabled={busy}
                            danger
                            onClick={() => setPendingDelete(spec)}
                          >
                            <Trash2 size={15} />
                          </ActionButton>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Card>
        )}

        {view === "tasks" && (
          <section className="mt-8" aria-labelledby="cron-templates-title">
            <div className="mb-3 flex items-center">
              <h2
                id="cron-templates-title"
                className="text-sm font-medium text-ink"
              >
                {t("crons.templates.title")}
              </h2>
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {CRON_TEMPLATES.map((template) => {
                const Icon = template.icon;
                const name = t(template.nameKey);
                const prompt = t(template.promptKey);
                return (
                  <button
                    key={template.nameKey}
                    type="button"
                    className="flex min-w-0 items-start gap-3 rounded-[var(--radius-md)] border border-line bg-surface p-4 text-left shadow-[var(--shadow-sm)] transition-colors duration-[var(--dur-fast)] hover:border-line-strong focus-visible:border-line-strong focus-visible:outline-none focus-visible:shadow-[0_0_0_3px_var(--ring)]"
                    onClick={() =>
                      openNew({
                        name,
                        cron: template.cron,
                        prompt,
                      })
                    }
                  >
                    <Icon
                      size={16}
                      className="mt-0.5 shrink-0 text-icon"
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-ink">
                        {name}
                      </span>
                      <span className="line-clamp-1 text-[13px] leading-5 text-ink-tertiary">
                        {prompt}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          </section>
        )}
      </PageContainer>

      <CronFormDialog
        editing={editing}
        newDraft={newDraft}
        targets={targets}
        onOpenChange={(open) => {
          if (!open) {
            setEditing(null);
            setNewDraft(null);
          }
        }}
        onSaved={(updated) => {
          setEditing(null);
          setNewDraft(null);
          // 保存接口已成功,反馈不依赖随后的列表刷新是否顺利。
          setNotice(t(updated ? "crons.updatedNotice" : "crons.savedNotice"));
          void load();
        }}
      />
      <HistoryDrawer
        job={historyJob}
        onOpenChange={(open) => {
          if (!open) setHistoryJob(null);
        }}
      />
      <ConfirmDialog
        open={pendingDelete !== null}
        title={t("crons.delete")}
        description={
          pendingDelete
            ? t("crons.deleteConfirm", { name: pendingDelete.name })
            : undefined
        }
        tone="danger"
        busy={pendingDelete?.id === busyJobId}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        onConfirm={() => pendingDelete && void remove(pendingDelete)}
      />
    </>
  );
}

function CronFormDialog({
  editing,
  newDraft,
  targets,
  onOpenChange,
  onSaved,
}: {
  editing: CronJobSpec | "new" | null;
  newDraft: CronTemplateDraft | null;
  targets: CronDispatchTarget[];
  onOpenChange: (open: boolean) => void;
  onSaved: (updated: boolean) => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const chats = useChatStore((state) => state.chats);
  const existing = editing && editing !== "new" ? editing : undefined;
  const [form, setForm] = useState<CronFormValue>(emptyForm);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 已有任务的目标可能已不在候选里(会话被删/超出候选上限):
  // 注入进列表,保证编辑时能原样保留原目标,而不是整个表单卡死。
  const effectiveTargets = useMemo(() => {
    if (!existing) return targets;
    const existingTarget = {
      channel: existing.dispatch.channel,
      user_id: existing.dispatch.target.user_id,
      session_id: existing.dispatch.target.session_id,
    };
    return findTarget(targets, targetKey(existingTarget))
      ? targets
      : [existingTarget, ...targets];
  }, [existing, targets]);

  /** 投递目标用会话名展示,裸 id 只留在悬停提示里。 */
  const targetLabel = (target: CronDispatchTarget) => {
    const chat = chats.find((item) => item.session_id === target.session_id);
    if (chat?.name) return `${chat.name} · ${target.channel}`;
    return `${target.channel} · ${target.session_id.slice(0, 12)}…`;
  };
  const timezone = useMemo(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    [],
  );

  useEffect(() => {
    if (!editing) return;
    // 新任务不默认选中投递目标:结果发给谁必须用户显式决定,
    // 默认选第一条存在误投风险(sol review P0)。
    const selectedTarget = existing
      ? {
          channel: existing.dispatch.channel,
          user_id: existing.dispatch.target.user_id,
          session_id: existing.dispatch.target.session_id,
        }
      : undefined;
    setForm(
      existing
        ? {
            name: existing.name,
            cron: cronExpression(existing) ?? emptyForm.cron,
            prompt: promptFromSpec(existing),
            targetKey: selectedTarget ? targetKey(selectedTarget) : "",
          }
        : {
            ...emptyForm,
            ...newDraft,
            targetKey: selectedTarget ? targetKey(selectedTarget) : "",
          },
    );
    setError(null);
  }, [editing, existing, newDraft, targets]);

  const selectedTarget = findTarget(effectiveTargets, form.targetKey);
  const canSave = Boolean(
    form.name.trim() &&
      form.cron.trim() &&
      form.prompt.trim() &&
      selectedTarget &&
      !saving,
  );
  const preset =
    CRON_PRESETS.find((item) => item.value === form.cron)?.value ?? "";

  const save = async () => {
    if (!canSave || !selectedTarget) return;
    setSaving(true);
    setError(null);
    try {
      const spec = buildCronSpec(
        form,
        selectedTarget,
        existing?.schedule.timezone || timezone,
        existing,
      );
      if (existing?.id) await cronApi.replace(existing.id, spec);
      else await cronApi.create(spec);
      onSaved(Boolean(existing));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog.Root open={editing !== null} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 max-h-[calc(100%-2rem)] w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-[var(--radius-lg)] border border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {existing
                  ? t("crons.form.editTitle")
                  : t("crons.form.newTitle")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-tertiary">
                {t("crons.form.description")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton size="sm" title={t("common.cancel")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
            className="space-y-4 p-5"
          >
            {error && (
              <div className="rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
                {error}
              </div>
            )}
            <Field label={t("crons.form.name")}>
              <Input
                autoFocus
                value={form.name}
                onChange={(event) =>
                  setForm((value) => ({ ...value, name: event.target.value }))
                }
                placeholder={t("crons.form.namePlaceholder")}
                className="mt-1.5"
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label={t("crons.form.preset")}>
                <Select
                  value={preset}
                  onChange={(event) => {
                    if (event.target.value) {
                      setForm((value) => ({
                        ...value,
                        cron: event.target.value,
                      }));
                    }
                  }}
                  className="mt-1.5"
                >
                  <option value="">{t("crons.form.customPreset")}</option>
                  {CRON_PRESETS.map((item) => (
                    <option key={item.value} value={item.value}>
                      {t(item.labelKey)}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field
                label={t("crons.form.cron")}
                hint={t("crons.form.cronHint")}
              >
                <Input
                  value={form.cron}
                  onChange={(event) =>
                    setForm((value) => ({
                      ...value,
                      cron: event.target.value,
                    }))
                  }
                  className="mt-1.5 font-mono"
                />
              </Field>
            </div>
            <Field label={t("crons.form.prompt")}>
              <textarea
                rows={5}
                value={form.prompt}
                onChange={(event) =>
                  setForm((value) => ({ ...value, prompt: event.target.value }))
                }
                placeholder={t("crons.form.promptPlaceholder")}
                className={`${inputClasses} mt-1.5 resize-y py-2`}
              />
            </Field>
            <Field
              label={t("crons.form.target")}
              hint={
                effectiveTargets.length === 0
                  ? t("crons.form.noTargets")
                  : undefined
              }
            >
              <Select
                value={form.targetKey}
                disabled={effectiveTargets.length === 0}
                onChange={(event) =>
                  setForm((value) => ({
                    ...value,
                    targetKey: event.target.value,
                  }))
                }
                className="mt-1.5"
              >
                {!form.targetKey && (
                  <option value="">{t("crons.form.targetPlaceholder")}</option>
                )}
                {effectiveTargets.map((target) => (
                  <option
                    key={targetKey(target)}
                    value={targetKey(target)}
                    title={`${target.channel} · ${target.user_id} · ${target.session_id}`}
                  >
                    {targetLabel(target)}
                  </option>
                ))}
              </Select>
              {effectiveTargets.length === 0 && (
                <Button
                  variant="secondary"
                  size="sm"
                  className="mt-2"
                  onClick={() => navigate("/")}
                >
                  {t("crons.form.noTargetCta")}
                </Button>
              )}
            </Field>
            <div className="flex justify-end gap-2 pt-1">
              <Dialog.Close asChild>
                <Button variant="ghost" size="sm">
                  {t("crons.form.cancel")}
                </Button>
              </Dialog.Close>
              <Button
                type="submit"
                variant="primary"
                size="sm"
                disabled={!canSave}
              >
                {saving ? t("crons.form.saving") : t("crons.form.save")}
              </Button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function GlobalRunHistory({
  jobs,
  runs,
  loading,
  error,
  language,
}: {
  jobs: JobRow[];
  runs: GlobalRun[];
  loading: boolean;
  error: string | null;
  language: "zh" | "en";
}) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <Card className="p-4">
        <SkeletonRows rows={6} />
      </Card>
    );
  }

  if (runs.length === 0) {
    return (
      <div>
        {error && (
          <div className="mb-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
            {error}
          </div>
        )}
        <EmptyState
          icon={<History size={20} />}
          title={t("crons.runs.emptyTitle")}
          description={
            jobs.length === 0
              ? t("crons.runs.noTasksDescription")
              : t("crons.runs.emptyDescription")
          }
        />
      </div>
    );
  }

  return (
    <div>
      {error && (
        <div className="mb-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
          {error}
        </div>
      )}
      <Card className="overflow-x-auto">
        <table className="w-full min-w-[42rem] border-collapse text-left">
          <thead className="bg-bubble-tool text-[11px] font-medium uppercase tracking-wide text-ink-tertiary">
            <tr>
              <th className="px-4 py-3">{t("crons.runs.taskColumn")}</th>
              <th className="px-4 py-3">{t("crons.runs.timeColumn")}</th>
              <th className="px-4 py-3">{t("crons.runs.triggerColumn")}</th>
              <th className="px-4 py-3">{t("crons.runs.resultColumn")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {runs.map(({ job, record, index }) => (
              <tr
                key={`${job.id ?? job.name}-${record.run_at}-${index}`}
                className="text-[13px] text-ink-secondary"
              >
                <td className="px-4 py-3 font-medium text-ink">{job.name}</td>
                <td className="whitespace-nowrap px-4 py-3 text-xs">
                  {formatDate(record.run_at, language)}
                </td>
                <td className="px-4 py-3 text-xs">
                  {t(
                    record.trigger === "manual"
                      ? "crons.trigger.manual"
                      : "crons.trigger.scheduled",
                  )}
                </td>
                <td className="max-w-[22rem] px-4 py-3">
                  <div className="flex items-center gap-2">
                    <StatusBadge status={record.status} />
                    {record.error && (
                      <span
                        className="min-w-0 truncate text-xs text-danger"
                        title={record.error}
                      >
                        {record.error}
                      </span>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
    </div>
  );
}

function HistoryDrawer({
  job,
  onOpenChange,
}: {
  job: CronJobSpec | null;
  onOpenChange: (open: boolean) => void;
}) {
  const { language, t } = useTranslation();
  const [records, setRecords] = useState<CronExecutionRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!job?.id) return;
    setLoading(true);
    setError(null);
    void cronApi
      .history(job.id)
      .then(setRecords)
      .catch((reason: unknown) => setError(readableError(reason)))
      .finally(() => setLoading(false));
  }, [job]);

  return (
    <Dialog.Root open={job !== null} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-drawer fixed inset-y-0 right-0 z-50 flex w-[min(28rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-[var(--shadow-lg)] outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {t("crons.historyTitle")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 truncate text-xs text-ink-tertiary">
                {t("crons.historyDescription", { name: job?.name ?? "" })}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <IconButton size="sm" title={t("common.cancel")}>
                <X size={16} />
              </IconButton>
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {loading ? (
              <div className="py-2">
                <SkeletonRows rows={5} />
              </div>
            ) : error ? (
              <div className="rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
                {error}
              </div>
            ) : records.length === 0 ? (
              <div className="py-12 text-center text-sm text-ink-tertiary">
                {t("crons.historyEmpty")}
              </div>
            ) : (
              <div className="space-y-2">
                {records.map((record, index) => (
                  <Card
                    key={`${record.run_at}-${index}`}
                    className="rounded-[var(--radius-md)] p-3"
                  >
                    <div className="flex items-start gap-3">
                      <Clock3
                        size={15}
                        className="mt-0.5 shrink-0 text-icon"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center justify-between gap-2">
                          <time className="text-xs font-medium text-ink">
                            {formatDate(record.run_at, language)}
                          </time>
                          <StatusBadge status={record.status} />
                        </div>
                        <p className="mt-1 text-xs text-ink-tertiary">
                          {record.error ||
                            t(
                              record.trigger === "manual"
                                ? "crons.trigger.manual"
                                : "crons.trigger.scheduled",
                            )}
                        </p>
                      </div>
                    </div>
                  </Card>
                ))}
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ActionButton({
  title,
  disabled,
  danger = false,
  onClick,
  children,
}: {
  title: string;
  disabled: boolean;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <IconButton
      size="sm"
      tone={danger ? "danger" : "default"}
      title={title}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </IconButton>
  );
}

function StatusBadge({ status }: { status: CronExecutionRecord["status"] }) {
  const { t } = useTranslation();
  const tone =
    status === "success"
      ? "ok"
      : status === "error"
      ? "danger"
      : status === "running"
      ? "accent"
      : "neutral";
  return <Badge tone={tone}>{statusLabel(status, t)}</Badge>;
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
        <span className="mt-1 block font-normal text-ink-tertiary">{hint}</span>
      )}
    </label>
  );
}

function scheduleLabel(
  spec: CronJobSpec,
  t: ReturnType<typeof useTranslation>["t"],
) {
  if (spec.schedule.type === "once") return t("crons.schedule.once");
  const cron = spec.schedule.cron;
  const preset = CRON_PRESETS.find((item) => item.value === cron);
  return preset ? t(preset.labelKey) : t("crons.form.customPreset");
}

function isJobActive(spec: CronJobSpec, state: CronJobState | null) {
  if (!spec.enabled) return false;
  return state === null ? true : Boolean(state.next_run_at);
}

function statusLabel(
  status: NonNullable<CronJobState["last_status"]>,
  t: ReturnType<typeof useTranslation>["t"],
) {
  switch (status) {
    case "success":
      return t("crons.status.success");
    case "error":
      return t("crons.status.error");
    case "running":
      return t("crons.status.running");
    case "skipped":
      return t("crons.status.skipped");
    case "cancelled":
      return t("crons.status.cancelled");
  }
}

function formatDate(value: string, language: "zh" | "en") {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
