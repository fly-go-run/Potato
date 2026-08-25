# -*- coding: utf-8 -*-
"""Explicit permission increments for tool calls.

Codex / official DSH require the model to declare a wider sandbox *before*
execution. Potato uses the same two-step ladder:

* ``workspace-write`` — default cage, no network
* ``network`` — same cage, outbound network opened
* ``danger-full-access`` — host execution; one-shot human grant, then
  remembered for this session only

Automatic review cannot mint this grant. A missing cage is still
``SANDBOX_UNAVAILABLE``, not an implicit host run.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from fnmatch import fnmatch
from threading import Lock
from typing import Any, Iterable, Mapping

WORKSPACE_WRITE = "workspace-write"
READ_ONLY = "read-only"
NETWORK = "network"
PATH = "path"
DANGER_FULL_ACCESS = "danger-full-access"
_ALLOWED_LEVELS = frozenset(
    {WORKSPACE_WRITE, NETWORK, PATH, DANGER_FULL_ACCESS},
)
_STANDING_SANDBOX_MODES = frozenset(
    {WORKSPACE_WRITE, READ_ONLY, DANGER_FULL_ACCESS},
)


def resolve_sandbox_mode(request_context: Mapping[str, Any] | None) -> str:
    """Session sandbox knob.

    ``request_context["sandbox_mode"]`` wins. Cron / IM channels that
    do not send the knob fall back to the agent profile, then
    workspace-write.
    """
    raw = str(
        (request_context or {}).get("sandbox_mode") or "",
    ).strip().lower()
    if raw in _STANDING_SANDBOX_MODES:
        return raw
    agent_id = str(
        (request_context or {}).get("agent_id")
        or (request_context or {}).get("root_agent_id")
        or "",
    ).strip()
    if agent_id:
        try:
            from ..config.config import load_agent_config

            profile = load_agent_config(agent_id)
            stored = str(
                getattr(profile, "sandbox_mode", "") or "",
            ).strip().lower()
            if stored in _STANDING_SANDBOX_MODES:
                return stored
        except Exception:
            pass
    return WORKSPACE_WRITE


def apply_standing_sandbox_mode(
    decision: Any,
    tc_spec: Any,
    sandbox_mode: str,
) -> Any:
    """Apply a user-selected standing cage.

    ``danger-full-access`` is an explicit host grant for routine shell
    ALLOW / sandbox-fallback. DENY and policy ASK stay in force.
    """
    from .policy import GovernanceAction, GovernanceDecision
    from .tool_registry import DEFAULT_REGISTRY
    from .write_boundary import is_file_write_tool

    if sandbox_mode == READ_ONLY:
        if (
            is_file_write_tool(tc_spec.tool_name)
            and decision.action is GovernanceAction.ALLOW
        ):
            return GovernanceDecision(
                action=GovernanceAction.ASK,
                reason="read-only sandbox: writes require approval",
                findings=decision.findings,
                source="read_only",
            )
        return decision
    if sandbox_mode != DANGER_FULL_ACCESS:
        return decision
    if DEFAULT_REGISTRY.get_type(tc_spec.tool_name) != "shell":
        return decision
    if decision.action not in (
        GovernanceAction.ALLOW,
        GovernanceAction.SANDBOX_FALLBACK,
    ):
        return decision
    return GovernanceDecision(
        action=GovernanceAction.ALLOW_UNSANDBOXED,
        reason="standing sandbox_mode=danger-full-access",
        findings=decision.findings,
        source=decision.source,
    )


class EscalationLevel(str, Enum):
    WORKSPACE_WRITE = WORKSPACE_WRITE
    NETWORK = NETWORK
    PATH = PATH
    DANGER_FULL_ACCESS = DANGER_FULL_ACCESS


@dataclass(frozen=True)
class EscalationGrant:
    session_id: str
    tool_name: str
    pattern: str
    permission: str
    glob: bool = False


_LOCK = Lock()
_GRANTS: list[EscalationGrant] = []


def parse_escalation_request(
    raw_params: Mapping[str, Any] | None,
) -> tuple[EscalationLevel | None, str | None]:
    """Return ``(level, error)``.

    ``error`` is set when the request is malformed. A missing or default
    ``workspace-write`` request is ``(None, None)``.
    """
    params = raw_params or {}
    raw_level = params.get("sandbox_permissions")
    justification = params.get("justification")
    level_text = str(raw_level or "").strip().lower()
    just_text = str(justification or "").strip()

    if not level_text:
        if just_text:
            return None, (
                "justification is only valid together with "
                "sandbox_permissions"
            )
        return None, None

    if level_text not in _ALLOWED_LEVELS:
        return None, (
            f'invalid sandbox_permissions "{raw_level}"; expected '
            f"{WORKSPACE_WRITE}, {NETWORK}, {PATH}, or "
            f"{DANGER_FULL_ACCESS}"
        )

    if level_text == WORKSPACE_WRITE:
        return EscalationLevel.WORKSPACE_WRITE, None

    if not just_text:
        return None, (
            f"sandbox_permissions={level_text} requires a "
            "non-empty justification"
        )
    if level_text == PATH:
        extra = str(params.get("additional_writable_path") or "").strip()
        if not extra:
            return None, (
                "sandbox_permissions=path requires "
                "additional_writable_path"
            )
        return EscalationLevel.PATH, None
    if level_text == NETWORK:
        return EscalationLevel.NETWORK, None
    return EscalationLevel.DANGER_FULL_ACCESS, None


def remember_session_grant(
    *,
    session_id: str,
    tool_name: str,
    pattern: str,
    permission: str = DANGER_FULL_ACCESS,
    glob: bool = False,
) -> None:
    """Remember one explicit host grant for the rest of this session."""
    if not session_id or not tool_name or not pattern:
        return
    grant = EscalationGrant(
        session_id=session_id,
        tool_name=tool_name,
        pattern=pattern,
        permission=permission,
        glob=glob,
    )
    with _LOCK:
        if grant not in _GRANTS:
            _GRANTS.append(grant)


def has_session_grant(
    *,
    session_id: str,
    tool_name: str,
    command: str,
    permission: str = DANGER_FULL_ACCESS,
) -> bool:
    """True when this session already granted *permission* for *command*."""
    if not session_id or not command:
        return False
    accepted = {permission}
    if permission == NETWORK:
        accepted.add(DANGER_FULL_ACCESS)
    with _LOCK:
        grants = list(_GRANTS)
    return any(
        grant.session_id == session_id
        and grant.tool_name == tool_name
        and grant.permission in accepted
        and _command_matches(command, grant.pattern, glob=grant.glob)
        for grant in grants
    )


def clear_session_grants(session_id: str | None = None) -> None:
    """Drop grants for tests or a finished session."""
    global _GRANTS
    with _LOCK:
        if session_id is None:
            _GRANTS = []
            return
        _GRANTS = [g for g in _GRANTS if g.session_id != session_id]


def _command_matches(
    command: str,
    pattern: str,
    *,
    glob: bool = False,
) -> bool:
    if command == pattern:
        return True
    if not glob:
        return False
    return fnmatch(command, pattern)


def iter_session_grants(session_id: str) -> Iterable[EscalationGrant]:
    with _LOCK:
        return tuple(g for g in _GRANTS if g.session_id == session_id)


def extra_writable_path(raw_params: Mapping[str, Any] | None) -> str:
    return str(
        (raw_params or {}).get("additional_writable_path") or "",
    ).strip()


def path_permission_key(path: str) -> str:
    return f"{PATH}:{path}"


def describe_permission_increment(
    *,
    source: str,
    raw_params: Mapping[str, Any] | None,
) -> str:
    """Human-readable summary of the extra capability this approval adds."""
    requested, _err = parse_escalation_request(raw_params)
    if requested is EscalationLevel.NETWORK:
        return "Open outbound network inside the sandbox"
    if requested is EscalationLevel.PATH:
        extra = extra_writable_path(raw_params)
        return f"Add writable directory {extra}" if extra else (
            "Add one extra writable directory"
        )
    if requested is EscalationLevel.DANGER_FULL_ACCESS:
        return "Run on the host without a sandbox"
    if source == "write_boundary":
        return "Write outside the default workspace/temp roots"
    if source == "read_only":
        return "Write while the sandbox is read-only"
    return ""
