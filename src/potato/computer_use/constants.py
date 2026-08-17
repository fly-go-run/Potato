# -*- coding: utf-8 -*-
"""Stable names for the Potato computer-use facade."""

from __future__ import annotations

# Model-facing tools. Keep this list tiny on purpose.
COMPUTER_USE_TOOL_NAMES: frozenset[str] = frozenset(
    {
        "computer_list_apps",
        "computer_observe",
        "computer_click",
        "computer_set_value",
        "computer_type_text",
        "computer_press_key",
        "computer_scroll",
        "computer_drag",
    },
)

# Tools that change a target app. Observe/list stay read-only.
CONTROL_TOOL_NAMES: frozenset[str] = frozenset(
    {
        "computer_click",
        "computer_set_value",
        "computer_type_text",
        "computer_press_key",
        "computer_scroll",
        "computer_drag",
    },
)

# Cua Driver MCP tools this facade is allowed to invoke.
ALLOWED_DRIVER_TOOLS: frozenset[str] = frozenset(
    {
        "list_apps",
        "list_windows",
        "get_window_state",
        "click",
        "set_value",
        "type_text",
        "press_key",
        "scroll",
        "drag",
    },
)

# Tools whose Cua schema accepts delivery_mode. set_value is AX-only.
INPUT_DRIVER_TOOLS: frozenset[str] = frozenset(
    {
        "click",
        "type_text",
        "press_key",
        "scroll",
        "drag",
    },
)

OBSERVATION_TTL_SECONDS = 120
DRIVER_CALL_TIMEOUT_SECONDS = 45.0
DAEMON_START_TIMEOUT_SECONDS = 12.0
