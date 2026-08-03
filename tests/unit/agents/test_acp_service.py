# -*- coding: utf-8 -*-
"""ACP service cleanup regressions."""

from unittest.mock import MagicMock, patch

import psutil

from qwenpaw.agents.acp.service import _kill_process_tree


def test_kill_process_tree_ignores_permission_denied_children() -> None:
    """ACP shutdown still closes resources when a child is protected."""
    parent = MagicMock()
    child = MagicMock()
    parent.children.return_value = [child]
    child.kill.side_effect = psutil.AccessDenied(pid=42)

    with patch(
        "qwenpaw.agents.acp.service.psutil.Process",
        return_value=parent,
    ):
        _kill_process_tree(41)

    child.kill.assert_called_once()
    parent.kill.assert_called_once()


def test_kill_process_tree_ignores_permission_denied_inspection() -> None:
    """Process-tree inspection can fail under a different user account."""
    parent = MagicMock()
    parent.children.side_effect = PermissionError("operation not permitted")
    parent.kill.side_effect = PermissionError("operation not permitted")

    with patch(
        "qwenpaw.agents.acp.service.psutil.Process",
        return_value=parent,
    ):
        _kill_process_tree(41)

    parent.kill.assert_called_once()
