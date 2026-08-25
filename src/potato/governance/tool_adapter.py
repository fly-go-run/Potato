# -*- coding: utf-8 -*-
# pylint: disable=protected-access
"""PolicyGuardedTool — Governance policy-checked tool wrapper.

Replaces the GuardedFunctionTool. Each tool call goes through two layers:
1. check_permissions: pre-execution decision
2. __call__: actual execution — applies sandbox_config; denials stay denied
"""

from __future__ import annotations

import logging
import uuid
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Any, Optional

from agentscope.message import TextBlock
from agentscope.tool import ToolChunk

from .policy import (
    GovernanceDecision,
    GovernanceAction,
    ToolCallSpec,
    _max_severity,
)
from .resource_governor import ResourceGovernor
from .tool_registry import DEFAULT_REGISTRY

logger = logging.getLogger(__name__)

_NO_RETRY_INSTRUCTION = (
    "\n\n\u26a0\ufe0f **System instruction**: this denial is final"
    " for the current request. Do not retry this tool with similar"
    " parameters. Reply to the user explaining why the action could"
    " not be completed and, if appropriate, ask them how they want"
    " to proceed."
)


@dataclass
class _PolicyInvocation:
    """Policy state belonging to one permission-check/execution pair.

    AgentScope runs ``check_permissions`` before it starts the actual tool
    call.  The latter may be moved into a child task by ToolCoordinator, so a
    ContextVar is the right hand-off mechanism: ``create_task`` copies the
    caller context while concurrent requests keep independent values.  The
    mutable ``consumed`` flag is shared by that copied context and prevents a
    completed invocation from being reused by a later pre-authorized call.
    """

    raw_params: dict[str, Any]
    tc_spec: ToolCallSpec
    policy_decision: GovernanceDecision | None = None
    sandbox_config: Any | None = None
    sandbox_mode: bool = False
    consumed: bool = False

    def consume_if_matches(self, kwargs: dict[str, Any]) -> bool:
        """Claim this state only for the matching tool invocation."""
        if self.consumed:
            return False
        # AgentScope injects this after permission checking, and the adapter
        # itself may add sandbox_config.  Neither belongs to the model's raw
        # tool input that policy evaluated.
        actual = {
            key: value
            for key, value in kwargs.items()
            if key not in {"_agent_state", "sandbox_config"}
        }
        if actual != self.raw_params:
            return False
        self.consumed = True
        return True


_current_policy_invocation: ContextVar[_PolicyInvocation | None] = ContextVar(
    "current_policy_invocation",
    default=None,
)


