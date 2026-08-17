# -*- coding: utf-8 -*-
from __future__ import annotations

from unittest.mock import patch

from potato.governance.policy import (
    GovernanceAction,
    GovernancePolicy,
    ToolCallSpec,
    _auto_default_user_rules,
)
from potato.governance.tool_registry import (
    ALLOWED_TOOL_TYPES,
    ToolRegistry,
    validate_tool_type,
)


def _policy() -> GovernancePolicy:
    registry = ToolRegistry()
    registry.register("ComputerClick", "computer", "app")
    registry.register("ComputerObserve", "computer", "app")
    return GovernancePolicy(
        builtin_rules=[],
        user_rules=[],
        execution_level="smart",
        _registry=registry,
    )


def test_click_defaults_to_ask_observe_defaults_to_allow() -> None:
    by_match = {rule.match: rule for rule in _auto_default_user_rules()}
    assert by_match["ComputerClick(**)"].action is GovernanceAction.ASK
    assert by_match["ComputerObserve(**)"].action is GovernanceAction.ALLOW
    assert by_match["ComputerListApps(**)"].action is GovernanceAction.ALLOW


def test_computer_is_a_registered_tool_type() -> None:
    assert "computer" in ALLOWED_TOOL_TYPES
    assert validate_tool_type("computer") == "computer"


def test_always_allowed_app_skips_ask_outside_strict() -> None:
    policy = _policy()
    spec = ToolCallSpec(
        tool_name="ComputerClick",
        target="com.apple.calculator",
        agent_id="a",
        session_id="s",
        raw_params={"app": "com.apple.calculator"},
    )
    with patch(
        "potato.computer_use.settings.is_app_always_allowed",
        return_value=True,
    ):
        decision = policy.evaluate(spec)
    assert decision.action is GovernanceAction.ALLOW
    assert decision.source == "computer_use.always_allowed_apps"


def test_protected_app_is_denied_before_always_allow() -> None:
    policy = _policy()
    spec = ToolCallSpec(
        tool_name="ComputerClick",
        target="io.agentscope.qwenpaw.desktop",
        agent_id="a",
        session_id="s",
        raw_params={"app": "io.agentscope.qwenpaw.desktop"},
    )
    with patch(
        "potato.computer_use.settings.is_app_always_allowed",
        return_value=True,
    ):
        decision = policy.evaluate(spec)
    assert decision.action is GovernanceAction.DENY
    assert decision.source == "computer_use.protected_apps"


def test_extract_target_uses_observation_not_claimed_app() -> None:
    from potato.computer_use.session import Observation, observation_store
    from potato.governance.tool_registry import ToolRegistry

    store = observation_store()
    store.clear()
    observation = store.put(
        Observation(
            observation_id="obs_lease",
            app="Calculator",
            bundle_id="com.apple.calculator",
            pid=42,
            window_id=7,
            snapshot_id="snap-1",
        ),
    )
    registry = ToolRegistry()
    registry.register("ComputerClick", "computer", "app")
    try:
        target = registry.extract_target(
            "ComputerClick",
            {
                "observation_id": observation.observation_id,
                "app": "com.apple.mail",
            },
        )
        assert target == "com.apple.calculator"
    finally:
        store.clear()


def test_stale_observation_is_denied_without_using_claimed_app() -> None:
    from potato.computer_use.protect import INVALID_COMPUTER_TARGET
    from potato.governance.tool_registry import ToolRegistry

    registry = ToolRegistry()
    registry.register("ComputerClick", "computer", "app")
    target = registry.extract_target(
        "ComputerClick",
        {"observation_id": "obs_missing", "app": "com.apple.calculator"},
    )
    assert target == INVALID_COMPUTER_TARGET

    policy = GovernancePolicy(
        builtin_rules=[],
        user_rules=[],
        execution_level="smart",
        _registry=registry,
    )
    decision = policy.evaluate(
        ToolCallSpec(
            tool_name="ComputerClick",
            target=target,
            agent_id="a",
            session_id="s",
            raw_params={
                "observation_id": "obs_missing",
                "app": "com.apple.calculator",
            },
        ),
    )
    assert decision.action is GovernanceAction.DENY
    assert decision.source == "computer_use.invalid_observation"


def test_strict_still_asks_always_allowed_app() -> None:
    policy = _policy()
    policy.execution_level = "strict"
    spec = ToolCallSpec(
        tool_name="ComputerClick",
        target="com.apple.calculator",
        agent_id="a",
        session_id="s",
        raw_params={"app": "com.apple.calculator"},
    )
    with patch(
        "potato.computer_use.settings.is_app_always_allowed",
        return_value=True,
    ):
        decision = policy.evaluate(spec)
    assert decision.action is GovernanceAction.ASK
