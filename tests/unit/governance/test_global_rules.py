# -*- coding: utf-8 -*-
"""User-global portable approval rules."""
from __future__ import annotations

from potato.governance.global_rules import (
    append_global_user_rule,
    is_portable_rule,
    load_global_user_rules,
)
from potato.governance.policy import (
    GovernanceAction,
    GovernanceRule,
    _create_default_policy,
    clear_global_rules_cache,
)
from potato.governance.tool_registry import DEFAULT_REGISTRY

from .test_policy import _tc


def test_shell_prefix_is_portable() -> None:
    rule = GovernanceRule(
        match="Bash(git *)",
        action=GovernanceAction.ALLOW,
        reason="user approved",
        duration="permanent",
    )
    assert is_portable_rule(rule) is True


def test_workspace_write_is_not_portable() -> None:
    rule = GovernanceRule(
        match="Write(/Users/me/proj/notes.txt)",
        action=GovernanceAction.ALLOW,
        reason="user approved",
        duration="permanent",
    )
    assert is_portable_rule(rule) is False


def test_plugin_file_write_is_not_portable() -> None:
    DEFAULT_REGISTRY.register("PluginWritePortable", "file", "file_path")
    try:
        rule = GovernanceRule(
            match="PluginWritePortable(out.txt)",
            action=GovernanceAction.ALLOW,
            reason="user approved",
            duration="permanent",
        )
        assert is_portable_rule(rule) is False
    finally:
        DEFAULT_REGISTRY._types.pop("PluginWritePortable", None)
        DEFAULT_REGISTRY._target_params.pop("PluginWritePortable", None)


def test_append_and_load_roundtrip(tmp_path, monkeypatch) -> None:
    import potato.governance.global_rules as module

    path = tmp_path / "default.rules.yaml"
    monkeypatch.setattr(module, "global_rules_path", lambda: path)
    clear_global_rules_cache()
    rule = GovernanceRule(
        match="Bash(git *)",
        action=GovernanceAction.ALLOW,
        reason="user approved",
        duration="permanent",
    )
    append_global_user_rule(rule)
    loaded = load_global_user_rules()
    assert any(item.match == "Bash(git *)" for item in loaded)
    append_global_user_rule(rule)
    assert len(load_global_user_rules()) == 1


def test_evaluate_uses_global_rules(tmp_path, monkeypatch) -> None:
    import potato.governance.policy as policy_mod

    rule = GovernanceRule(
        match="Bash(custom-cli *)",
        action=GovernanceAction.ALLOW,
        reason="global allow",
        duration="permanent",
    )
    monkeypatch.setattr(
        policy_mod,
        "_cached_global_user_rules",
        lambda: [rule],
    )
    policy = _create_default_policy(str(tmp_path), str(tmp_path))
    decision = policy.evaluate(_tc("Bash", "custom-cli status"))
    assert decision.action is GovernanceAction.ALLOW
    assert decision.reason == "global allow"


def test_local_deny_beats_global_allow(tmp_path, monkeypatch) -> None:
    import potato.governance.policy as policy_mod

    global_rule = GovernanceRule(
        match="Bash(git *)",
        action=GovernanceAction.ALLOW,
        reason="global allow",
        duration="permanent",
    )
    monkeypatch.setattr(
        policy_mod,
        "_cached_global_user_rules",
        lambda: [global_rule],
    )
    policy = _create_default_policy(str(tmp_path), str(tmp_path))
    policy.user_rules.insert(
        0,
        GovernanceRule(
            match="Bash(git push *)",
            action=GovernanceAction.DENY,
            reason="local deny push",
        ),
    )
    decision = policy.evaluate(_tc("Bash", "git push origin main"))
    assert decision.action is GovernanceAction.DENY
    assert decision.reason == "local deny push"
