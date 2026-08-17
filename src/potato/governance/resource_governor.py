# -*- coding: utf-8 -*-
"""Resource Governor — Policy evaluation + audit logging +
sandbox config compilation.

Core responsibilities: policy evaluation, audit recording, dynamic rule
addition, sandbox config compilation.
"""

from __future__ import annotations
import hashlib
import logging
from pathlib import Path
from typing import Optional

from .policy import (
    GovernancePolicy,
    GovernanceRule,
    GovernanceAction,
    GovernanceDecision,
    ToolCallSpec,
    FILE_READ_TOOLS,
    FILE_WRITE_TOOLS,
    load_governance_policy,
    save_governance_policy,
    _parse_match,
)
from .escalation import (
    DANGER_FULL_ACCESS,
    EscalationLevel,
    extra_writable_path,
    has_session_grant,
    parse_escalation_request,
    path_permission_key,
)
from .write_boundary import (
    refine_file_write_decision,
    sandbox_deny_paths,
    validate_extra_writable,
)
from .audit import AuditLog
from .tool_registry import DEFAULT_REGISTRY
from ..constant import WORKING_DIR
from ..utils.io_utils import get_sync_path_lock, run_sync_io

from ..sandbox import (
    SandboxCapability,
    SandboxConfig,
    MountSpec,
    probe_sandbox_support,
    detect_platform_mode,
)

logger = logging.getLogger(__name__)


# Module-level debounce: avoid spamming the auto-disable warning on every
# tool-execution check.  0 = never warned; otherwise the epoch of the last
# warning.
_sandbox_admin_warned_at: float = 0.0


