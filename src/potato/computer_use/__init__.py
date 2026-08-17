# -*- coding: utf-8 -*-
"""Potato computer-use facade over Cua Driver.

The model sees a Codex-sized API. Cua Driver is the background engine.
Foreground escalation is never exposed.
"""

from .bundle import CUA_DRIVER_VERSION, ensure_driver_binary
from .constants import COMPUTER_USE_TOOL_NAMES, CONTROL_TOOL_NAMES
from .settings import (
    computer_use_enabled,
    get_computer_use_settings,
    is_app_always_allowed,
)

__all__ = [
    "COMPUTER_USE_TOOL_NAMES",
    "CONTROL_TOOL_NAMES",
    "CUA_DRIVER_VERSION",
    "computer_use_enabled",
    "ensure_driver_binary",
    "get_computer_use_settings",
    "is_app_always_allowed",
]
