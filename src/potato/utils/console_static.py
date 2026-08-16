# -*- coding: utf-8 -*-
"""Resolve default web UI assets (shared by app and CLI)."""
from __future__ import annotations

import os
from pathlib import Path

from ..constant import EnvVarLoader

# ``POTATO_WEB_STATIC_DIR`` selects an alternate build of the default web UI.
# ``POTATO_CONSOLE_STATIC_DIR`` remains a deliberate escape hatch for the
# legacy console while the old UI is still supported for recovery scenarios.
WEB_STATIC_ENV = "POTATO_WEB_STATIC_DIR"
CONSOLE_STATIC_ENV = "POTATO_CONSOLE_STATIC_DIR"


def _default_static_candidates(
    *,
    package_dir: Path,
    repo_dir: Path,
    cwd: Path,
) -> tuple[Path, ...]:
    """Return default UI locations in their intended precedence order.

    ``potato/console`` is the historical package-data directory and now
    contains the built ``app`` artifact in wheels and containers.  A source
    checkout prefers ``app/dist`` so local development cannot accidentally
    serve a stale bundled artifact from a previous package build.  Legacy
    console locations are intentionally omitted; selecting them requires the
    explicit :data:`CONSOLE_STATIC_ENV` override.
    """
    return (
        repo_dir / "app" / "dist",
        package_dir / "console",
        cwd / "app" / "dist",
    )


def resolve_web_static_dir() -> str:
    """Return the directory expected to contain the default web UI.

    Resolution order is: canonical web override, explicit legacy-console
    override, source ``app/dist``, packaged web artifact, then the current
    directory's ``app/dist``.  The legacy console is selected only when an
    operator explicitly sets :data:`CONSOLE_STATIC_ENV`.
    """
    static_dir = EnvVarLoader.get_str(WEB_STATIC_ENV)
    if static_dir:
        return static_dir

    static_dir = EnvVarLoader.get_str(CONSOLE_STATIC_ENV)
    if static_dir:
        return static_dir

    pkg_dir = Path(__file__).resolve().parent.parent
    repo_dir = pkg_dir.parent.parent
    cwd = Path(os.getcwd())
    for candidate in _default_static_candidates(
        package_dir=pkg_dir,
        repo_dir=repo_dir,
        cwd=cwd,
    ):
        if candidate.is_dir() and (candidate / "index.html").is_file():
            return str(candidate)

    return str(cwd / "app" / "dist")


def resolve_console_static_dir() -> str:
    """Backward-compatible alias for :func:`resolve_web_static_dir`."""
    return resolve_web_static_dir()


def find_potato_source_repo_root() -> Path | None:
    """Return the git checkout root if this Python
    is running from Potato source.

    Looks upward from :mod:`potato` for ``app/package.json``,
    ``app/package-lock.json``, and ``src/potato/``
    (bundled static target).
    Returns ``None`` for a normal pip/wheel install.
    """
    try:
        import potato  # noqa: PLC0415 — avoid import cycle at module load
    except Exception:  # pylint: disable=broad-exception-caught
        return None
    cur = Path(potato.__file__).resolve().parent
    for _ in range(20):
        web_app = cur / "app"
        if (
            (web_app / "package.json").is_file()
            and (web_app / "package-lock.json").is_file()
            and (cur / "src" / "potato").is_dir()
        ):
            return cur
        if cur.parent == cur:
            break
        cur = cur.parent
    return None
