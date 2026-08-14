/**
 * 工具结果结构化契约（RFC tool-runtime r2 §1）。
 *
 * 后端在 FunctionCallOutput.meta 携带 metadata["qp"]：
 * `{ v: 1, kind, ok, data }`。meta 是纯增量通道——历史会话、取消的
 * 调用、未迁移的工具都没有它，所有消费方必须容忍 null 并回落到
 * 既有的文本解析路径。
 */

export type QpKind =
  | "file_write"
  | "file_edit"
  | "file_read"
  | "shell"
  | "file_sent"
  | "web_search"
  | "batch";

export interface QpMeta {
  v: number;
  kind: QpKind | (string & {});
  ok: boolean;
  data: Record<string, unknown>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

/**
 * 宽容解析：形状不对一律返回 null（畸形 meta 不能让卡片崩溃）。
 * 只认 v === 1；未来版本升级由这里统一挡住,消费方不用各自判版本。
 */
export function parseQpMeta(raw: unknown): QpMeta | null {
  if (!isRecord(raw)) return null;
  if (raw.v !== 1) return null;
  if (typeof raw.kind !== "string" || !raw.kind) return null;
  if (typeof raw.ok !== "boolean") return null;
  if (!isRecord(raw.data)) return null;
  return { v: raw.v, kind: raw.kind, ok: raw.ok, data: raw.data };
}

/** data 里的非负整数字段；缺失或形状不对返回 null,绝不猜。 */
export function qpCount(meta: QpMeta | null, key: string): number | null {
  if (!meta) return null;
  const raw = meta.data[key];
  if (typeof raw !== "number" || !Number.isFinite(raw) || raw < 0) return null;
  return raw;
}

export function qpBool(meta: QpMeta | null, key: string): boolean | null {
  if (!meta) return null;
  const raw = meta.data[key];
  return typeof raw === "boolean" ? raw : null;
}

export function qpString(meta: QpMeta | null, key: string): string | null {
  if (!meta) return null;
  const raw = meta.data[key];
  return typeof raw === "string" ? raw : null;
}

// ---------------------------------------------------------------------------
// legacy 路径计数器（仅 dev）：验收标准是"新会话七类工具跑完计数为零"。
// 生产构建里 recordLegacyParse 是 no-op,不引入任何开销。
// ---------------------------------------------------------------------------

const legacyCounts = new Map<string, number>();

export function recordLegacyParse(seam: string): void {
  if (!import.meta.env.DEV) return;
  legacyCounts.set(seam, (legacyCounts.get(seam) ?? 0) + 1);
}

export function legacyParseCounts(): Readonly<Record<string, number>> {
  return Object.fromEntries(legacyCounts);
}

export function resetLegacyParseCounts(): void {
  legacyCounts.clear();
}
