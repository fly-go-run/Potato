# -*- coding: utf-8 -*-
"""Write-boundary + P0 capability regressions."""
from __future__ import annotations

import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from potato.governance.policy import (
    GovernanceAction,
    GovernanceDecision,
    _create_default_policy,
)
from potato.governance.audit import AuditLog
from potato.governance.write_boundary import (
    assert_inside_writable_roots,
    canonical_path,
    classify_write_target,
    default_writable_roots,
    extra_denied_roots,
    is_file_write_tool,
    read_only_subpaths,
    refine_file_write_decision,
    validate_extra_writable,
)
from potato.governance.tool_registry import DEFAULT_REGISTRY
from potato.sandbox import SandboxCapability

from .test_policy import _make_governor, _tc


def test_tmp_and_tmpdir_are_writable_roots(tmp_path):
    import tempfile

    roots = default_writable_roots(tmp_path)
    assert canonical_path("/tmp") in roots
    assert canonical_path(tempfile.gettempdir()) in roots
    tmpdir = os.environ.get("TMPDIR")
    if tmpdir:
        assert canonical_path(tmpdir) in roots


def test_desktop_is_outside_default_roots(tmp_path):
    roots = default_writable_roots(tmp_path)
    denied = read_only_subpaths([canonical_path(tmp_path)])
    desktop = str(Path.home() / "Desktop" / "note.txt")
    assert classify_write_target(desktop, roots, denied) == "outside"


def test_git_hooks_are_denied_inside_workspace(tmp_path):
    ws = tmp_path / "ws"
    ws.mkdir()
    hooks = ws / ".git" / "hooks"
    hooks.mkdir(parents=True)
    roots = default_writable_roots(ws)
    denied = read_only_subpaths([canonical_path(ws)])
    assert (
        classify_write_target(str(hooks / "pre-commit"), roots, denied)
        == "denied"
    )