class ResourceGovernor:
    """ResourceGovernor — core of policy and audit.

    Responsibilities:
        1. Policy evaluation: assert_policy(tool_call) → GovernanceDecision
        2. Audit logging: audit(tool_call, decision) records an audit log entry
        3. Sandbox config compilation: compile_sandbox_config() → SandboxConfig
        4. Dynamic rule addition: add_rule(...) after user approval

    NOT responsible for (TBD):
        - sandbox creation/destruction → managed by orchestration layer
        - Runtime/Agent scheduling → TBD
    """

    def __init__(
        self,
        workspace_dir: str,
        governance_dir: Optional[str] = None,
        coding_project_dir: Optional[str] = None,
    ):
        self.workspace_dir = Path(workspace_dir)
        # Coding project dir (Coding Mode). Falls back to the workspace
        # when unset so the CODING_PROJECT_DIR policy placeholder always
        # resolves to a concrete path.
        self.coding_project_dir = Path(
            coding_project_dir or workspace_dir,
        )
        # Policy is stored outside the workspace to prevent agent tampering.
        # Use ``<basename>_<hash>`` so two workspaces with the same basename
        # but different absolute paths (e.g. ``/Users/a/project`` vs
        # ``/Users/b/project``) do not share the same policy directory.
        if governance_dir is not None:
            self._governance_dir = Path(governance_dir)
        else:
            self._governance_dir = WORKING_DIR / "governance"
        ws_resolved = str(self.workspace_dir.resolve())
        ws_hash = hashlib.sha256(
            ws_resolved.encode("utf-8"),
        ).hexdigest()[:12]
        self._policy_dir = (
            self._governance_dir / f"{self.workspace_dir.name}_{ws_hash}"
        )
        self._policy_path = self._policy_dir / "policy.yaml"
        self._policy: Optional[GovernancePolicy] = None
        self._sandbox_available: bool = False
        self._sandbox_capability: Optional[SandboxCapability] = None
        self.session_sandbox_mode: str = "workspace-write"

    # ------------------------------------------------------------------
    # Lifecycle (kept but not expanded, overlaps with runtime)
    # ------------------------------------------------------------------

    @property
    def sandbox_available(self) -> bool:
        """Whether the current platform supports sandbox.

        Readable after start().
        """
        return self._sandbox_available

    @property
    def sandbox_capability(self) -> Optional[SandboxCapability]:
        """Probe result from start() (SandboxCapability)."""
        return self._sandbox_capability

    @staticmethod
    def _sandbox_globally_enabled() -> bool:
        """Read the global ``security.sandbox_enabled`` switch (config.json).

        Uses the mtime-cached :func:`load_config`, so this is cheap on the
        hot path and automatically reflects Console updates (``save_config``
        invalidates the cache). Defaults to False (sandbox off). On a config
        read error it returns True (fail-safe): a glitch then routes the
        command through the sandbox instead of running it unsandboxed.

        On Windows, if ``sandbox_enabled`` is True but the process lacks
        administrator privileges, the switch is treated as False for this
        session and a warning is logged.  The config file is NOT modified
        so the user's intent is preserved for future admin launches.
        """
        global _sandbox_admin_warned_at
        try:
            from ..config import load_config

            config = load_config()
            enabled = bool(config.security.sandbox_enabled)

            # Runtime guard: if sandbox is enabled but we're on Windows
            # without admin, treat as disabled for this session.
            if enabled:
                from ..utils.platform import is_windows_admin

                if not is_windows_admin():
                    import time as _time

                    now = _time.monotonic()
                    # Throttle: this method is called on every tool-execution
                    # check.  Without a debounce interval the same warning
                    # would be emitted hundreds of times per session.
                    # 30 s keeps the user informed without flooding the log.
                    if now - _sandbox_admin_warned_at > 30:
                        logger.warning(
                            "Windows sandbox inactive for this session: "
                            "sandbox_enabled is true but the process lacks "
                            "administrator privileges. To use the sandbox, "
                            "restart Potato as administrator.",
                        )
                        _sandbox_admin_warned_at = now
                    return False

            return enabled
        except Exception:
            logger.debug(
                "ResourceGovernor: failed to read sandbox_enabled; "
                "assuming enabled (fail-safe).",
                exc_info=True,
            )
            return True

    def _sandbox_usable(self) -> bool:
        """Effective sandbox availability: platform support AND global switch.

        When the operator turns the switch off, the sandbox is treated as
        unavailable and shell calls that needed containment are DENY
        (``SANDBOX_UNAVAILABLE``).
        """
        return self._sandbox_available and self._sandbox_globally_enabled()

    @property
    def sandbox_usable(self) -> bool:
        """Whether sandbox execution is supported and globally enabled."""
        return self._sandbox_usable()

    def start(self) -> None:
        """Load policy and probe sandbox capabilities."""
        with get_sync_path_lock(self._policy_path):
            self._policy_dir.mkdir(parents=True, exist_ok=True)
            self._policy = load_governance_policy(
                str(self._policy_dir),
                str(self.workspace_dir),
                str(self.coding_project_dir),
            )

            # Persist migrations/defaults while holding the same lock used by
            # approval transactions in other governor instances.
            try:
                save_governance_policy(
                    self._policy,
                    str(self._policy_dir),
                    str(self.workspace_dir),
                    str(self.coding_project_dir),
                )
            except Exception:
                logger.exception(
                    "ResourceGovernor.start: failed to persist policy.yaml",
                )

        self._sandbox_capability = probe_sandbox_support()
        self._sandbox_available = self._sandbox_capability.supported
        if not self._sandbox_available:
            logger.warning(
                "ResourceGovernor: sandbox not available — %s. "
                "Shell calls that need containment will be DENY "
                "(SANDBOX_UNAVAILABLE).",
                self._sandbox_capability.reason,
            )

    def stop(self) -> None:
        """Finish this governor without closing process-wide resources.

        Policy mutation methods persist their transactions immediately.
        Saving this instance's snapshot here could overwrite rules committed
        by another request. The shared AuditLog is closed at process exit,
        not when an individual request-scoped governor stops.
        """

    # ------------------------------------------------------------------
    # Core interface 1: Policy evaluation
    # ------------------------------------------------------------------

    def assert_policy(self, tc_spec: ToolCallSpec) -> GovernanceDecision:
        """Evaluate policy for a tool call.

        Flow:
            1. policy.evaluate(tc_spec) → GovernanceDecision
            2. Explicit ``danger-full-access`` → session grant or ASK
            3. Sandbox unavailable: a shell command that needed
               containment → DENY (SANDBOX_UNAVAILABLE)
            4. Sandboxed shell decisions (including ASK) compile a cage
            5. Log the governance decision (observability)
            6. Return decision (does NOT record audit)

        Returns GovernanceDecision:
            ALLOW            → explicit resource tool executes directly;
                               bash tool executes with sandbox
                               pre-authorization
            ALLOW_UNSANDBOXED→ explicit host grant only (model declared
                               danger-full-access and a human approved it)
            DENY             → rejected
            ASK              → ask user
            SANDBOX_FALLBACK → bash tool with no rule match, sandbox fallback
        """
        decision = self.policy.evaluate(tc_spec)
        decision = refine_file_write_decision(
            decision,
            tc_spec,
            workspace_dir=self.workspace_dir,
            coding_project_dir=self.coding_project_dir,
            policy_dir=self._policy_dir,
        )

        is_shell = DEFAULT_REGISTRY.get_type(tc_spec.tool_name) == "shell"
        if is_shell:
            requested, esc_error = parse_escalation_request(tc_spec.raw_params)
        else:
            requested, esc_error = None, None
        if esc_error:
            return GovernanceDecision(
                action=GovernanceAction.DENY,
                reason=esc_error,
                findings=decision.findings,
                source="escalation",
            )
        if (
            is_shell
            and requested
            in (
                EscalationLevel.DANGER_FULL_ACCESS,
                EscalationLevel.NETWORK,
                EscalationLevel.PATH,
            )
            and decision.action is not GovernanceAction.DENY
        ):
            extra_path = ""
            if requested is EscalationLevel.PATH:
                extra_path, path_err = validate_extra_writable(
                    extra_writable_path(tc_spec.raw_params),
                    workspace_dir=self.workspace_dir,
                    coding_project_dir=self.coding_project_dir,
                    policy_dir=self._policy_dir,
                )
                if path_err or not extra_path:
                    return GovernanceDecision(
                        action=GovernanceAction.DENY,
                        reason=path_err or "invalid extra writable path",
                        findings=decision.findings,
                        source="escalation",
                    )
            permission = (
                path_permission_key(extra_path)
                if requested is EscalationLevel.PATH
                else requested.value
            )
            just = str(
                (tc_spec.raw_params or {}).get("justification") or "",
            ).strip()
            increment_ready = has_session_grant(
                session_id=tc_spec.session_id,
                tool_name=tc_spec.tool_name,
                command=tc_spec.target,
                permission=permission,
            )
            # A session increment only adds capability. It must not
            # replace a policy ASK (STRICT / HIGH finding / builtin).
            if increment_ready and decision.action in (
                GovernanceAction.ALLOW,
                GovernanceAction.SANDBOX_FALLBACK,
            ):
                if requested is EscalationLevel.DANGER_FULL_ACCESS:
                    return GovernanceDecision(
                        action=GovernanceAction.ALLOW_UNSANDBOXED,
                        reason="session grant: danger-full-access",
                        findings=decision.findings,
                        source="escalation",
                    )
                granted = GovernanceDecision(
                    action=GovernanceAction.ALLOW,
                    reason=f"session grant: {requested.value}",
                    findings=decision.findings,
                    source="escalation",
                )
                if self._sandbox_usable():
                    granted.sandbox_config = self.compile_sandbox_config(
                        tc_spec,
                        allow_network=requested is EscalationLevel.NETWORK,
                        extra_writable=extra_path or None,
                    )
                return granted
            if increment_ready and decision.action is GovernanceAction.ASK:
                ask = GovernanceDecision(
                    action=GovernanceAction.ASK,
                    reason=decision.reason,
                    findings=decision.findings,
                    source=decision.source,
                )
            else:
                ask = GovernanceDecision(
                    action=GovernanceAction.ASK,
                    reason=just
                    or (
                        "request extra writable path"
                        if requested is EscalationLevel.PATH
                        else "request network access"
                        if requested is EscalationLevel.NETWORK
                        else "request host execution"
                    ),
                    findings=decision.findings,
                    source="escalation",
                )
            if (
                requested is not EscalationLevel.DANGER_FULL_ACCESS
                and self._sandbox_usable()
            ):
                ask.sandbox_config = self.compile_sandbox_config(
                    tc_spec,
                    allow_network=requested is EscalationLevel.NETWORK,
                    extra_writable=extra_path or None,
                )
            return ask

        needs_sandbox = is_shell and decision.action in (
            GovernanceAction.SANDBOX_FALLBACK,
            GovernanceAction.ALLOW,
            GovernanceAction.ASK,
        )

        # Sandbox not usable (platform unsupported OR the global
        # security.sandbox_enabled switch is off). A missing cage must not
        # become host execution — that is an implicit privilege expansion.
        # Unsandboxed run requires a separate, explicit grant before the call.
        if needs_sandbox and not self._sandbox_usable():
            # Standing host mode is the explicit "run without a cage"
            # knob. It must not collapse to SANDBOX_UNAVAILABLE.
            if self.session_sandbox_mode == DANGER_FULL_ACCESS:
                if decision.action in (
                    GovernanceAction.ALLOW,
                    GovernanceAction.SANDBOX_FALLBACK,
                ):
                    return GovernanceDecision(
                        action=GovernanceAction.ALLOW_UNSANDBOXED,
                        reason="standing sandbox_mode=danger-full-access",
                        findings=decision.findings,
                        source=decision.source,
                    )
                if decision.action is GovernanceAction.ASK:
                    decision.sandbox_config = None
                    return decision
            if self._sandbox_available:
                reason = "sandbox disabled by config"
            else:
                capability_reason = (
                    self._sandbox_capability.reason
                    if self._sandbox_capability is not None
                    else "sandbox probe has not completed"
                )
                reason = f"sandbox unavailable ({capability_reason})"
            logger.info(
                "ResourceGovernor: %s, denying '%s' (SANDBOX_UNAVAILABLE)",
                reason,
                tc_spec.tool_name,
            )
            decision = GovernanceDecision(
                action=GovernanceAction.DENY,
                reason=f"SANDBOX_UNAVAILABLE: {reason}",
                findings=decision.findings,
                source=decision.source,
            )

        # Shell ALLOW / fallback / policy ASK stay caged. Host execution
        # is only the explicit danger-full-access branch above.
        if (
            decision.action
            in (
                GovernanceAction.SANDBOX_FALLBACK,
                GovernanceAction.ALLOW,
                GovernanceAction.ASK,
            )
            and is_shell
        ):
            decision.sandbox_config = self.compile_sandbox_config(tc_spec)

        # Observability: log every governance decision so operators can
        # trace policy evaluation results without querying audit.db.
        # ``target`` is truncated to keep log lines bounded.
        target_repr = (tc_spec.target or "")[:120]
        # sandbox backend actually used for this call: the compiled
        # config's mode (bubblewrap/landlock/...), or "-" when the
        # decision does not route through a sandbox.
        sandbox_mode = (
            decision.sandbox_config.mode.value
            if decision.sandbox_config is not None
            else "-"
        )
        logger.info(
            "governance decision: tool=%s target=%r action=%s source=%s "
            "sandbox=%s reason=%s",
            tc_spec.tool_name,
            target_repr,
            decision.action.value,
            decision.source,
            sandbox_mode,
            decision.reason,
        )
        return decision

    # ------------------------------------------------------------------
    # Core interface 2: Audit logging
    # ------------------------------------------------------------------

    def audit(
        self,
        tc_spec: ToolCallSpec,
        decision: GovernanceDecision,
    ) -> None:
        """Record a governance decision to the audit log.

        Callers should invoke this after ``assert_policy()`` to persist
        the decision for compliance / forensics:

            decision = governor.assert_policy(tc_spec)
            governor.audit(tc_spec, decision)
        """
        if self._policy is not None and self._policy.audit_level == "none":
            return
        self.audit_log.record(
            str(self.workspace_dir),
            tc_spec,
            decision,
        )

    # ------------------------------------------------------------------
    # Core interface 3: Compile sandbox config
    # ------------------------------------------------------------------

    def compile_sandbox_config(  # pylint: disable=unused-argument
        self,
        tc_spec: ToolCallSpec,
        *,
        allow_network: bool = False,
        extra_writable: str | None = None,
    ) -> SandboxConfig:
        """Compile sandbox filesystem permission config based on policy.

        Sandbox security model:
            - Workspace is the working directory, always mounted
              readwrite (Bash needs it to work)
            - Paths from FILE_READ_TOOLS / FILE_WRITE_TOOLS in
              user_rules are compiled into mounts
            - deny_paths block sensitive paths (defense-in-depth)
            - Policy decisions control whether a command can execute;
              sandbox controls filesystem boundaries

        Mounts compilation logic:
            Iterate over user_rules, for each rule:
              - Parse match → (tool_name, pattern)
              - If tool_name ∈ FILE_READ_TOOLS → readonly mount
              - If tool_name ∈ FILE_WRITE_TOOLS → readwrite mount
            Same path uses the most permissive access (write > read).

        Returns SandboxConfig dataclass (from potato.sandbox.config).
        """
        ws = str(self.workspace_dir)
        workspace_writable = self.session_sandbox_mode != "read-only"

        # ── Compile mounts from user_rules ──
        # path → writable mapping: same path uses the most permissive access
        mount_map: dict[str, bool] = {}

        for rule in self.policy.user_rules:
            try:
                rule_tool, rule_pattern = _parse_match(rule.match)
            except (ValueError, IndexError):
                continue

            # Extract path from pattern: strip trailing * and other
            # wildcards to get directory prefix
            path = self._resolve_mount_path(rule_pattern, ws)
            if not path:
                continue

            if rule_tool in FILE_READ_TOOLS:
                # readonly mount, but keep write if already present
                if path not in mount_map:
                    mount_map[path] = False
            elif rule_tool in FILE_WRITE_TOOLS:
                # readwrite mount
                mount_map[path] = True

        mounts = [
            MountSpec(
                path=p,
                writable=w if workspace_writable else False,
            )
            for p, w in mount_map.items()
        ]
        mounts.insert(
            0,
            MountSpec(path=ws, writable=workspace_writable),
        )

        # Coding project dir is readwrite by default (Coding Mode). When
        # it is distinct from the workspace, mount it explicitly so Bash
        # can write there; the policy ALLOW rule alone is not enough for
        # sandboxed shell tools.
        cpd = str(self.coding_project_dir)
        if cpd and cpd != ws and not any(m.path == cpd for m in mounts):
            mounts.append(
                MountSpec(path=cpd, writable=workspace_writable),
            )
        if extra_writable and not any(
            m.path == extra_writable for m in mounts
        ):
            mounts.append(MountSpec(path=extra_writable, writable=True))

        return SandboxConfig(
            mode=detect_platform_mode(),
            workspace_dir=ws,
            mounts=mounts,
            deny_paths=sandbox_deny_paths(
                workspace_dir=ws,
                coding_project_dir=self.coding_project_dir,
                policy_dir=self._policy_dir,
            ),
            # Default is no network (Codex workspace-write). Opening the
            # network is an explicit increment: sandbox_permissions=network.
            # Domain filtering is best-effort, so the grant is all outbound.
            network_allow=["*"] if allow_network else [],
            timeout_seconds=60,
            env_vars={k: "" for k in self.policy.env_blacklist},
        )

    @staticmethod
    def _resolve_mount_path(pattern: str, workspace_dir: str) -> str:
        """Derive a mount path from a rule pattern.

        Strategy:
            - WORKSPACE_DIR/* → workspace_dir (mount as a whole)
            - /absolute/path/* → /absolute/path (take directory part)
            - relative path → workspace_dir / relative (take directory part)
            - Pure wildcards (*, **) → skip, cannot derive a concrete path
        """
        p = pattern.rstrip("*").rstrip("/")

        if not p or p == ".":
            return ""

        # WORKSPACE_DIR placeholder (defensive: should already be
        # replaced at load time)
        if "WORKSPACE_DIR" in p:
            p = p.replace("WORKSPACE_DIR", workspace_dir)

        # Absolute path
        if p.startswith("/"):
            return p

        # Relative path → resolve based on workspace
        return str(Path(workspace_dir) / p)

    # ------------------------------------------------------------------
    # Core interface 4: Dynamic rule addition
    # ------------------------------------------------------------------

    def add_rule(self, rule: GovernanceRule) -> None:
        """Dynamically append a rule to the policy after user approval.

        Approved rules carry a duration (session / permanent).
        The rule is also persisted to policy.yaml.
        Note: rules are only appended to user_rules; builtin_rules are
        immutable.
        """
        with get_sync_path_lock(self._policy_path):
            policy = load_governance_policy(
                str(self._policy_dir),
                str(self.workspace_dir),
                str(self.coding_project_dir),
            )
            policy.add_rule(rule)
            save_governance_policy(
                policy,
                str(self._policy_dir),
                str(self.workspace_dir),
                str(self.coding_project_dir),
            )
            self._policy = policy

    async def add_approved_rule(
        self,
        tc_spec: ToolCallSpec,
        *,
        generalized_target: str,
        duration: str = "permanent",
    ) -> bool:
        """Add an ALLOW rule for a human-approved tool call.

        Human exact/similar grants persist like Codex ``default.rules``:
        ``duration=permanent`` and no session binding, so the next chat
        does not re-prompt. Automatic review must never call this.

        Args:
            generalized_target: the generalized target/pattern (e.g.
                ``"git *"``), already computed upstream by
                ``generalize_target_for_approval``.
            duration: ``permanent`` (default) or ``session``.

        Returns True if a rule was actually added, False if skipped
        (e.g. builtin ask, empty target, hard write boundary).
        """
        if self.is_builtin_ask(tc_spec):
            return False
        if self._is_hard_denied_write(tc_spec):
            return False

        if (
            DEFAULT_REGISTRY.get_type(tc_spec.tool_name) == "computer"
            or tc_spec.tool_name.startswith("Computer")
        ):
            from ..computer_use.protect import live_observation_bundle_id
            from ..computer_use.settings import is_stable_app_id

            live_bundle_id = live_observation_bundle_id(tc_spec.raw_params)
            if (
                not is_stable_app_id(live_bundle_id)
                or tc_spec.target != live_bundle_id
            ):
                logger.debug(
                    "ResourceGovernor: skipping Computer Use rule without "
                    "a live observation bundle id",
                )
                return False
            generalized_target = live_bundle_id

        try:
            if not generalized_target:
                logger.debug(
                    "ResourceGovernor: empty pattern, skipping rule "
                    "for tool=%s target=%s",
                    tc_spec.tool_name,
                    tc_spec.target,
                )
                return False

            persist_duration = (
                "session" if duration == "session" else "permanent"
            )
            match = f"{tc_spec.tool_name}({generalized_target})"
            rule = GovernanceRule(
                match=match,
                action=GovernanceAction.ALLOW,
                reason="user approved",
                grantee=tc_spec.agent_id or "*",
                duration=persist_duration,
                session_id=(
                    tc_spec.session_id
                    if persist_duration == "session"
                    else None
                ),
            )
            await run_sync_io(self.add_rule, rule)
            if persist_duration == "permanent":
                from .global_rules import append_global_user_rule

                await run_sync_io(append_global_user_rule, rule)
            logger.info(
                "ResourceGovernor: added approved rule: %s duration=%s",
                rule.match,
                persist_duration,
            )
            return True
        except Exception:
            logger.debug(
                "ResourceGovernor: failed to persist approved rule",
                exc_info=True,
            )
            return False

    def _is_hard_denied_write(self, tc_spec: ToolCallSpec) -> bool:
        """True when a file write hits a hard capability boundary."""
        from .write_boundary import (
            canonical_path,
            classify_write_target,
            default_writable_roots,
            extra_denied_roots,
            is_file_write_tool,
            read_only_subpaths,
        )

        if not is_file_write_tool(tc_spec.tool_name):
            return False

        project = [canonical_path(self.workspace_dir)]
        if self.coding_project_dir:
            coding = canonical_path(self.coding_project_dir)
            if coding not in project:
                project.append(coding)
        kind = classify_write_target(
            tc_spec.target,
            default_writable_roots(
                self.workspace_dir,
                self.coding_project_dir,
            ),
            read_only_subpaths(project)
            + extra_denied_roots(policy_dir=self._policy_dir),
        )
        return kind == "denied"

    def is_builtin_ask(self, tc_spec: ToolCallSpec) -> bool:
        """Determine whether a tool call's ASK comes from builtin_rules.

        builtin ask → no rule recorded on approval (asks every time)
        user ask   → rule recorded on approval (won't ask next time)

        Called by tool_adapter's approval flow to decide whether to
        persist a new rule.
        """
        if not self._policy:
            return False
        source = self._policy.evaluate_source(tc_spec)
        return source == "builtin_rules"

    # ------------------------------------------------------------------
    # Property access
    # ------------------------------------------------------------------

    @property
    def policy(self) -> GovernancePolicy:
        if self._policy is None:
            raise RuntimeError("ResourceGovernor not started")
        return self._policy

    @property
    def audit_log(self) -> AuditLog:
        """Get the global AuditLog singleton."""
        return AuditLog.get_instance(
            db_dir=self._governance_dir,
        )
