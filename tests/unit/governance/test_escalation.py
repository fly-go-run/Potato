# -*- coding: utf-8 -*-
"""Explicit sandbox escalation + session grants."""
from __future__ import annotations

import pytest

from potato.governance.escalation import (
    DANGER_FULL_ACCESS,
    EscalationLevel,
    NETWORK,
    PATH,
    READ_ONLY,
    WORKSPACE_WRITE,
    apply_standing_sandbox_mode,
    clear_session_grants,
    describe_permission_increment,
    has_session_grant,
    parse_escalation_request,
    path_permission_key,
    remember_session_grant,
    resolve_sandbox_mode,
)
from potato.governance.policy import GovernanceAction, ToolCallSpec
from potato.governance.resource_governor import ResourceGovernor
from potato.sandbox import SandboxCapability

from .test_policy import _make_governor, _tc


@pytest.fixture(autouse=True)
def _clean_grants():
    clear_session_grants()
    yield
    clear_session_grants()


def test_parse_default_is_no_escalation():
    level, err = parse_escalation_request({"command": "ls"})
    assert level is None
    assert err is None


def test_parse_justification_without_permissions_errors():
    level, err = parse_escalation_request({"justification": "need host"})
    assert level is None
    assert err is not None


def test_parse_full_access_requires_justification():
    level, err = parse_escalation_request(
        {"sandbox_permissions": DANGER_FULL_ACCESS},
    )
    assert level is None
    assert "justification" in err


def test_parse_network_requires_justification():
    level, err = parse_escalation_request(
        {"sandbox_permissions": NETWORK},
    )
    assert level is None
    assert "justification" in err


def test_parse_network_ok():
    level, err = parse_escalation_request(
        {
            "sandbox_permissions": NETWORK,
            "justification": "git fetch needs origin",
        },
    )
    assert err is None
    assert level is EscalationLevel.NETWORK


def test_parse_path_requires_extra_dir():
    level, err = parse_escalation_request(
        {
            "sandbox_permissions": PATH,
            "justification": "need desktop",
        },
    )
    assert level is None
    assert "additional_writable_path" in err


def test_parse_path_ok():
    level, err = parse_escalation_request(
        {
            "sandbox_permissions": PATH,
            "justification": "need desktop",
            "additional_writable_path": "/tmp/extra-out",
        },
    )
    assert err is None
    assert level is EscalationLevel.PATH


def test_exact_grant_does_not_glob_match():
    remember_session_grant(
        session_id="s1",
        tool_name="Bash",
        pattern="ls *",
        permission=DANGER_FULL_ACCESS,
        glob=False,
    )
    assert has_session_grant(
        session_id="s1",
        tool_name="Bash",
        command="ls *",
    )
    assert not has_session_grant(
        session_id="s1",
        tool_name="Bash",
        command="ls * ; rm -rf ~",
    )


def test_resolve_sandbox_mode_falls_back_to_agent_profile(monkeypatch):
    from types import SimpleNamespace

    monkeypatch.setattr(
        "potato.config.config.load_agent_config",
        lambda _agent: SimpleNamespace(sandbox_mode=READ_ONLY),
    )
    assert (
        resolve_sandbox_mode({"agent_id": "agent-1"}) == READ_ONLY
    )


def test_host_grant_implies_network_grant():
    remember_session_grant(
        session_id="s1",
        tool_name="Bash",
        pattern="git *",
        permission=DANGER_FULL_ACCESS,
        glob=True,
    )
    assert has_session_grant(
        session_id="s1",
        tool_name="Bash",
        command="git fetch",
        permission=NETWORK,
    )


def test_parse_full_access_ok():
    level, err = parse_escalation_request(
        {
            "sandbox_permissions": DANGER_FULL_ACCESS,
            "justification": "git push needs the network and ssh agent",
        },
    )
    assert err is None
    assert level is EscalationLevel.DANGER_FULL_ACCESS


def test_session_grant_matches_similar_prefix():
    remember_session_grant(
        session_id="s1",
        tool_name="Bash",
        pattern="ssh *",
        permission=DANGER_FULL_ACCESS,
        glob=True,
    )
    assert has_session_grant(
        session_id="s1",
        tool_name="Bash",
        command="ssh home",
    )
    assert not has_session_grant(
        session_id="s2",
        tool_name="Bash",
        command="ssh home",
    )


def _shell_spec(command: str, **extra) -> ToolCallSpec:
    params = {"command": command, **extra}
    return ToolCallSpec(
        tool_name="Bash",
        target=command,
        agent_id="test-agent",
        session_id="sess-1",
        raw_params=params,
    )


