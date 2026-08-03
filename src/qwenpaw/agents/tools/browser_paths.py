# -*- coding: utf-8 -*-
"""Pure browser executable, profile, and download-path helpers."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

TRUSTED_BROWSER_KEYWORDS = frozenset(
    {
        "chrome",
        "chromium",
        "edge",
        "firefox",
        "brave",
        "vivaldi",
        "opera",
        "360se",
        "yandex",
        "tor",
    },
)

_BROWSER_TYPE_ORDER = (
    "edge",
    "chromium",
    "chrome",
    "brave",
    "vivaldi",
    "opera",
    "firefox",
    "360se",
    "yandex",
    "tor",
)


def validate_browser_executable_path(executable_path: str) -> None:
    """Raise ValueError unless *executable_path* names an existing browser."""
    if not executable_path:
        return
    name = Path(executable_path).name.lower()
    if not any(keyword in name for keyword in TRUSTED_BROWSER_KEYWORDS):
        raise ValueError(
            f"executable_path rejected: '{Path(executable_path).name}' "
            "does not match any trusted browser name "
            f"(keywords: {', '.join(sorted(TRUSTED_BROWSER_KEYWORDS))})",
        )
    if not Path(executable_path).is_file():
        raise ValueError(
            f"executable_path rejected: '{executable_path}' does not exist",
        )


def browser_type_from_executable(executable_path: str) -> str:
    """Infer the stable browser-type suffix for an executable path."""
    name = Path(executable_path).name.lower() if executable_path else ""
    for browser_type in _BROWSER_TYPE_ORDER:
        if browser_type in name:
            return browser_type
    return ""


def resolve_browser_user_data_dir(
    workspace_dir: str,
    executable_path: str,
    *,
    explicit_executable_path: bool = False,
) -> str:
    """Return the compatible profile directory for a browser launch."""
    if not workspace_dir:
        return ""
    base = Path(workspace_dir) / "browser"
    if not explicit_executable_path:
        return str(base / "user_data")
    browser_type = browser_type_from_executable(executable_path)
    # Preserve the legacy fallback for an explicit but unclassified executable.
    # A type-specific profile is only safe once the executable name is known.
    if not browser_type:
        return str(base / "user_data")
    return str(base / f"user_data_{browser_type}")


def safe_download_filename(filename: Any, default: str = "download") -> str:
    """Return a cross-platform-safe basename for a browser download."""
    name = Path(str(filename or "")).name.strip()
    name = re.sub(r'[\\/:*?"<>|\x00-\x1f]+', "_", name)
    name = name.strip(" .")
    return name or default
