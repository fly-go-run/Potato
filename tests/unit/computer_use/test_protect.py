# -*- coding: utf-8 -*-
from __future__ import annotations

import os

from potato.computer_use.protect import (
    INVALID_COMPUTER_TARGET,
    is_protected_app,
    policy_target_for_computer,
    same_app,
)
from potato.computer_use.session import Observation, observation_store


def test_same_app_matches_name_or_bundle() -> None:
    assert same_app("Calculator", "com.apple.calculator", "Calculator")
    assert same_app("com.apple.calculator", "com.apple.calculator", "Calculator")
    assert not same_app("Mail", "com.apple.calculator", "Calculator")
    # Localized display name: the English spelling observe accepted still
    # matches through the bundle id.
    assert same_app("Calculator", "com.apple.calculator", "计算器")
    assert not same_app("Mail", "com.apple.calculator", "计算器")


def test_protected_apps_include_potato_and_terminal() -> None:
    assert is_protected_app(bundle_id="io.agentscope.qwenpaw.desktop")
    assert is_protected_app(app_name="Terminal")
    assert is_protected_app(pid=os.getpid())
    assert not is_protected_app(bundle_id="com.apple.calculator", app_name="Calculator")


def test_policy_target_prefers_observation_bundle() -> None:
    store = observation_store()
    store.clear()
    store.put(
        Observation(
            observation_id="obs_x",
            app="Calculator",
            bundle_id="com.apple.calculator",
            pid=1,
            window_id=2,
            snapshot_id="s",
        ),
    )
    try:
        assert (
            policy_target_for_computer(
                {"observation_id": "obs_x", "app": "Mail"},
            )
            == "com.apple.calculator"
        )
        assert policy_target_for_computer({"app": "Mail"}) == "Mail"
        assert (
            policy_target_for_computer(
                {"observation_id": "obs_missing", "app": "Mail"},
            )
            == INVALID_COMPUTER_TARGET
        )
    finally:
        store.clear()


def test_policy_target_does_not_extend_observation() -> None:
    from potato.computer_use.constants import OBSERVATION_TTL_SECONDS

    store = observation_store()
    store.clear()
    obs = Observation(
        observation_id="obs_ro",
        app="Calculator",
        bundle_id="com.apple.calculator",
        pid=1,
        window_id=2,
        snapshot_id="s",
    )
    store.put(obs)
    try:
        policy_target_for_computer({"observation_id": "obs_ro"})
        assert obs.expired(now=obs.created_at + OBSERVATION_TTL_SECONDS + 1)
    finally:
        store.clear()


def test_hold_and_release_observation_for_approval() -> None:
    from potato.computer_use.constants import OBSERVATION_TTL_SECONDS
    from potato.computer_use.protect import (
        hold_observation_for_approval,
        release_observation_hold,
    )

    store = observation_store()
    store.clear()
    obs = Observation(
        observation_id="obs_h",
        app="Calculator",
        bundle_id="com.apple.calculator",
        pid=1,
        window_id=2,
        snapshot_id="s",
    )
    store.put(obs)
    try:
        later = obs.created_at + OBSERVATION_TTL_SECONDS + 1
        hold_observation_for_approval({"observation_id": "obs_h"}, 300.0)
        assert not obs.expired(now=later)
        assert not obs.expired(now=obs.created_at + 320.0)
        release_observation_hold({"observation_id": "obs_h"})
        assert obs.expired(now=later)
        # No observation id: both are no-ops.
        hold_observation_for_approval({}, 300.0)
        release_observation_hold({})
    finally:
        store.clear()


def test_describe_computer_action_names_element_and_input() -> None:
    from potato.computer_use.protect import describe_computer_action

    store = observation_store()
    store.clear()
    store.put(
        Observation(
            observation_id="obs_d",
            app="Calculator",
            bundle_id="com.apple.calculator",
            pid=1,
            window_id=2,
            snapshot_id="s",
            elements=[
                {
                    "element_index": 3,
                    "role": "button",
                    "label": "Delete",
                    "value": "",
                },
                {
                    "element_index": 4,
                    "role": "textfield",
                    "label": "",
                    "value": "old",
                },
            ],
        ),
    )
    try:
        line = describe_computer_action(
            "ComputerClick",
            {"observation_id": "obs_d", "element_index": 3, "button": "left"},
        )
        assert line == 'click · button "Delete"'
        line = describe_computer_action(
            "ComputerSetValue",
            {
                "observation_id": "obs_d",
                "element_index": 4,
                "value": "hi\nthere",
            },
        )
        assert line.startswith("set value · textfield [old] · value: ")
        assert "\\n" in line
        line = describe_computer_action(
            "ComputerTypeText",
            {"observation_id": "obs_d", "text": "x" * 200},
        )
        assert "…" in line and len(line) < 120
        line = describe_computer_action(
            "ComputerClick",
            {"observation_id": "obs_d", "x": 10, "y": 20, "button": "right"},
        )
        assert line == "click · at (10, 20) · button: right"
        # x/y win over element_index, exactly like computer_click().
        line = describe_computer_action(
            "ComputerClick",
            {"observation_id": "obs_d", "element_index": 3, "x": 900, "y": 700},
        )
        assert line == "click · at (900, 700)"
        line = describe_computer_action(
            "ComputerClick",
            {"observation_id": "obs_d", "element_index": 99},
        )
        assert "no longer in observation" in line
    finally:
        store.clear()
    # No observation at all still describes the input.
    assert describe_computer_action("ComputerPressKey", {"key": "return"}) == (
        "press key · key: return"
    )
