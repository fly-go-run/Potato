import { AlertTriangle, Check, ChevronRight, CopyCheck, X } from "lucide-react";
import { useState } from "react";
import {
  approvalParameterSummary,
  type PendingApproval,
} from "../../lib/approvals";
import { useTranslation } from "../../lib/i18n";
import { useChatStore } from "../../stores/chat";
import { Badge, Button } from "../ui";
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
    <section className="my-4 overflow-hidden rounded-lg border border-line bg-surface shadow-[var(--shadow-sm)]">
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
          <Button
            variant="primary"
            size="sm"
            disabled={processing !== null}
            onClick={() => void act("approve")}
          >
            <Check size={14} />
            {processing === "approve:exact"
              ? t("approval.processing")
              : t("approval.approve")}
          </Button>
          {approval.is_generalized && (
            <Button
              variant="secondary"
              size="sm"
              disabled={processing !== null}
              onClick={() => void act("approve", "similar")}
              title={approval.similar_target || undefined}
            >
              <CopyCheck size={14} />
              {processing === "approve:similar"
                ? t("approval.processing")
                : t("approval.approveSimilar")}
            </Button>
          )}
          <Button
            variant="danger"
            size="sm"
            disabled={processing !== null}
            onClick={() => void act("deny")}
          >
            <X size={14} />
            {processing === "deny:exact"
              ? t("approval.processing")
              : t("approval.deny")}
          </Button>
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
  const tone =
    normalized === "low"
      ? "neutral"
      : normalized === "medium"
        ? "warn"
        : "danger";
  return <Badge tone={tone}>{t(key)}</Badge>;
}