def _is_execution_level_off() -> bool:
    """Check if execution_level is 'off' (dev mode pass-through).

    Reads directly from policy.yaml (without needing a governor) to
    support the case where governor initialization failed but the user
    explicitly configured execution_level=off for development.
    """
    try:
        from pathlib import Path

        import yaml

        from ..constant import WORKING_DIR

        policy_paths = (
            Path(WORKING_DIR) / ".potato" / "policy.yaml",
            Path(WORKING_DIR) / ".qwenpaw" / "policy.yaml",
            Path(WORKING_DIR) / ".copaw" / "policy.yaml",
        )
        policy_path = next(
            (path for path in policy_paths if path.exists()),
            None,
        )
        if policy_path is None:
            return False
        with open(policy_path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        return isinstance(data, dict) and data.get("execution_level") == "off"
    except Exception:
        return False


def _resolve_effective_approval_level(
    request_context: dict[str, str] | None,
) -> Optional[Any]:
    """Resolve the effective approval_level for this tool call.

    Priority:
      1. ``request_context["approval_level"]`` — session-level override
         injected by the frontend (localStorage per chat, carried in each
         request). Zero I/O: already in memory.
      2. ``agent.json`` → ``AgentProfileConfig.approval_level`` — the
         agent-level default set via the Web UI 'Tool Execution Security' card.
      3. ``None`` — unresolvable (caller falls back to AUTO).

    Returns the :class:`ToolExecutionLevel` enum, or ``None``.
    """
    if not request_context:
        return None

    from ..security.tool_guard.execution_level import ToolExecutionLevel

    # Session-level override (injected by frontend per request)
    session_raw = request_context.get("approval_level")
    if session_raw:
        level = ToolExecutionLevel.from_config(session_raw)
        if level is not None:
            return level

    # Agent-level default from agent.json
    agent_id = request_context.get("agent_id", "")
    if not agent_id:
        return None
    try:
        from ..config.config import load_agent_config

        profile = load_agent_config(agent_id)
        raw = getattr(profile, "approval_level", None)
        return ToolExecutionLevel.from_config(raw)
    except Exception:
        return None


# ---------------------------------------------------------------------------
# PolicyGuardedTool
# ---------------------------------------------------------------------------


class PolicyGuardedTool:
    """Governance policy-checked tool wrapper.

    Dynamically inherits from FunctionTool, implementing:
    - check_permissions: calls assert_policy() + audit() for policy decision
    - __call__: overrides to apply sandbox_config; violations are not
      retried unsandboxed

    .. warning:: Known limitation — dynamic anonymous class

        ``__new__`` creates a fresh class via ``type(...)`` on every call,
        so ``isinstance(tool, PolicyGuardedTool)`` always returns False.
        This is the same pattern used by ``GuardedFunctionTool`` and
        ``DriverCapabilityTool``.  A unified refactor (metaclass or mixin)
        is tracked as a follow-up task.
    """

    def __new__(cls, *args: Any, **kwargs: Any) -> Any:
        from agentscope.tool import FunctionTool

        if cls is PolicyGuardedTool:
            real_cls = type(
                "PolicyGuardedTool",
                (FunctionTool,),
                {
                    "__init__": _policy_tool_init,
                    "_build_tc_spec": _build_tc_spec,
                    "check_permissions": _policy_tool_check_permissions,
                    "__call__": _policy_tool_call,
                    "__doc__": cls.__doc__,
                },
            )
            return real_cls(*args, **kwargs)
        return object.__new__(cls)


def _policy_tool_init(
    self: Any,
    func: Any,
    *,
    governor: Optional[ResourceGovernor] = None,
    request_context: dict[str, str] | None = None,
    **kwargs: Any,
) -> None:
    from agentscope.tool import FunctionTool

    FunctionTool.__init__(self, func, **kwargs)
    self._qp_governor = governor
    self._qp_request_context = request_context or {}


def _build_tc_spec(
    self: Any,
    params: dict[str, Any] | None = None,
) -> ToolCallSpec:
    """Build ToolCallSpec for one invocation's dynamic target."""
    governor = self._qp_governor
    raw_params = dict(params or {})
    tool_name = DEFAULT_REGISTRY.python_to_policy_name(
        getattr(self, "name", "Unknown"),
    )
    request_ctx = getattr(self, "_qp_request_context", {}) or {}
    return ToolCallSpec(
        tool_name=tool_name,
        target=DEFAULT_REGISTRY.extract_target(
            tool_name,
            raw_params,
            workspace_dir=str(governor.workspace_dir) if governor else "",
        ),
        agent_id=request_ctx.get("agent_id", ""),
        session_id=request_ctx.get("session_id", ""),
        raw_params=raw_params,
    )


def _prepare_off_mode_sandbox(
    tool: Any,
    governor: Any,
    invocation: _PolicyInvocation,
) -> None:
    """Compile+attach a ``sandbox_config`` for fail-closed tools in OFF mode.

    ``approval_level=OFF`` short-circuits the policy pipeline to ALLOW-all,
    which normally also skips the ``SANDBOX_FALLBACK`` branch that compiles a
    ``sandbox_config`` (see :func:`_policy_tool_check_permissions`). Sandbox
    provisioning and user approval are independent concerns: skipping "ask the
    user" must not skip "run it in a sandbox".

    Only tools flagged ``requires_sandbox`` in the registry are handled — i.e.
    the REPL, which returns ``DENIED`` without a config. Fail-open shell tools
    like ``Bash`` are deliberately left untouched: in OFF mode they run
    unsandboxed by design, and forcing a sandbox on them would silently narrow
    their filesystem access.

    A no-op (leaving ``sandbox_config=None``) when the sandbox is not usable
    -- either the platform has no sandbox, or the operator turned the global
    ``security.sandbox_enabled`` switch off. In both cases such a REPL is
    never registered in the first place, or it tolerates unsandboxed
    execution via ``allow_unsandboxed``.

    Uses ``governor.sandbox_usable`` (platform support AND the global switch)
    rather than the platform-only ``sandbox_available`` probe, so that an
    explicit ``sandbox_enabled=false`` is honoured on the OFF-mode path just
    like it is on the normal policy path (see
    :meth:`ResourceGovernor._sandbox_usable`).
    """
    if governor is None:
        return
    policy_name = DEFAULT_REGISTRY.python_to_policy_name(
        getattr(tool, "name", "Unknown"),
    )
    if not DEFAULT_REGISTRY.requires_sandbox(policy_name):
        return
    if not getattr(governor, "sandbox_usable", False):
        return
    try:
        invocation.sandbox_config = governor.compile_sandbox_config(
            invocation.tc_spec,
        )
        invocation.sandbox_mode = True
    except Exception:
        # Leave sandbox_config unset; the tool's own fail-closed guard still
        # protects us — better a clean denial than an unsandboxed run.
        logger.exception(
            "OFF-mode sandbox_config compilation failed for '%s'.",
            getattr(tool, "name", "Unknown"),
        )


# pylint: disable=too-many-return-statements
async def _policy_tool_check_permissions(
    self: Any,
    input_data: dict[str, Any] | None = None,
    context: Any = None,
    *_extra_args: Any,
    **_extra_kwargs: Any,
) -> Any:
    """Perform governance policy evaluation for a tool call.

    Flow:
        1. Construct ToolCallSpec(tool_name, target, agent_id, session_id)
        2. governor.assert_policy(tool_call) → GovernanceDecision
        3. governor.audit(tool_call, decision)
        4. Map to PermissionDecision
    """
    from agentscope.permission import PermissionBehavior, PermissionDecision

    del context

    governor = getattr(self, "_qp_governor", None)
    invocation = _PolicyInvocation(
        raw_params=dict(input_data or {}),
        tc_spec=self._build_tc_spec(input_data or {}),
    )
    _current_policy_invocation.set(invocation)

    # ── Effective approval_level check (session > agent) ──
    request_ctx = getattr(self, "_qp_request_context", None) or {}
    effective_level = _resolve_effective_approval_level(request_ctx)
    from .escalation import (
        DANGER_FULL_ACCESS,
        apply_standing_sandbox_mode,
        resolve_sandbox_mode,
    )

    sandbox_mode = resolve_sandbox_mode(request_ctx)
    if governor is not None:
        governor.session_sandbox_mode = sandbox_mode

    # Sync effective approval_level to the governor's policy
    # so the three-phase evaluation uses the correct threshold.
    if governor is not None and effective_level is not None:
        governor.policy.execution_level = effective_level.value

    if effective_level is not None and effective_level.is_disabled():
        # OFF means "never ask". The sandbox knob still decides the cage:
        # workspace-write runs policy (ASK becomes allow) inside the
        # sandbox; danger-full-access is the Codex Full Access preset.
        if sandbox_mode == DANGER_FULL_ACCESS or governor is None:
            _prepare_off_mode_sandbox(self, governor, invocation)
            return PermissionDecision(
                behavior=PermissionBehavior.ALLOW,
                message="governance: approval_level=off, all tools allowed.",
            )
        decision = governor.assert_policy(invocation.tc_spec)
        decision = apply_standing_sandbox_mode(
            decision,
            invocation.tc_spec,
            sandbox_mode,
        )
        governor.audit(invocation.tc_spec, decision)
        if decision.action is GovernanceAction.DENY:
            _current_policy_invocation.set(None)
            return PermissionDecision(
                behavior=PermissionBehavior.DENY,
                message=(
                    f"governance: '{invocation.tc_spec.tool_name}' "
                    "is denied by policy"
                ),
            )
        # OFF never mints a capability increment. A self-declared
        # danger-full-access / network / path request is stripped back
        # to the default cage (or denied when no cage exists).
        if decision.source == "escalation":
            if not governor._sandbox_usable():
                _current_policy_invocation.set(None)
                return PermissionDecision(
                    behavior=PermissionBehavior.DENY,
                    message=(
                        "governance: approval_level=off cannot grant "
                        "sandbox_permissions without a sandbox"
                    ),
                )
            decision.sandbox_config = governor.compile_sandbox_config(
                invocation.tc_spec,
            )
        if decision.source == "read_only":
            _current_policy_invocation.set(None)
            return PermissionDecision(
                behavior=PermissionBehavior.DENY,
                message=(
                    "governance: read-only sandbox blocks writes when "
                    "approval_level=off"
                ),
            )
        invocation.policy_decision = decision
        invocation.sandbox_config = decision.sandbox_config
        invocation.sandbox_mode = decision.sandbox_config is not None
        if invocation.sandbox_config is None:
            _prepare_off_mode_sandbox(self, governor, invocation)
        return PermissionDecision(
            behavior=PermissionBehavior.ALLOW,
            message="governance: approval_level=off, ask skipped.",
        )

    if governor is None:
        _current_policy_invocation.set(None)
        # Check if execution_level is "off" (dev mode) — allow pass-through
        if _is_execution_level_off():
            return PermissionDecision(
                behavior=PermissionBehavior.ALLOW,
                message="governance: execution_level=off (dev mode), "
                "governor unavailable — pass-through.",
            )
        # Fail-closed: if governance layer failed to initialize, deny all
        # tool calls rather than silently allowing unguarded execution.
        logger.error(
            "PolicyGuardedTool: governor is None for tool '%s' — "
            "governance layer not initialized; denying (fail-closed).",
            getattr(self, "name", "Unknown"),
        )
        return PermissionDecision(
            behavior=PermissionBehavior.DENY,
            message=(
                "Governance layer unavailable (governor not initialized). "
                "All tool calls are denied until governance is restored. "
                "Please check server logs for initialization errors."
            ),
        )

    tc_spec = invocation.tc_spec

    decision = governor.assert_policy(tc_spec)
    decision = apply_standing_sandbox_mode(decision, tc_spec, sandbox_mode)
    governor.audit(tc_spec, decision)

    invocation.policy_decision = decision
    invocation.sandbox_config = decision.sandbox_config
    invocation.sandbox_mode = decision.sandbox_config is not None

    if decision.action in (
        GovernanceAction.ALLOW,
        GovernanceAction.ALLOW_UNSANDBOXED,
    ):
        return PermissionDecision(
            behavior=PermissionBehavior.ALLOW,
            message=(
                "governance: tool allowed."
                if decision.action is GovernanceAction.ALLOW
                else "governance: tool allowed with explicit host grant."
            ),
        )
    elif decision.action is GovernanceAction.DENY:
        _current_policy_invocation.set(None)
        return PermissionDecision(
            behavior=PermissionBehavior.DENY,
            message=f"governance: '{tc_spec.tool_name}' is denied by policy",
        )
    elif decision.action is GovernanceAction.SANDBOX_FALLBACK:
        # Bash tool with no rule match → allow execution in sandbox
        return PermissionDecision(
            behavior=PermissionBehavior.ALLOW,
            message="governance: sandbox fallback.",
        )
    elif decision.action is GovernanceAction.ASK:
        # Requires user confirmation
        approval = await _ask_user_approval(
            governor=governor,
            tc_spec=tc_spec,
            request_context=getattr(self, "_qp_request_context", {}) or {},
            policy_findings=decision.findings,
            governance_reason=decision.reason,
            source=decision.source,
        )
        if approval.behavior != PermissionBehavior.ALLOW:
            _current_policy_invocation.set(None)
        else:
            from .escalation import (
                DANGER_FULL_ACCESS,
                EscalationLevel,
                has_session_grant,
                parse_escalation_request,
            )

            requested, _err = parse_escalation_request(
                invocation.tc_spec.raw_params,
            )
            if requested is EscalationLevel.DANGER_FULL_ACCESS and (
                decision.source == "escalation"
                or has_session_grant(
                    session_id=tc_spec.session_id,
                    tool_name=tc_spec.tool_name,
                    command=tc_spec.target,
                    permission=DANGER_FULL_ACCESS,
                )
            ):
                invocation.sandbox_config = None
                invocation.sandbox_mode = False
        return approval
    else:
        # Unknown decision → deny as safe default
        _current_policy_invocation.set(None)
        return PermissionDecision(
            behavior=PermissionBehavior.DENY,
            message=f"Unknown policy decision: {decision.action}",
        )


async def _policy_tool_call(
    self: Any,
    *args: Any,
    **kwargs: Any,
) -> Any:
    """Override FunctionTool.__call__ to attach sandbox_config.

    A sandbox denial is a containment result, not an implicit escalation.
    The command is not retried on the host; unsandboxed execution requires
    an explicit pre-declared grant on a later call.
    """
    invocation = _current_policy_invocation.get()
    if invocation is not None and not invocation.consume_if_matches(kwargs):
        invocation = None
    if invocation is not None and invocation.sandbox_mode:
        if invocation.sandbox_config is not None:
            kwargs["sandbox_config"] = invocation.sandbox_config

    # Call the original function
    from agentscope.tool import FunctionTool
    from agentscope.message import ToolResultState

    result = await FunctionTool.__call__(self, *args, **kwargs)

    # Check if sandbox violation was returned (state=DENIED)
    if not (
        isinstance(result, ToolChunk)
        and result.state == ToolResultState.DENIED
    ):
        return result

    # Extract violation message from metadata or content
    violation_msg = ""
    if hasattr(result, "metadata") and result.metadata:
        violation_msg = result.metadata.get("sandbox_violation", "")
    if not violation_msg:
        # Legacy fallback for results created before structured metadata.
        for block in result.content or []:
            if hasattr(block, "text") and "Sandbox violation:" in block.text:
                violation_msg = (
                    block.text.split("Sandbox violation:", 1)[1]
                    .split("\n")[0]
                    .strip()
                )
                break

    logger.info(
        "PolicyGuardedTool: sandbox violation for '%s': %s "
        "(not retrying unsandboxed)",
        getattr(self, "name", "Unknown"),
        violation_msg,
    )

    governor = getattr(self, "_qp_governor", None)
    tc_spec = invocation.tc_spec if invocation is not None else None
    if tc_spec is None:
        tc_spec = self._build_tc_spec({})
    if governor is not None:
        governor.audit(
            tc_spec,
            GovernanceDecision(
                action=GovernanceAction.DENY,
                reason=(
                    f"sandbox violation: {violation_msg}"
                    if violation_msg
                    else "sandbox violation"
                ),
                source="sandbox",
            ),
        )

    return ToolChunk(
        is_last=True,
        state=ToolResultState.DENIED,
        content=[
            TextBlock(
                type="text",
                text=(
                    f"Sandbox violation: {violation_msg}\n"
                    "The command was not retried outside the sandbox. "
                    "Host execution requires an explicit pre-declared "
                    "grant, not a post-failure retry."
                    f"{_NO_RETRY_INSTRUCTION}"
                ),
            ),
        ],
    )


# ---------------------------------------------------------------------------
# ASK path: reuse ApprovalService
# ---------------------------------------------------------------------------


async def _ask_user_approval(  # pylint: disable=too-many-statements
    governor: ResourceGovernor,
    tc_spec: ToolCallSpec,
    request_context: dict[str, str],
    *,
    violation_msg: str | None = None,
    governance_reason: str | None = None,
    policy_findings: list[Any] | None = None,
    source: str = "No rule hit",
) -> Any:
    """Request user approval, blocking until a reply is received."""
    from agentscope.permission import PermissionBehavior, PermissionDecision

    from ..app.approvals import get_approval_service
    from ..constant import TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS
    from .escalation import describe_permission_increment
    from ..security.tool_guard.approval import (
        ApprovalDecision,
        ApprovalScope,
        format_findings_summary,
    )
    from ..security.tool_guard.models import (
        GuardFinding,
        GuardSeverity,
        GuardThreatCategory,
        ToolGuardResult,
    )

    tool_name = tc_spec.tool_name
    target = tc_spec.target
    agent_id = tc_spec.agent_id
    session_id = tc_spec.session_id
    params = tc_spec.raw_params

    ctx = request_context or {}
    user_id = str(ctx.get("user_id") or "")
    channel = str(ctx.get("channel") or "")
    root_session_id = str(ctx.get("root_session_id") or session_id)
    root_agent_id = str(ctx.get("root_agent_id") or agent_id or "unknown")

    # AUTO is an unattended, model-reviewed approval mode. Prefer a
    # provider-advertised command-review companion model, but fall back to the
    # active chat model when no such model is listed. Review failures are
    # fail-closed and return immediately instead of leaving the task parked on
    # a human approval waiter. SMART / STRICT retain the existing human flow.
    # HIGH findings are not eligible for automatic review — only a human
    # one-shot grant can open that gate.
    effective_level = _resolve_effective_approval_level(ctx)
    findings_are_high = _max_severity(policy_findings or []) in {
        "HIGH",
        "CRITICAL",
    }
    if (
        effective_level is not None
        and effective_level.value == "auto"
        and not findings_are_high
        and source not in {
            "escalation",
            "write_boundary",
            "read_only",
        }
    ):
        from .auto_review import review_tool_call

        review = await review_tool_call(
            tool_name=tool_name,
            target=target,
            params=dict(params or {}),
            agent_id=agent_id,
            governance_reason=governance_reason,
            policy_findings=policy_findings,
            violation_msg=violation_msg,
            review_context=(
                ctx.get("review_context")
                or ctx.get("user_intent")
                or ctx.get("last_user_message")
                or ""
            ),
            request_metadata={
                "source": ctx.get("source", ""),
                "channel": ctx.get("channel", ""),
                "approval_level": effective_level.value,
            },
        )
        logger.info(
            "PolicyGuardedTool: AUTO review tool=%s model=%s dedicated=%s "
            "approved=%s human=%s risk=%s authorization=%s",
            tool_name,
            review.model_id or "unavailable",
            review.used_dedicated_model,
            review.approved,
            review.require_human,
            review.risk_level,
            review.user_authorization,
        )
        if review.require_human:
            logger.info(
                "PolicyGuardedTool: AUTO review deferred to human "
                "tool=%s reason=%s",
                tool_name,
                review.reason,
            )
        elif review.approved:
            governor.audit(
                tc_spec,
                GovernanceDecision(
                    action=GovernanceAction.ALLOW,
                    reason="Auto Review Approve",
                ),
            )
            return PermissionDecision(
                behavior=PermissionBehavior.ALLOW,
                message=(
                    "Approved by automatic command review"
                    + (f" ({review.model_id})." if review.model_id else ".")
                ),
            )
        else:
            governor.audit(
                tc_spec,
                GovernanceDecision(
                    action=GovernanceAction.DENY,
                    reason=f"Auto Review Deny: {review.reason}",
                ),
            )
            return PermissionDecision(
                behavior=PermissionBehavior.DENY,
                message=(
                    f"Automatic command review denied '{tool_name}': "
                    f"{review.reason}." + _NO_RETRY_INSTRUCTION
                ),
            )

    from .generalize import generalize_target_for_approval

    generalized_target = await generalize_target_for_approval(
        tool_name,
        target,
        source,
        agent_id=agent_id,
    )
    display_target = generalized_target or target

    # Construct a ToolGuardResult for ApprovalService.
    # If deep-scan findings were attached by policy.evaluate(),
    # convert them into GuardFindings for the approval card.
    if policy_findings:
        converted_findings = []
        for pf in policy_findings:
            # pf is a governance.detectors.GuardFinding (dataclass)
            converted_findings.append(
                GuardFinding(
                    id=getattr(pf, "id", uuid.uuid4().hex[:8]),
                    rule_id=getattr(pf, "rule_id", "policy_deep_scan"),
                    category=GuardThreatCategory(
                        getattr(pf, "category", "resource_abuse"),
                    ),
                    severity=GuardSeverity(
                        getattr(pf, "severity", "INFO"),
                    ),
                    title=getattr(pf, "title", "Policy Approval Required"),
                    description=getattr(pf, "description", ""),
                    tool_name=tool_name,
                    param_name=getattr(pf, "param_name", None),
                    matched_value=getattr(pf, "matched_value", None),
                    matched_pattern=getattr(pf, "matched_pattern", None),
                    snippet=getattr(pf, "snippet", None),
                    remediation=getattr(pf, "remediation", None),
                    guardian=getattr(pf, "detector", "governance_policy"),
                    metadata=getattr(pf, "metadata", {}),
                ),
            )
        guard_result = ToolGuardResult(
            tool_name=tool_name,
            params=params,
            findings=converted_findings,
            guardians_used=["governance_policy"],
        )
    else:
        guard_result = ToolGuardResult(
            tool_name=tool_name,
            params=params,
            findings=[
                GuardFinding(
                    id=uuid.uuid4().hex[:8],
                    rule_id="policy_ask",
                    category=GuardThreatCategory.RESOURCE_ABUSE,
                    severity=(
                        GuardSeverity.HIGH
                        if violation_msg
                        else GuardSeverity.INFO
                    ),
                    title=(
                        "Sandbox Violation — Approve Unsandboxed Execution?"
                        if violation_msg
                        else "Policy Approval Required"
                    ),
                    description=(
                        (
                            f"Governance reason: {governance_reason}"
                            if governance_reason
                            else ""
                        )
                        + (
                            f"\n\n\u26a0\ufe0f Sandbox violation: "
                            f"{violation_msg}"
                            "\n\nThis call is not retried on the host. "
                            "Approve only records the grant for a later "
                            "explicit request."
                            if violation_msg
                            else ""
                        )
                    ),
                    tool_name=tool_name,
                    remediation=(
                        "Approve or deny this tool call. A sandbox "
                        "denial is not re-run unsandboxed."
                        if violation_msg
                        else "Approve or deny this tool call"
                    ),
                    guardian="governance_policy",
                    metadata={
                        "target": target,
                        **(
                            {
                                "sandbox_violation": violation_msg,
                                "escalation": "sandbox_to_host",
                            }
                            if violation_msg
                            else {}
                        ),
                    },
                ),
            ],
            guardians_used=["governance_policy"],
        )

    svc = get_approval_service()
    tool_call_id = str(ctx.get("tool_call_id") or "")
    if session_id and tool_call_id:
        await svc.cancel_stale_pending_for_tool_call(session_id, tool_call_id)

    pending = await svc.create_pending(
        session_id=session_id,
        root_session_id=root_session_id,
        owner_agent_id=root_agent_id,
        user_id=user_id,
        channel=channel,
        agent_id=agent_id or "unknown",
        tool_name=tool_name,
        result=guard_result,
        timeout_seconds=TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS,
        extra={
            "tool_call": {
                "id": tool_call_id,
                "name": tool_name,
                "input": dict(params or {}),
            },
            "display": {
                "tool_name": tool_name,
                "tool_source": source,
                "exact_target": target,
                "similar_target": display_target,
                "is_generalized": display_target != target,
                "permission_increment": describe_permission_increment(
                    source=source,
                    raw_params=params,
                ),
            },
            "channel_meta": ctx.get("channel_meta"),
            "_channel_instance": ctx.get("_channel_instance"),
            **(
                {"_spawn_subagent": True} if ctx.get("_spawn_subagent") else {}
            ),
        },
    )

    logger.info(
        "PolicyGuardedTool: awaiting approval for tool=%s session=%s "
        "request_id=%s target=%s",
        tool_name,
        session_id[:8] if session_id else "",
        pending.request_id[:8],
        target,
    )

    try:
        decision = await svc.wait_for_approval(
            pending.request_id,
            TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS,
        )
    except Exception as exc:
        logger.error(
            "PolicyGuardedTool: wait_for_approval crashed (%s); denying",
            exc,
            exc_info=True,
        )
        decision = ApprovalDecision.DENIED

    # Record user approve/deny result to audit log
    approved = decision == ApprovalDecision.APPROVED
    # The scope the user chose (set by resolve_request on the same pending
    # object). None = no choice offered → EXACT.
    scope = getattr(pending, "scope", None)
    scope_label = scope.value if scope else "exact"
    approval_decision = GovernanceDecision(
        action=GovernanceAction.ALLOW if approved else GovernanceAction.DENY,
        reason=(f"User Approve ({scope_label})" if approved else "User Deny"),
    )
    governor.audit(tc_spec, approval_decision)

    summary = format_findings_summary(guard_result)
    if decision == ApprovalDecision.APPROVED:
        # ── Record approved rule (skip for builtin ask) ──
        # SIMILAR → the generalized pattern; EXACT (default) → the literal
        # target the user actually approved. Widening is opt-in.
        rule_target = (
            generalized_target if scope == ApprovalScope.SIMILAR else target
        )
        # Human grants persist across chats (Codex default.rules analog).
        # AUTO review never reaches this branch. Host execution is a
        # session-scoped increment, not a permanent unsandbox rule.
        await governor.add_approved_rule(
            tc_spec,
            generalized_target=rule_target,
            duration="permanent",
        )
        if source == "escalation":
            from .escalation import (
                DANGER_FULL_ACCESS,
                EscalationLevel,
                NETWORK,
                extra_writable_path,
                parse_escalation_request,
                path_permission_key,
                remember_session_grant,
            )
            from .write_boundary import validate_extra_writable

            requested, _err = parse_escalation_request(params)
            permission = DANGER_FULL_ACCESS
            if requested is EscalationLevel.NETWORK:
                permission = NETWORK
            elif requested is EscalationLevel.PATH:
                extra, path_err = validate_extra_writable(
                    extra_writable_path(params),
                    workspace_dir=governor.workspace_dir,
                    coding_project_dir=governor.coding_project_dir,
                    policy_dir=governor._policy_dir,
                )
                if extra and not path_err:
                    permission = path_permission_key(extra)
            remember_session_grant(
                session_id=session_id,
                tool_name=tool_name,
                pattern=rule_target,
                permission=permission,
                glob=scope == ApprovalScope.SIMILAR,
            )
        return PermissionDecision(
            behavior=PermissionBehavior.ALLOW,
            message=f"Approved by user ({scope_label}).\n{summary}",
        )

    denial_msg = (
        f"User denied the request to run '{tool_name}'.\n{summary}"
        if decision == ApprovalDecision.DENIED
        else (
            f"Approval for '{tool_name}' timed out after "
            f"{int(TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS)}s.\n{summary}"
        )
    )
    return PermissionDecision(
        behavior=PermissionBehavior.DENY,
        message=denial_msg + _NO_RETRY_INSTRUCTION,
    )
