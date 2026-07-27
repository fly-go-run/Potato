import { AlertTriangle, Check, ChevronRight, CopyCheck, X } from "lucide-react";
import { useState } from "react";
import {
  approvalParameterSummary,
  type PendingApproval,
} from "../../lib/approvals";
import { useTranslation } from "../../lib/i18n";
import { useChatStore } from "../../stores/chat";
import { JsonView } from "./Markdown";

export function ApprovalCard({ approval }: { approval: PendingApproval }) {
  const { t } = useTranslation();
  const actOnApproval = useChatStore((state) => state.actOnApproval);
  const [processing, setProcessing] = useState<string | null>(null);
  const summary = approvalParameterSummary(approval.tool_params);

  const act = async (
    action: "approve" | "deny",
    scope: "exact" | "similar" = "exact",
  ) => {
    const key = `${action}:${scope}`;
    setProcessing(key);
    try {
      await actOnApproval(approval.request_id, action, scope);
    } finally {
      setProcessing(null);
    }
  };

  return (
    <section className="my-4 overflow-hidden rounded-lg border border-line bg-surface shadow-sm">
      <div className="flex items-start gap-3 border-b border-line bg-bubble-tool px-4 py-3">
        <div className="mt-0.5 rounded-md bg-accent-soft p-1.5 text-accent">
          <AlertTriangle size={16} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-medium text-ink-secondary">
              {t("approval.title")}
            </span>
            <SeverityBadge severity={approval.severity} />
          </div>
          <div className="mt-1 truncate text-sm font-medium text-ink">
            {approval.tool_display_name || approval.tool_name}
          </div>
          {summary && (
            <div className="mt-0.5 truncate font-mono text-xs text-ink-muted">
              {summary}
            </div>
          )}
        </div>
      </div>

      <div className="space-y-3 px-4 py-3">
        {(approval.findings_summary || approval.findings_count > 0) && (
          <div className="rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
            {approval.findings_summary ||
              t("approval.summary", { count: approval.findings_count })}
          </div>
        )}

        {(approval.exact_target || approval.tool_source) && (
          <div className="grid gap-2 text-xs sm:grid-cols-2">
            {approval.exact_target && (
              <div>
                <div className="text-ink-muted">{t("approval.target")}</div>
                <div className="mt-0.5 break-all font-mono text-ink-secondary">
                  {approval.exact_target}
                </div>
              </div>
            )}
            {approval.tool_source && (
              <div>
                <div className="text-ink-muted">{t("approval.source")}</div>
                <div className="mt-0.5 text-ink-secondary">
                  {approval.tool_source}
                </div>
              </div>
            )}
          </div>
        )}

        <details className="group rounded-md bg-bubble-tool">
          <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs text-ink-secondary">
            <ChevronRight
              size={13}
              className="transition-transform group-open:rotate-90"
            />
            {t("approval.parameters")}
          </summary>
          <div className="border-t border-line px-3 py-2">
            <JsonView value={approval.tool_params} />
          </div>
        </details>

        <div className="flex flex-wrap items-center gap-2 pt-1">
          <button
            type="button"
            disabled={processing !== null}
            onClick={() => void act("approve")}
            className="inline-flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-surface transition-colors hover:bg-accent-hover disabled:opacity-40"
          >
            <Check size={14} />
            {processing === "approve:exact"
              ? t("approval.processing")
              : t("approval.approve")}
          </button>
          {approval.is_generalized && (
            <button
              type="button"
              disabled={processing !== null}
              onClick={() => void act("approve", "similar")}
              className="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 text-xs font-medium text-ink-secondary transition-colors hover:border-line-strong hover:text-ink disabled:opacity-40"
              title={approval.similar_target || undefined}
            >
              <CopyCheck size={14} />
              {processing === "approve:similar"
                ? t("approval.processing")
                : t("approval.approveSimilar")}
            </button>
          )}
          <button
            type="button"
            disabled={processing !== null}
            onClick={() => void act("deny")}
            className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium text-danger transition-colors hover:bg-danger-soft disabled:opacity-40"
          >
            <X size={14} />
            {processing === "deny:exact"
              ? t("approval.processing")
              : t("approval.deny")}
          </button>
        </div>
      </div>
    </section>
  );
}

function SeverityBadge({ severity }: { severity: string }) {
  const { t } = useTranslation();
  const normalized = severity.toLowerCase();
  const key =
    normalized === "low" ||
    normalized === "medium" ||
    normalized === "high" ||
    normalized === "critical"
      ? (`approval.severity.${normalized}` as const)
      : "approval.severity.unknown";
  const classes =
    normalized === "low"
      ? "bg-accent-soft text-accent"
      : normalized === "medium"
        ? "bg-bubble-tool text-warn"
        : "bg-danger-soft text-danger";
  return (
    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${classes}`}>
      {t(key)}
    </span>
  );
}
