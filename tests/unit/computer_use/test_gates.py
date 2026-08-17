# -*- coding: utf-8 -*-
from __future__ import annotations

from types import SimpleNamespace
from unittest.mock import patch

from potato.app.workspace.local_workspace import PotatoLocalWorkspace
from potato.computer_use.constants import COMPUTER_USE_TOOL_NAMES
from potato.runtime.tool_registry import ToolRegistry


def _workspace() -> PotatoLocalWorkspace:
    workspace = PotatoLocalWorkspace.__new__(PotatoLocalWorkspace)
    workspace._tool_registry = ToolRegistry()
    return workspace


def test_disabled_computer_use_denies_facade_tools() -> None:
    workspace = _workspace()
    cfg = SimpleNamespace(
        tools=SimpleNamespace(
            builtin_tools={
                name: SimpleNamespace(enabled=True)
                for name in COMPUTER_USE_TOOL_NAMES
            },
        ),
    )
    with patch(
        "potato.computer_use.settings.computer_use_enabled",
        return_value=False,
    ):
        _allowed, denied = workspace._resolve_config_gates(cfg)
    assert COMPUTER_USE_TOOL_NAMES <= denied


def test_enabled_computer_use_does_not_force_deny() -> None:
    workspace = _workspace()
    cfg = SimpleNamespace(
        tools=SimpleNamespace(
            builtin_tools={
                name: SimpleNamespace(enabled=False)
                for name in COMPUTER_USE_TOOL_NAMES
            },
        ),
    )
    with patch(
        "potato.computer_use.settings.computer_use_enabled",
        return_value=True,
    ):
        _allowed, denied = workspace._resolve_config_gates(cfg)
    assert denied.isdisjoint(COMPUTER_USE_TOOL_NAMES)
