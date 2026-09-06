# -*- coding: utf-8 -*-
"""App identity for Computer Use leases. Never trust a free-text app label."""

from __future__ import annotations

import os
import re
from typing import Any

from .constants import APPROVAL_HOLD_SLACK_SECONDS
from .errors import ComputerUseError
from .session import Observation, observation_store

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
    """Accept the same spellings computer_observe accepted.

    Observe resolves "Calculator" to com.apple.calculator even when the
    localized display name is "计算器", so the action's app claim must
    match by substring too, not only by exact bundle id or name.
    """
    key = (claimed or "").strip().lower()
    if not key:
        return True
    bundle = (bundle_id or "").strip().lower()
    name = (app_name or "").strip().lower()
    return key in {bundle, name} or key in bundle or key in name


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


def hold_observation_for_approval(input_data: dict, seconds: float) -> None:
    """Keep the call's observation alive while a human decides.

    Only the approval path calls this, so an allow/deny decision that
    never waits cannot stretch the 120 s freshness window.
    """
    obs_id = str(input_data.get("observation_id") or "").strip()
    if obs_id:
        observation_store().hold_for_approval(
            obs_id,
            float(seconds) + APPROVAL_HOLD_SLACK_SECONDS,
        )


def release_observation_hold(input_data: dict) -> None:
    """Undo the approval extension after a deny, timeout, or cancel."""
    obs_id = str(input_data.get("observation_id") or "").strip()
    if obs_id:
        observation_store().release_hold(obs_id)


def _short(value: Any, limit: int = 80) -> str:
    text = str(value if value is not None else "").replace("\n", "\\n")
    return text if len(text) <= limit else text[: limit - 1] + "…"


def describe_computer_action(tool_name: str, input_data: dict) -> str:
    """Human-readable line for the approval card: what will be done to what.

    ``exact_target`` stays the bundle id (it feeds rule creation); this is
    display only. Returns an empty string when there is nothing useful.
    """
    action = (tool_name or "").strip().removeprefix("Computer")
    action = re.sub(r"(?<!^)(?=[A-Z])", " ", action).strip().lower() or "action"
    parts: list[str] = [action]

    element_desc = ""
    obs_id = str(input_data.get("observation_id") or "").strip()
    index = input_data.get("element_index")
    has_xy = input_data.get("x") is not None and input_data.get("y") is not None
    # computer_click() prefers x/y whenever both are present and ignores
    # element_index; the card must describe the same target.
    if action == "click" and has_xy:
        index = None
    if obs_id and index is not None:
        try:
            observation: Observation = observation_store().get(obs_id)
            element = observation.element(int(index))
        except (ComputerUseError, TypeError, ValueError):
            element = None
        if element:
            role = _short(element.get("role") or "", 24)
            label = _short(element.get("label") or "", 60)
            value = _short(element.get("value") or "", 40)
            desc = " ".join(
                item for item in (role, f'"{label}"' if label else "") if item
            )
            if value and not label:
                desc = f"{desc} [{value}]".strip()
            element_desc = desc
        else:
            element_desc = f"element #{index} (no longer in observation)"
    if element_desc:
        parts.append(element_desc)
    elif has_xy:
        parts.append(f"at ({input_data.get('x')}, {input_data.get('y')})")
    elif all(
        input_data.get(k) is not None for k in ("from_x", "from_y", "to_x", "to_y")
    ):
        parts.append(
            f"from ({input_data.get('from_x')}, {input_data.get('from_y')}) "
            f"to ({input_data.get('to_x')}, {input_data.get('to_y')})",
        )

    for key, prefix in (
        ("text", "text: "),
        ("value", "value: "),
        ("key", "key: "),
        ("direction", "direction: "),
        ("button", "button: "),
    ):
        raw = input_data.get(key)
        if raw not in (None, ""):
            if key == "button" and str(raw) == "left":
                continue
            parts.append(
                prefix + repr(_short(raw))
                if key in ("text", "value")
                else prefix + _short(raw)
            )
    if input_data.get("amount") not in (None, "", 3):
        parts.append(f"amount: {input_data.get('amount')}")
    return " · ".join(parts)


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
