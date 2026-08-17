# -*- coding: utf-8 -*-
"""Read computer-use config without importing the tool module."""

from __future__ import annotations

import re
from typing import Any


_BUNDLE_ID_RE = re.compile(
    r"^[A-Za-z0-9][A-Za-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*)+$",
)
_WINDOWS_AUMID_RE = re.compile(r"^[^!\\/\s]+![^!\\/\s]+$")


def is_stable_app_id(app: str) -> bool:
    """Return whether *app* is a bundle id or Windows AUMID."""
    value = (app or "").strip()
    return bool(
        _BUNDLE_ID_RE.fullmatch(value)
        or _WINDOWS_AUMID_RE.fullmatch(value)
    )


def get_computer_use_settings() -> Any:
    from ..config import load_config

    return load_config().computer_use


def computer_use_enabled() -> bool:
    try:
        return bool(get_computer_use_settings().enabled)
    except Exception:
        return False


def is_app_always_allowed(app: str) -> bool:
    """True when the stable application id is on the settings allow-list."""
    key = (app or "").strip().lower()
    if not is_stable_app_id(key):
        return False
    try:
        allowed = get_computer_use_settings().always_allowed_apps or []
    except Exception:
        return False
    return any(
        is_stable_app_id(str(item))
        and str(item).strip().lower() == key
        for item in allowed
    )
