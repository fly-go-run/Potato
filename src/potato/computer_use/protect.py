# -*- coding: utf-8 -*-
"""App identity for Computer Use leases. Never trust a free-text app label."""

from __future__ import annotations

import os

from .errors import ComputerUseError
from .session import observation_store

INVALID_COMPUTER_TARGET = "__potato_invalid_observation__"

# Clicking these can bypass Potato's own approval or the sandbox.
PROTECTED_BUNDLE_IDS = frozenset(
    {
        "io.agentscope.qwenpaw.desktop",
        "com.apple.Terminal",
        "com.googlecode.iterm2",
        "com.github.wez.wezterm",
        "org.alacritty",
        "com.apple.systempreferences",
        "com.apple.Preferences",
        "com.microsoft.windows.terminal",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe",
    },
)

PROTECTED_NAMES = frozenset(
    {
        "terminal",
        "iterm",
        "iterm2",
        "windows terminal",
        "windowsterminal",
        "alacritty",
        "wezterm",
        "system settings",
        "system preferences",
        "potato",
        "qwenpaw",
    },
)


def same_app(claimed: str, bundle_id: str, app_name: str) -> bool:
    key = (claimed or "").strip().lower()
    if not key:
        return True
    return key in {
        (bundle_id or "").strip().lower(),
        (app_name or "").strip().lower(),
    }


def is_protected_app(*, bundle_id: str = "", app_name: str = "", pid: int = 0) -> bool:
    if pid and pid == os.getpid():
        return True
    bundle = (bundle_id or "").strip().lower()
    if bundle and bundle in {item.lower() for item in PROTECTED_BUNDLE_IDS}:
        return True
    name = (app_name or "").strip().lower()
    if name in PROTECTED_NAMES:
        return True
    return "potato" in name or "qwenpaw" in name


def policy_target_for_computer(input_data: dict) -> str:
    """Lease identity: observation bundle if present, else the observe ``app``."""
    obs_id = str(input_data.get("observation_id") or "").strip()
    claimed = str(input_data.get("app") or "").strip()
    if not obs_id:
        return claimed
    try:
        observation = observation_store().get(obs_id)
    except ComputerUseError:
        return INVALID_COMPUTER_TARGET
    return observation.bundle_id or observation.app


def live_observation_bundle_id(input_data: dict) -> str:
    """Return the live observation's stable app id, or an empty string."""
    obs_id = str(input_data.get("observation_id") or "").strip()
    if not obs_id:
        return ""
    try:
        observation = observation_store().get(obs_id)
    except ComputerUseError:
        return ""
    return observation.bundle_id.strip()


def assert_observation_matches_claim(observation, claimed_app: str) -> None:
    if claimed_app and not same_app(
        claimed_app,
        observation.bundle_id,
        observation.app,
    ):
        raise ComputerUseError(
            "APP_MISMATCH",
            "app does not match this observation. "
            "Use the bundle_id returned by computer_observe.",
        )
    if is_protected_app(
        bundle_id=observation.bundle_id,
        app_name=observation.app,
        pid=observation.pid,
    ):
        raise ComputerUseError(
            "APP_PROTECTED",
            "Computer Use cannot operate Potato, Terminal, or System Settings.",
        )