class TestGovernorWriteBoundary:
    @pytest.fixture()
    def governor(self, tmp_path):
        gov = _make_governor(tmp_path)
        gov.start()
        yield gov
        gov.stop()
        AuditLog._instance = None

    def test_write_outside_workspace_asks(self, governor):
        decision = governor.assert_policy(
            _tc("Write", "/usr/local/potato-p0.txt"),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "write_boundary"

    def test_write_tmp_still_allows(self, governor):
        decision = governor.assert_policy(_tc("Write", "/tmp/potato-p0.txt"))
        assert decision.action is GovernanceAction.ALLOW

    def test_write_git_hooks_denied(self, governor, tmp_path):
        hooks = tmp_path / ".git" / "hooks"
        hooks.mkdir(parents=True)
        decision = governor.assert_policy(
            _tc("Write", str(hooks / "pre-commit")),
        )
        assert decision.action is GovernanceAction.DENY
        assert decision.source == "write_boundary"

    def test_write_agent_json_denied(self, governor, tmp_path):
        decision = governor.assert_policy(
            _tc("Write", str(tmp_path / "agent.json")),
        )
        assert decision.action is GovernanceAction.DENY

    def test_symlink_workspace_escape_asks(self, tmp_path):
        real = tmp_path / "real-ws"
        real.mkdir()
        link = tmp_path / "link-ws"
        link.symlink_to(real)
        (real / "out").symlink_to(Path.home())
        decision = refine_file_write_decision(
            GovernanceDecision(
                action=GovernanceAction.ALLOW,
                reason="workspace glob",
                source="user_rules",
            ),
            _tc("Write", str(link / "out" / "stolen.txt")),
            workspace_dir=str(link),
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "write_boundary"

    def test_dotdot_escape_asks(self, tmp_path):
        from potato.governance.write_boundary import (
            _is_symlink_escape,
            _spelled_roots,
        )

        target = (
            f"{tmp_path}/sub/../../../../../../../../usr/local/"
            "potato-dotdot.txt"
        )
        assert _is_symlink_escape(
            target,
            [canonical_path(tmp_path)],
            _spelled_roots(tmp_path),
        )
        decision = refine_file_write_decision(
            GovernanceDecision(
                action=GovernanceAction.ALLOW,
                reason="workspace glob",
                source="user_rules",
            ),
            _tc("Write", target),
            workspace_dir=tmp_path,
        )
        assert decision.action is GovernanceAction.ASK

    def test_symlink_escape_out_of_workspace_asks(self, governor, tmp_path):
        link = tmp_path / "link-home"
        link.symlink_to(Path.home())
        decision = governor.assert_policy(
            _tc("Write", str(link / "potato-p0-escape.txt")),
        )
        assert decision.action is GovernanceAction.ASK


class TestHighCannotBeCoveredByAllow:
    def test_user_allow_cannot_cover_high_finding(self, tmp_path, monkeypatch):
        policy = _create_default_policy(str(tmp_path), str(tmp_path))
        policy.execution_level = "smart"
        finding = SimpleNamespace(
            severity="HIGH",
            title="risky write",
            description="HIGH: credential-like payload",
            detector="pattern_detector",
        )
        monkeypatch.setattr(
            policy,
            "_deep_security_scan",
            lambda *_a, **_k: [finding],
        )
        decision = policy.evaluate(_tc("Write", str(tmp_path / "ok.txt")))
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "detection_rules"


class TestScanFailClosed:
    def test_write_scan_failure_denies(self, tmp_path, monkeypatch):
        policy = _create_default_policy(str(tmp_path), str(tmp_path))

        def _boom(*_a, **_k):
            raise RuntimeError("scanner down")

        monkeypatch.setattr(policy, "_deep_security_scan", _boom)
        decision = policy.evaluate(_tc("Write", str(tmp_path / "ok.txt")))
        assert decision.action is GovernanceAction.DENY
        assert "scan unavailable" in decision.reason

    def test_shell_scan_failure_denies(self, tmp_path, monkeypatch):
        policy = _create_default_policy(str(tmp_path), str(tmp_path))

        def _boom(*_a, **_k):
            raise RuntimeError("scanner down")

        monkeypatch.setattr(policy, "_deep_security_scan", _boom)
        decision = policy.evaluate(_tc("Bash", "echo hi"))
        assert decision.action is GovernanceAction.DENY

    def test_read_scan_failure_fail_open(self, tmp_path, monkeypatch):
        policy = _create_default_policy(str(tmp_path), str(tmp_path))

        def _boom(*_a, **_k):
            raise RuntimeError("scanner down")

        monkeypatch.setattr(policy, "_deep_security_scan", _boom)
        decision = policy.evaluate(_tc("Read", str(tmp_path / "ok.txt")))
        assert decision.action is GovernanceAction.ALLOW


class TestSandboxUnavailable:
    def test_shell_does_not_become_unsandboxed(self, tmp_path):
        gov = _make_governor(tmp_path)
        gov._policy = _create_default_policy(str(tmp_path))
        gov._sandbox_available = False
        gov._sandbox_capability = SandboxCapability(
            supported=False,
            mode=None,
            reason="test: sandbox disabled",
        )
        decision = gov.assert_policy(_tc("Bash", "echo hello"))
        assert decision.action is GovernanceAction.DENY
        assert "SANDBOX_UNAVAILABLE" in decision.reason


def test_user_allow_outside_root_is_kept(tmp_path):
    """An explicit user ALLOW is not a hard boundary — only fallback is."""
    desktop = Path.home() / "Desktop" / "note.txt"
    decision = refine_file_write_decision(
        GovernanceDecision(
            action=GovernanceAction.ALLOW,
            reason="user granted desktop",
            source="user_rules",
        ),
        _tc("Write", str(desktop)),
        workspace_dir=tmp_path,
    )
    assert decision.action is GovernanceAction.ALLOW


def test_glob_is_not_treated_as_a_write(tmp_path):
    decision = refine_file_write_decision(
        GovernanceDecision(
            action=GovernanceAction.ALLOW,
            reason="search",
            source="user_rules",
        ),
        _tc("Glob", "/usr/local/**"),
        workspace_dir=tmp_path,
    )
    assert decision.action is GovernanceAction.ALLOW
    assert is_file_write_tool("Glob") is False
    assert is_file_write_tool("Grep") is False


def test_plugin_file_write_uses_same_boundary(tmp_path):
    DEFAULT_REGISTRY.register("PluginWriteBound", "file", "file_path")
    try:
        assert is_file_write_tool("PluginWriteBound") is True
        decision = refine_file_write_decision(
            GovernanceDecision(
                action=GovernanceAction.ALLOW,
                reason="fallback",
                source="fallback",
            ),
            _tc("PluginWriteBound", "/usr/local/potato-plugin.txt"),
            workspace_dir=tmp_path,
        )
        assert decision.action is GovernanceAction.ASK
        assert decision.source == "write_boundary"
    finally:
        DEFAULT_REGISTRY._types.pop("PluginWriteBound", None)
        DEFAULT_REGISTRY._target_params.pop("PluginWriteBound", None)


def test_assert_inside_writable_roots_rejects_desktop(tmp_path):
    desktop = str(Path.home() / "Desktop" / "note.txt")
    allowed, err = assert_inside_writable_roots(
        desktop,
        workspace_dir=tmp_path,
    )
    assert allowed is None
    assert err is not None


def test_assert_inside_writable_roots_allows_workspace(tmp_path):
    target = str(tmp_path / "browser" / "page.png")
    allowed, err = assert_inside_writable_roots(
        target,
        workspace_dir=tmp_path,
    )
    assert err is None
    assert allowed is not None


def test_extra_writable_cannot_open_git_hooks(tmp_path):
    hooks = tmp_path / ".git" / "hooks"
    hooks.mkdir(parents=True)
    allowed, err = validate_extra_writable(
        str(hooks),
        workspace_dir=tmp_path,
    )
    assert allowed is None
    assert err is not None


def test_global_rules_dir_is_hard_denied(tmp_path, monkeypatch):
    import potato.governance.global_rules as module

    rules = tmp_path / "user-gov" / "default.rules.yaml"
    monkeypatch.setattr(module, "global_rules_path", lambda: rules)
    denied = extra_denied_roots(credential_paths=())
    assert canonical_path(rules.parent) in denied


def test_policy_dir_is_hard_denied(tmp_path):
    policy_dir = tmp_path / "governance" / "ws_abc"
    policy_dir.mkdir(parents=True)
    denied = extra_denied_roots(policy_dir=policy_dir, credential_paths=())
    roots = default_writable_roots(tmp_path)
    assert (
        classify_write_target(str(policy_dir / "policy.yaml"), roots, denied)
        == "denied"
    )
