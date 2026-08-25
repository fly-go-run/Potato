# -*- coding: utf-8 -*-
"""User-global approval rules shared across workspaces.

Shell prefixes such as ``Bash(git *)`` are portable. Workspace-absolute
file rules stay local because they would not match another project.
"""
from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

import yaml

from ..constant import WORKING_DIR
from ..utils.io_utils import write_yaml_atomic
from .policy import GovernanceRule, _parse_match, _parse_rules
from .write_boundary import is_file_write_tool

logger = logging.getLogger(__name__)

_GLOBAL_RULES_PATH = WORKING_DIR / "governance" / "default.rules.yaml"


def global_rules_path() -> Path:
    return _GLOBAL_RULES_PATH


def load_global_user_rules() -> list[GovernanceRule]:
    path = global_rules_path()
    if not path.is_file():
        return []
    try:
        with open(path, encoding="utf-8") as handle:
            data = yaml.safe_load(handle)
    except Exception:
        logger.warning("failed to read global approval rules", exc_info=True)
        return []
    if not isinstance(data, dict):
        return []
    return _parse_rules(data.get("user_rules"))


def is_portable_rule(rule: GovernanceRule) -> bool:
    """True when the match is useful outside the approving workspace."""
    try:
        tool, pattern = _parse_match(rule.match)
    except (ValueError, IndexError):
        return False
    if is_file_write_tool(tool):
        return False
    if pattern.startswith("/") or ":\\" in pattern or pattern.startswith("~"):
        return False
    return True


def append_global_user_rule(rule: GovernanceRule) -> None:
    """Persist one portable ALLOW rule for every future workspace."""
    if rule.duration != "permanent" or not is_portable_rule(rule):
        return
    existing = load_global_user_rules()
    if any(
        item.match == rule.match and item.action == rule.action
        for item in existing
    ):
        return
    existing.insert(0, rule)
    payload: dict[str, Any] = {
        "user_rules": [
            {
                "match": item.match,
                "action": item.action.value,
                "reason": item.reason,
            }
            for item in existing
        ],
    }
    try:
        global_rules_path().parent.mkdir(parents=True, exist_ok=True)
        write_yaml_atomic(
            global_rules_path(),
            payload,
            default_flow_style=False,
            allow_unicode=True,
            sort_keys=False,
        )
    except Exception:
        logger.warning("failed to persist global approval rule", exc_info=True)
