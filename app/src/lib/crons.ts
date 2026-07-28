export type CronSchedule =
  | {
      type: "cron";
      cron: string;
      timezone?: string;
    }
  | {
      type: "once";
      run_at?: string;
      at?: string;
      timezone?: string;
      [key: string]: unknown;
    };

export interface CronDispatchTarget {
  channel: string;
  user_id: string;
  session_id: string;
}

export interface CronDispatch {
  type: "channel";
  channel: string;
  target: {
    user_id: string;
    session_id: string;
  };
  mode?: "stream" | "final";
  silent?: boolean;
  meta?: Record<string, unknown>;
}

export interface CronJobSpec {
  id?: string;
  name: string;
  enabled: boolean;
  schedule: CronSchedule;
  task_type: "agent" | "text" | string;
  request: {
    input: unknown;
    session_id?: string | null;
    user_id?: string | null;
    [key: string]: unknown;
  } | null;
  dispatch: CronDispatch;
  save_result_to_inbox?: boolean;
  runtime?: Record<string, unknown>;
  meta?: Record<string, unknown>;
}

export interface CronJobState {
  next_run_at: string | null;
  last_run_at: string | null;
  last_status:
    | "success"
    | "error"
    | "running"
    | "skipped"
    | "cancelled"
    | null;
  last_error: string | null;
}

export interface CronExecutionRecord {
  run_at: string;
  status: "success" | "error" | "running" | "skipped" | "cancelled";
  error?: string | null;
  trigger: "scheduled" | "manual";
}

export interface CronFormValue {
  name: string;
  cron: string;
  prompt: string;
  targetKey: string;
}

export const CRON_PRESETS = [
  { value: "0 * * * *", labelKey: "crons.preset.hourly" },
  { value: "0 9 * * *", labelKey: "crons.preset.dailyNine" },
  { value: "0 9 * * mon", labelKey: "crons.preset.mondayNine" },
] as const;

export function targetKey(target: CronDispatchTarget): string {
  return JSON.stringify([
    target.channel,
    target.user_id,
    target.session_id,
  ]);
}

export function findTarget(
  targets: CronDispatchTarget[],
  key: string,
): CronDispatchTarget | null {
  return targets.find((target) => targetKey(target) === key) ?? null;
}

export function buildCronSpec(
  form: CronFormValue,
  target: CronDispatchTarget,
  timezone: string,
  existing?: CronJobSpec,
): CronJobSpec {
  const prompt = form.prompt.trim();
  const input = [
    {
      role: "user",
      type: "message",
      content: [{ type: "text", text: prompt }],
    },
  ];
  return {
    ...(existing ?? {}),
    name: form.name.trim(),
    enabled: existing?.enabled ?? true,
    schedule: {
      type: "cron",
      cron: form.cron.trim(),
      timezone,
    },
    task_type: "agent",
    request: {
      ...(existing?.request ?? {}),
      input,
    },
    dispatch: {
      ...(existing?.dispatch ?? {}),
      type: "channel",
      channel: target.channel,
      target: {
        user_id: target.user_id,
        session_id: target.session_id,
      },
    },
  };
}

export function promptFromSpec(spec: CronJobSpec): string {
  if (!spec.request) return "";
  const input = spec.request.input;
  if (!Array.isArray(input)) return "";
  for (const message of input) {
    if (!message || typeof message !== "object") continue;
    const content = (message as { content?: unknown }).content;
    if (typeof content === "string") return content;
    if (!Array.isArray(content)) continue;
    for (const part of content) {
      if (
        part &&
        typeof part === "object" &&
        typeof (part as { text?: unknown }).text === "string"
      ) {
        return (part as { text: string }).text;
      }
    }
  }
  return "";
}

/** The compact editor only round-trips recurring agent jobs safely. */
export function isCronJobEditable(spec: CronJobSpec): boolean {
  return (
    spec.schedule.type === "cron" &&
    spec.task_type === "agent" &&
    spec.request !== null
  );
}

export function cronExpression(spec: CronJobSpec): string | null {
  return spec.schedule.type === "cron" ? spec.schedule.cron : null;
}
