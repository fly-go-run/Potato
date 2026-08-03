import { describe, expect, it } from "vitest";
import { filterApprovalsForSession, type PendingApproval } from "./approvals";

function approval(requestId: string, rootSessionId: string): PendingApproval {
  return {
    request_id: requestId,
    session_id: `${rootSessionId}-child`,
    root_session_id: rootSessionId,
    tool_name: "execute_shell_command",
    tool_params: { command: "pwd" },
    severity: "high",
    findings_count: 1,
    findings_summary: "shell command",
    source_type: "tool_guard",
    driver: "shell",
    created_at: "2026-07-27T00:00:00Z",
    timeout_seconds: 300,
    tool_display_name: "Shell",
    tool_source: "shell guardian",
    exact_target: "pwd",
    similar_target: "*",
    is_generalized: true,
  };
}

describe("filterApprovalsForSession", () => {
  it("uses root_session_id and excludes approvals from other roots", () => {
    const result = filterApprovalsForSession(
      [approval("same-root", "root-1"), approval("other-root", "root-2")],
      "root-1",
    );

    expect(result.map((item) => item.request_id)).toEqual(["same-root"]);
  });

  it("returns no approvals without a current session", () => {
    expect(filterApprovalsForSession([approval("a", "root-1")], "")).toEqual(
      [],
    );
  });
});
