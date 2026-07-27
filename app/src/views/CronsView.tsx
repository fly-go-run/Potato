import * as Dialog from "@radix-ui/react-dialog";
import {
  CalendarClock,
  Clock3,
  History,
  LoaderCircle,
  Pencil,
  Play,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { cronApi } from "../lib/api";
import {
  buildCronSpec,
  CRON_PRESETS,
  findTarget,
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
}

const emptyForm: CronFormValue = {
  name: "",
  cron: "0 9 * * *",
  prompt: "",
  targetKey: "",
};

export function CronsView() {
  const { language, t } = useTranslation();
  const [jobs, setJobs] = useState<JobRow[]>([]);
  const [targets, setTargets] = useState<CronDispatchTarget[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editing, setEditing] = useState<CronJobSpec | "new" | null>(null);
  const [historyJob, setHistoryJob] = useState<CronJobSpec | null>(null);
  const [busyJobId, setBusyJobId] = useState<string | null>(null);

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
            return null;
          }
        }),
      );
      setJobs(specs.map((spec, index) => ({ spec, state: states[index] })));
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

  const act = async (
    job: CronJobSpec,
    action: "pause" | "resume" | "run",
  ) => {
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
    if (
      !job.id ||
      !window.confirm(t("crons.deleteConfirm", { name: job.name }))
    ) {
      return;
    }
    setBusyJobId(job.id);
    setError(null);
    setNotice(null);
    try {
      await cronApi.delete(job.id);
      setJobs((items) => items.filter((item) => item.spec.id !== job.id));
      setNotice(t("crons.deleted", { name: job.name }));
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyJobId(null);
    }
  };

  return (
    <div className="h-full overflow-y-auto bg-surface">
      <div className="mx-auto max-w-6xl px-6 py-8 sm:px-10">
        <header className="mb-6 flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-medium tracking-tight text-ink">
              {t("crons.title")}
            </h1>
            <p className="mt-1 text-sm text-ink-muted">
              {t("crons.subtitle")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setEditing("new")}
            className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface transition-colors hover:bg-accent-hover"
          >
            <Plus size={15} />
            {t("crons.new")}
          </button>
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

        {loading && jobs.length === 0 ? (
          <div className="flex items-center justify-center gap-2 rounded-lg border border-line py-16 text-sm text-ink-muted">
            <LoaderCircle size={16} className="animate-spin" />
            {t("crons.loading")}
          </div>
        ) : jobs.length === 0 ? (
          <div className="flex flex-col items-center rounded-lg border border-dashed border-line px-6 py-16 text-center">
            <CalendarClock size={28} className="text-ink-muted" />
            <h2 className="mt-4 font-medium text-ink">
              {t("crons.emptyTitle")}
            </h2>
            <p className="mt-1 max-w-sm text-sm text-ink-muted">
              {t("crons.emptyDescription")}
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto rounded-lg border border-line">
            <table className="w-full min-w-[56rem] border-collapse text-left">
              <thead className="bg-bubble-tool text-[11px] font-medium uppercase tracking-wide text-ink-muted">
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
                {jobs.map(({ spec, state }) => {
                  const busy = busyJobId === spec.id;
                  const active = isJobActive(spec, state);
                  return (
                    <tr key={spec.id} className="text-sm text-ink-secondary">
                      <td className="px-4 py-3">
                        <div className="font-medium text-ink">{spec.name}</div>
                        {state?.last_status && (
                          <div className="mt-0.5 text-[11px] text-ink-muted">
                            {statusLabel(state.last_status, t)}
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <div>{scheduleLabel(spec.schedule.cron, t)}</div>
                        <div className="mt-0.5 font-mono text-[11px] text-ink-muted">
                          {spec.schedule.cron}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <button
                          type="button"
                          role="switch"
                          aria-checked={active}
                          disabled={busy}
                          onClick={() =>
                            void act(spec, active ? "pause" : "resume")
                          }
                          className="flex items-center gap-2 disabled:opacity-40"
                        >
                          <span
                            className={`relative inline-flex h-5 w-9 shrink-0 rounded-full transition-colors ${
                              active ? "bg-accent" : "bg-line-strong"
                            }`}
                          >
                            <span
                              className={`absolute left-0 top-0.5 h-4 w-4 rounded-full bg-surface shadow-sm transition-transform ${
                                active ? "translate-x-[1.125rem]" : "translate-x-0.5"
                              }`}
                            />
                          </span>
                          <span className="text-xs">
                            {active
                              ? t("crons.enabled")
                              : t("crons.paused")}
                          </span>
                        </button>
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
                            title={t("crons.edit")}
                            disabled={busy}
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
                            onClick={() => void remove(spec)}
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
          </div>
        )}
      </div>

      <CronFormDialog
        editing={editing}
        targets={targets}
        onOpenChange={(open) => {
          if (!open) setEditing(null);
        }}
        onSaved={() => {
          setEditing(null);
          void load();
        }}
      />
      <HistoryDrawer
        job={historyJob}
        onOpenChange={(open) => {
          if (!open) setHistoryJob(null);
        }}
      />
    </div>
  );
}

function CronFormDialog({
  editing,
  targets,
  onOpenChange,
  onSaved,
}: {
  editing: CronJobSpec | "new" | null;
  targets: CronDispatchTarget[];
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const existing = editing && editing !== "new" ? editing : undefined;
  const [form, setForm] = useState<CronFormValue>(emptyForm);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timezone = useMemo(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
    [],
  );

  useEffect(() => {
    if (!editing) return;
    const selectedTarget = existing
      ? {
          channel: existing.dispatch.channel,
          user_id: existing.dispatch.target.user_id,
          session_id: existing.dispatch.target.session_id,
        }
      : targets[0];
    setForm(
      existing
        ? {
            name: existing.name,
            cron: existing.schedule.cron,
            prompt: promptFromSpec(existing),
            targetKey: selectedTarget ? targetKey(selectedTarget) : "",
          }
        : {
            ...emptyForm,
            targetKey: selectedTarget ? targetKey(selectedTarget) : "",
          },
    );
    setError(null);
  }, [editing, existing, targets]);

  const selectedTarget = findTarget(targets, form.targetKey);
  const canSave = Boolean(
    form.name.trim() &&
      form.cron.trim() &&
      form.prompt.trim() &&
      selectedTarget &&
      !saving,
  );
  const preset = CRON_PRESETS.find((item) => item.value === form.cron)?.value ?? "";

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
      onSaved();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog.Root open={editing !== null} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[calc(100%-2rem)] w-[calc(100%-2rem)] max-w-xl -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg border border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {existing
                  ? t("crons.form.editTitle")
                  : t("crons.form.newTitle")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 text-xs text-ink-muted">
                {t("crons.form.description")}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-md p-1 text-ink-muted hover:bg-line/50 hover:text-ink"
              >
                <X size={16} />
              </button>
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
              <input
                autoFocus
                value={form.name}
                onChange={(event) =>
                  setForm((value) => ({ ...value, name: event.target.value }))
                }
                placeholder={t("crons.form.namePlaceholder")}
                className={inputClassName}
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label={t("crons.form.preset")}>
                <select
                  value={preset}
                  onChange={(event) => {
                    if (event.target.value) {
                      setForm((value) => ({
                        ...value,
                        cron: event.target.value,
                      }));
                    }
                  }}
                  className={inputClassName}
                >
                  <option value="">{t("crons.form.customPreset")}</option>
                  {CRON_PRESETS.map((item) => (
                    <option key={item.value} value={item.value}>
                      {t(item.labelKey)}
                    </option>
                  ))}
                </select>
              </Field>
              <Field
                label={t("crons.form.cron")}
                hint={t("crons.form.cronHint")}
              >
                <input
                  value={form.cron}
                  onChange={(event) =>
                    setForm((value) => ({
                      ...value,
                      cron: event.target.value,
                    }))
                  }
                  className={`${inputClassName} font-mono`}
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
                className={`${inputClassName} resize-y`}
              />
            </Field>
            <Field
              label={t("crons.form.target")}
              hint={
                targets.length === 0 ? t("crons.form.noTargets") : undefined
              }
            >
              <select
                value={form.targetKey}
                disabled={targets.length === 0}
                onChange={(event) =>
                  setForm((value) => ({
                    ...value,
                    targetKey: event.target.value,
                  }))
                }
                className={inputClassName}
              >
                {!form.targetKey && (
                  <option value="">{t("crons.form.chooseTarget")}</option>
                )}
                {targets.map((target) => (
                  <option key={targetKey(target)} value={targetKey(target)}>
                    {target.channel} · {target.user_id} · {target.session_id}
                  </option>
                ))}
              </select>
            </Field>
            <div className="flex justify-end gap-2 pt-1">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="rounded-md px-3 py-2 text-xs font-medium text-ink-secondary hover:bg-line/50"
                >
                  {t("crons.form.cancel")}
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={!canSave}
                className="rounded-md bg-accent px-3 py-2 text-xs font-medium text-surface hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
              >
                {saving ? t("crons.form.saving") : t("crons.form.save")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
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
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/20" />
        <Dialog.Content className="fixed inset-y-0 right-0 z-50 flex w-[min(28rem,calc(100%-2rem))] flex-col border-l border-line bg-raised shadow-raised outline-none">
          <header className="flex items-start gap-3 border-b border-line px-5 py-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="font-medium text-ink">
                {t("crons.historyTitle")}
              </Dialog.Title>
              <Dialog.Description className="mt-0.5 truncate text-xs text-ink-muted">
                {t("crons.historyDescription", { name: job?.name ?? "" })}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-md p-1 text-ink-muted hover:bg-line/50 hover:text-ink"
              >
                <X size={16} />
              </button>
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {loading ? (
              <div className="flex items-center justify-center gap-2 py-12 text-sm text-ink-muted">
                <LoaderCircle size={16} className="animate-spin" />
                {t("crons.historyLoading")}
              </div>
            ) : error ? (
              <div className="rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
                {error}
              </div>
            ) : records.length === 0 ? (
              <div className="py-12 text-center text-sm text-ink-muted">
                {t("crons.historyEmpty")}
              </div>
            ) : (
              <div className="space-y-2">
                {records.map((record, index) => (
                  <article
                    key={`${record.run_at}-${index}`}
                    className="rounded-md border border-line bg-surface p-3"
                  >
                    <div className="flex items-start gap-3">
                      <Clock3 size={15} className="mt-0.5 shrink-0 text-ink-muted" />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center justify-between gap-2">
                          <time className="text-xs font-medium text-ink">
                            {formatDate(record.run_at, language)}
                          </time>
                          <StatusBadge status={record.status} />
                        </div>
                        <p className="mt-1 text-xs text-ink-muted">
                          {record.error ||
                            t(
                              record.trigger === "manual"
                                ? "crons.trigger.manual"
                                : "crons.trigger.scheduled",
                            )}
                        </p>
                      </div>
                    </div>
                  </article>
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
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`rounded-md p-2 transition-colors disabled:opacity-30 ${
        danger
          ? "text-ink-muted hover:bg-danger-soft hover:text-danger"
          : "text-ink-muted hover:bg-line/50 hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

function StatusBadge({
  status,
}: {
  status: CronExecutionRecord["status"];
}) {
  const { t } = useTranslation();
  const tone =
    status === "success"
      ? "bg-accent-soft text-ok"
      : status === "error"
        ? "bg-danger-soft text-danger"
        : "bg-bubble-tool text-ink-secondary";
  return (
    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${tone}`}>
      {statusLabel(status, t)}
    </span>
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

const inputClassName =
  "mt-1.5 block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none transition-colors focus:border-line-strong disabled:cursor-not-allowed disabled:bg-bubble-tool disabled:text-ink-muted";

function scheduleLabel(
  cron: string,
  t: ReturnType<typeof useTranslation>["t"],
) {
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
