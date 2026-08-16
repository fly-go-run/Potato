import { humanToolName } from "../components/chat/ToolCard";
import { type Language, type TranslationKey } from "./i18n";
import { skillDisplayName } from "./skillPresentation";
import type { ToolFamily, ToolGroupRow } from "./stepGroups";

type Translate = (
  key: TranslationKey,
  params?: Record<string, string | number>,
) => string;

const SEARCH_FAMILIES = new Set<ToolFamily>(["search", "grep", "glob"]);

/**
 * Fold-row object text. Icon carries the verb; this is only the object
 * (+ ± / ×N / 等) so a missing object never leaves a hanging unit.
 */
export function formatStepGroupObject(
  row: ToolGroupRow,
  translate: Translate,
  language: Language,
): string {
  if (SEARCH_FAMILIES.has(row.family)) {
    return formatSearchObject(row);
  }

  const names = objectNames(row, language);
  const count = groupCount(row);
  let object = names.join(", ");

  if (!object) {
    object = unitFallback(row, count, translate);
  } else if (shouldAppendMore(row)) {
    object = `${object} ${translate("chat.step.more")}`;
  }

  return withEditStats(object, row);
}

const SEARCH_NOISE =
  /^(天气|实时|温度|降水预报|预报|weather|temperature|forecast|realtime|current)$/i;
const SEARCH_DATE =
  /^\d{4}[-年/]\d{1,2}[-月/]\d{1,2}日?$|^\d{1,2}月\d{1,2}日$/;

function formatSearchObject(row: ToolGroupRow): string {
  const keyword = compactSearchQuery(row.objects[0] || row.object);
  const count = row.pairs.length;
  if (!keyword) return "";
  return count > 1 ? `${keyword} ×${count}` : keyword;
}

/** Keep the place/time object; drop date stamps and synonym stuffing. */
export function compactSearchQuery(raw: string): string {
  const tokens = raw.split(/\s+/).filter(Boolean);
  const kept = tokens.filter(
    (token) => !SEARCH_DATE.test(token) && !SEARCH_NOISE.test(token),
  );
  const picked = (kept.length > 0 ? kept : tokens).slice(0, 2);
  const cjk = picked.every((token) => /[\u4e00-\u9fff]/.test(token));
  return picked.join(cjk ? "" : " ");
}

function objectNames(row: ToolGroupRow, language: Language): string[] {
  if (row.family === "skill") {
    return row.objects.map((name) => skillDisplayName(name, language));
  }
  return row.objects.slice();
}

function groupCount(row: ToolGroupRow): number {
  return row.family === "read" || row.family === "edit"
    ? row.uniqueFiles
    : row.pairs.length;
}

function unitFallback(
  row: ToolGroupRow,
  count: number,
  translate: Translate,
): string {
  if (row.family === "shell") {
    return translate("chat.step.cmds", { count });
  }
  if (row.family === "read" || row.family === "edit") {
    return translate("chat.step.files", { count });
  }
  if (row.family === "other") {
    return humanToolName(row.name, translate);
  }
  return "";
}

function shouldAppendMore(row: ToolGroupRow): boolean {
  if (row.objectVaried) return true;
  if (row.family === "read" || row.family === "edit") {
    return row.uniqueFiles > row.objects.length;
  }
  if (row.family === "shell") {
    return row.pairs.length > 1 && row.objects.length > 0;
  }
  return false;
}

function withEditStats(object: string, row: ToolGroupRow): string {
  if (row.family !== "edit") return object;
  const stats: string[] = [];
  if (row.additions > 0) stats.push(`+${row.additions}`);
  if (row.deletions > 0) stats.push(`−${row.deletions}`);
  if (stats.length === 0) return object;
  return object ? `${object} ${stats.join(" ")}` : stats.join(" ");
}