class TestGovernorEscalation:
    @pytest.fixture()
    def governor(self, tmp_path):
        gov = _make_governor(tmp_path)
        gov.start()
        yield gov
        gov.stop()

    def test_missing_justification_denied(self, governor):
        decision = governor.assert_policy(
            _shell_spec(
                "ssh home",
                sandbox_permissions=DANGER_FULL_ACCESS,
            ),
        )
        assert decision.action is GovernanceAction.DENY
        assert decision.source == "escalation"

    def test_explicit_host_request_asks(self, governor):
        decision = governor.assert_policy(
            _shell_spec(
                "ssh home",
                sandbox_permissions=DANGER_FULL_ACCESS,
                justification="need the user's ssh agent",
            ),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "escalation"
        assert decision.sandbox_config is None

    def test_session_grant_skips_prompt(self, governor):
        remember_session_grant(
            session_id="sess-1",
            tool_name="Bash",
            pattern="ssh *",
            glob=True,
        )
        decision = governor.assert_policy(
            _shell_spec(
                "ssh home",
                sandbox_permissions=DANGER_FULL_ACCESS,
                justification="need the user's ssh agent",
            ),
        )
        assert decision.action is GovernanceAction.ALLOW_UNSANDBOXED
        assert decision.sandbox_config is None

    def test_policy_ask_keeps_sandbox(self, governor, monkeypatch):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        decision = governor.assert_policy(_tc("Bash", "ls -lh ~/.ssh"))
        assert decision.action is GovernanceAction.ASK
        assert decision.sandbox_config is not None

    def test_default_sandbox_has_no_network(self, governor, monkeypatch):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        decision = governor.assert_policy(_tc("Bash", "echo hello"))
        assert decision.sandbox_config is not None
        assert decision.sandbox_config.network_allow == []

    def test_network_request_asks_inside_cage(self, governor, monkeypatch):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        decision = governor.assert_policy(
            _shell_spec(
                "git fetch",
                sandbox_permissions=NETWORK,
                justification="need origin",
            ),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "escalation"
        assert decision.sandbox_config is not None
        assert decision.sandbox_config.network_allow == ["*"]

    def test_session_network_grant_does_not_skip_strict_ask(
        self,
        governor,
        monkeypatch,
    ):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        governor.policy.execution_level = "strict"
        remember_session_grant(
            session_id="sess-1",
            tool_name="Bash",
            pattern="git *",
            permission=NETWORK,
            glob=True,
        )
        decision = governor.assert_policy(
            _shell_spec(
                "git fetch origin",
                sandbox_permissions=NETWORK,
                justification="need origin",
            ),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "STRICT mode"
        assert decision.sandbox_config is not None
        assert decision.sandbox_config.network_allow == ["*"]

    def test_justification_on_file_tool_is_ignored(self, governor):
        spec = _tc("Read", str(governor.workspace_dir) + "/notes.txt")
        spec.raw_params = {"justification": "because"}
        decision = governor.assert_policy(spec)
        assert decision.action is not GovernanceAction.DENY or (
            "justification" not in decision.reason
        )

    def test_standing_full_access_when_sandbox_unavailable(
        self,
        governor,
    ):
        governor.session_sandbox_mode = DANGER_FULL_ACCESS
        governor._sandbox_available = False
        decision = governor.assert_policy(_tc("Bash", "echo hello"))
        assert decision.action is GovernanceAction.ALLOW_UNSANDBOXED

    def test_session_network_grant_stays_sandboxed(
        self,
        governor,
        monkeypatch,
    ):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        remember_session_grant(
            session_id="sess-1",
            tool_name="Bash",
            pattern="git *",
            permission=NETWORK,
            glob=True,
        )
        decision = governor.assert_policy(
            _shell_spec(
                "git fetch origin",
                sandbox_permissions=NETWORK,
                justification="need origin",
            ),
        )
        assert decision.action is GovernanceAction.ALLOW
        assert decision.sandbox_config is not None
        assert decision.sandbox_config.network_allow == ["*"]

    def test_cannot_escalate_a_hard_deny(self, governor):
        decision = governor.assert_policy(
            _shell_spec(
                "sudo rm -rf /",
                sandbox_permissions=DANGER_FULL_ACCESS,
                justification="please",
            ),
        )
        assert decision.action is GovernanceAction.DENY
        assert decision.source != "escalation"

    def test_path_request_asks_and_mounts_extra(self, governor, monkeypatch):
        from pathlib import Path

        extra_dir = Path(governor.workspace_dir).parent / "extra-out"
        extra_dir.mkdir(exist_ok=True)
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        decision = governor.assert_policy(
            _shell_spec(
                "cp notes.txt " + str(extra_dir / "notes.txt"),
                sandbox_permissions=PATH,
                justification="export next to the project",
                additional_writable_path=str(extra_dir),
            ),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "escalation"
        assert decision.sandbox_config is not None
        assert any(
            mount.path == str(extra_dir.resolve()) and mount.writable
            for mount in decision.sandbox_config.mounts
        )

    def test_session_path_grant_skips_prompt(self, governor, monkeypatch):
        from pathlib import Path

        extra_dir = Path(governor.workspace_dir).parent / "extra-grant"
        extra_dir.mkdir(exist_ok=True)
        extra = str(extra_dir.resolve())
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        remember_session_grant(
            session_id="sess-1",
            tool_name="Bash",
            pattern="cp *",
            permission=path_permission_key(extra),
            glob=True,
        )
        decision = governor.assert_policy(
            _shell_spec(
                "cp notes.txt " + str(extra_dir / "notes.txt"),
                sandbox_permissions=PATH,
                justification="export next to the project",
                additional_writable_path=extra,
            ),
        )
        assert decision.action is GovernanceAction.ALLOW
        assert decision.sandbox_config is not None
        assert any(
            mount.path == extra and mount.writable
            for mount in decision.sandbox_config.mounts
        )

    def test_read_only_sandbox_mounts_workspace_ro(
        self,
        governor,
        monkeypatch,
    ):
        monkeypatch.setattr(governor, "_sandbox_usable", lambda: True)
        governor.session_sandbox_mode = READ_ONLY
        decision = governor.assert_policy(_tc("Bash", "echo hello"))
        assert decision.sandbox_config is not None
        workspace_mounts = [
            mount
            for mount in decision.sandbox_config.mounts
            if mount.path == str(governor.workspace_dir)
        ]
        assert workspace_mounts
        assert workspace_mounts[0].writable is False


def test_resolve_sandbox_mode_defaults_to_workspace():
    assert resolve_sandbox_mode({}) == WORKSPACE_WRITE
    assert resolve_sandbox_mode(None) == WORKSPACE_WRITE
    assert resolve_sandbox_mode({"sandbox_mode": READ_ONLY}) == READ_ONLY
    assert (
        resolve_sandbox_mode({"sandbox_mode": "danger-full-access"})
        == DANGER_FULL_ACCESS
    )


def test_standing_read_only_asks_for_writes():
    from potato.governance.policy import GovernanceDecision

    decision = GovernanceDecision(
        action=GovernanceAction.ALLOW,
        reason="workspace write",
        source="user_rules",
    )
    updated = apply_standing_sandbox_mode(
        decision,
        _tc("Write", "notes.txt"),
        READ_ONLY,
    )
    assert updated.action is GovernanceAction.ASK
    assert updated.source == "read_only"


def test_permission_increment_text():
    assert "network" in describe_permission_increment(
        source="escalation",
        raw_params={
            "sandbox_permissions": NETWORK,
            "justification": "need origin",
        },
    ).lower()
    assert "Desktop" in describe_permission_increment(
        source="escalation",
        raw_params={
            "sandbox_permissions": PATH,
            "justification": "save export",
            "additional_writable_path": "/tmp/Desktop",
        },
    ) or "writable" in describe_permission_increment(
        source="escalation",
        raw_params={
            "sandbox_permissions": PATH,
            "justification": "save export",
            "additional_writable_path": "/tmp/Desktop",
        },
    )
    assert "host" in describe_permission_increment(
        source="escalation",
        raw_params={
            "sandbox_permissions": DANGER_FULL_ACCESS,
            "justification": "need ssh",
        },
    ).lower()
    assert describe_permission_increment(
        source="write_boundary",
        raw_params={},
    )


def test_standing_full_access_unsandboxes_routine_shell():
    from potato.governance.policy import GovernanceDecision

    decision = GovernanceDecision(
        action=GovernanceAction.SANDBOX_FALLBACK,
        reason="sandbox fallback",
        source="sandbox",
    )
    updated = apply_standing_sandbox_mode(
        decision,
        _tc("Bash", "echo hello"),
        DANGER_FULL_ACCESS,
    )
    assert updated.action is GovernanceAction.ALLOW_UNSANDBOXED
    assert updated.sandbox_config is None


def test_standing_full_access_does_not_override_deny():
    from potato.governance.policy import GovernanceDecision

    decision = GovernanceDecision(
        action=GovernanceAction.DENY,
        reason="blocked",
        source="builtin_rules",
    )
    updated = apply_standing_sandbox_mode(
        decision,
        _tc("Bash", "sudo rm -rf /"),
        DANGER_FULL_ACCESS,
    )
    assert updated.action is GovernanceAction.DENY


def test_sandbox_unavailable_still_denies_without_grant(tmp_path):
    gov = ResourceGovernor(
        str(tmp_path),
        governance_dir=str(tmp_path / "governance"),
    )
    gov.start()
    gov._sandbox_available = False
    gov._sandbox_capability = SandboxCapability(
        supported=False,
        mode=None,
        reason="test",
    )
    decision = gov.assert_policy(_tc("Bash", "echo hello"))
    assert decision.action is GovernanceAction.DENY
    assert "SANDBOX_UNAVAILABLE" in decision.reason
    gov.stop()
