# -*- coding: utf-8 -*-
"""Shared ACP metadata keys.

This module is intentionally lightweight so CLI code can import constants
without importing the ACP server implementation.
"""

ACP_CODING_PROJECT_META_KEY = "potato.coding_project_dir"
LEGACY_QWENPAW_CODING_PROJECT_META_KEY = "qwenpaw.coding_project_dir"
ACP_EPHEMERAL_META_KEY = "potato.ephemeral"
ACP_APPROVAL_EXPIRES_AT_META_KEY = "potato.approval_expires_at"


def get_coding_project_dir(metadata: dict | None):
    """Return a coding project path from Potato or QwenPaw metadata."""

    if not isinstance(metadata, dict):
        return None
    return metadata.get(ACP_CODING_PROJECT_META_KEY) or metadata.get(
        LEGACY_QWENPAW_CODING_PROJECT_META_KEY,
    )

__all__ = [
    "ACP_APPROVAL_EXPIRES_AT_META_KEY",
    "ACP_CODING_PROJECT_META_KEY",
    "ACP_EPHEMERAL_META_KEY",
    "LEGACY_QWENPAW_CODING_PROJECT_META_KEY",
    "get_coding_project_dir",
]
