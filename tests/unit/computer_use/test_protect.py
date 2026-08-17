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
