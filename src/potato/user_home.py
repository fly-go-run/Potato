# -*- coding: utf-8 -*-
"""User data home: new installs use ~/.potato; old ones migrate once.

After a successful copy, later upgrades only read ~/.potato.
If the copy fails, the process keeps using the legacy home so the
app still starts.
"""
from __future__ import annotations

import logging
import os
import shutil
from datetime import datetime, timezone
from pathlib import Path

logger = logging.getLogger(__name__)

MIGRATION_MARKER = ".migrated_from"

_SKIP_DIR_NAMES = frozenset(
    {
        "venv",
        "__pycache__",
        "local_models",
        "models",
        "node_modules",
        ".git",
        "bin",
    },
)
_SKIP_FILE_NAMES = frozenset(
    {
        "desktop.log",
        "desktop.log.1",
        "potato.log",
        "qwenpaw.log",
        "desktop_backend.pid",
        "desktop_port",
        ".potato_restore.lock",
        ".qwenpaw_restore.lock",
        ".telemetry_collected",
    },
)


def is_working_data_home(path: Path) -> bool:
    """True when *path* is a real data dir, not just ``.env``."""
    return (path / "config.json").is_file() or (path / "workspaces").is_dir()


def resolve_secret_dir(
    working_dir: Path,
    *,
    explicit: str = "",
) -> Path:
    """Secrets live next to the data home: ``~/.potato.secret`` etc."""
    if explicit.strip():
        return Path(explicit).expanduser().resolve()
    return Path(str(working_dir) + ".secret").resolve()


def _legacy_homes(base: Path) -> list[Path]:
    return [base / ".qwenpaw", base / ".copaw"]


def _has_migration_marker(potato: Path) -> bool:
    return (potato / MIGRATION_MARKER).is_file()


def _copy_tree(src: Path, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    for item in src.iterdir():
        name = item.name
        if name in _SKIP_DIR_NAMES or name in _SKIP_FILE_NAMES:
            continue
        if name == MIGRATION_MARKER:
            continue
        target = dest / name
        if item.is_dir():
            _copy_tree(item, target)
            continue
        if not item.is_file():
            continue
        if target.exists():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(item, target)


def _copy_secret_dir(src: Path, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    master = src / ".master_key"
    if master.is_file() and not (dest / ".master_key").exists():
        shutil.copy2(master, dest / ".master_key")
        try:
            os.chmod(dest / ".master_key", 0o600)
        except OSError:
            pass
    _copy_tree(src, dest)


def migrate_legacy_user_home(source: Path, dest: Path) -> bool:
    """Copy *source* into *dest*. Keep existing dest files (e.g. ``.env``)."""
    try:
        dest.mkdir(parents=True, exist_ok=True)
        _copy_tree(source, dest)
        secret_src = Path(str(source) + ".secret")
        secret_dest = Path(str(dest) + ".secret")
        if secret_src.is_dir():
            # Fernet needs the same master key. macOS may keep it in
            # Keychain; Windows usually has no usable OS vault, so the
            # copied ``.master_key`` file is what decrypts provider JSON.
            _copy_secret_dir(secret_src, secret_dest)
        marker = dest / MIGRATION_MARKER
        marker.write_text(
            "source={src}\nsecret_source={sec}\nmigrated_at={ts}\n".format(
                src=source,
                sec=secret_src if secret_src.is_dir() else "",
                ts=datetime.now(timezone.utc).isoformat(timespec="seconds"),
            ),
            encoding="utf-8",
        )
        logger.info("Migrated user data from %s to %s", source, dest)
        return True
    except Exception:
        logger.exception(
            "Failed to migrate user data from %s to %s",
            source,
            dest,
        )
        return False


def resolve_working_dir(
    *,
    home: Path | None = None,
    explicit: str = "",
    migrate: bool = True,
) -> Path:
    """Pick the user data home, migrating a legacy install once.

    New machines only ever see ``~/.potato``. An existing
    ``~/.qwenpaw`` / ``~/.copaw`` is copied into ``~/.potato`` on the
    first new-version launch; later launches prefer the new path.
    Copy failure keeps the legacy path so the app still starts.
    """
    if explicit.strip():
        return Path(explicit).expanduser().resolve()
    base = home if home is not None else Path.home()
    potato = base / ".potato"
    if _has_migration_marker(potato) or (
        is_working_data_home(potato)
        and not any(is_working_data_home(p) for p in _legacy_homes(base))
    ):
        return potato.resolve()

    for legacy in _legacy_homes(base):
        if not is_working_data_home(legacy):
            continue
        if migrate and migrate_legacy_user_home(legacy, potato):
            return potato.resolve()
        return legacy.resolve()
    return potato.resolve()
