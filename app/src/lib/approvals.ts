export type ApprovalSeverity = "low" | "medium" | "high" | "critical" | string;

export interface PendingApproval {
  request_id: string;
  session_id: string;
  root_session_id: string;
  tool_name: string;
  tool_params: Record<string, unknown>;
  severity: ApprovalSeverity;
  findings_count: number;
  findings_summary: string | null;
  source_type: string;
  driver: string | null;
  created_at: string;
  timeout_seconds: number;
  tool_display_name: string;
  tool_source: string;
  exact_target: string;
  similar_target: string;
  is_generalized: boolean;
}

export interface PushMessagesResponse {
  messages: unknown[];
  pending_approvals: PendingApproval[];
}

export function filterApprovalsForSession(
  approvals: PendingApproval[],
  sessionId: string,
): PendingApproval[] {
  if (!sessionId) return [];
  return approvals.filter((approval) => approval.root_session_id === sessionId);
}

export function approvalParameterSummary(
  parameters: Record<string, unknown>,
): string {
  return Object.entries(parameters)
    .slice(0, 3)
    .map(([key, value]) => `${key}: ${compactValue(value)}`)
    .join(" · ");
}

function compactValue(value: unknown): string {
  const text =
    typeof value === "string" ? value : JSON.stringify(value) ?? String(value);
  return text.length > 72 ? `${text.slice(0, 69)}…` : text;
}
