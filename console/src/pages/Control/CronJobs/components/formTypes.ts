import type dayjs from "dayjs";
import type {
  CronJobRuntime,
  CronJobSchedule,
  CronJobSpecInput,
} from "../../../../api/types";
import type { CronType } from "./parseCron";

type FormJsonValue = string | number | boolean | object;

/**
 * Values held by the drawer, including temporary UI-only scheduling fields.
 * They are converted to the stricter API input shape on submit.
 */
export interface CronJobFormValues {
  id?: CronJobSpecInput["id"];
  name?: string;
  enabled?: boolean;
  save_result_to_inbox?: boolean;
  schedule?: Partial<CronJobSchedule>;
  task_type?: CronJobSpecInput["task_type"];
  text?: string;
  request?: {
    input?: FormJsonValue;
    session_id?: string | null;
    user_id?: string | null;
  };
  dispatch?: {
    type: "channel";
    channel?: string;
    target: { user_id: string; session_id: string };
    mode?: "stream" | "final";
    silent?: boolean;
  };
  runtime?: CronJobRuntime;
  meta?: Record<string, FormJsonValue>;

  scheduleType?: "cron" | "once";
  onceRunAt?: dayjs.Dayjs | null;
  onceRepeatEnabled?: boolean;
  onceRepeatEveryDays?: number;
  onceRepeatEndType?: "never" | "until" | "count";
  onceRepeatUntil?: dayjs.Dayjs | null;
  onceRepeatCount?: number;
  cronType?: CronType;
  cronTime?: dayjs.Dayjs | null;
  cronDaysOfWeek?: string[];
  cronCustom?: string;
}
