# -*- coding: utf-8 -*-
"""Fast desktop first-run bootstrap.

The sidecar used to run the full ``potato init --defaults`` before binding
its port. That copies skills and markdown and delays the first HTTP response
by a couple of seconds. Write a minimal config so Uvicorn can start, then
finish the rest after the server is accepting requests.
"""

from __future__ import annotations

import logging
from pathlib import Path

logger = logging.getLogger(__name__)


def ensure_minimal_desktop_config(working_dir: Path) -> bool:
    """Create an empty ``config.json`` when missing.

    Returns True when this process created the file so the caller can
    finish first-run defaults in the background.
    """
    config_path = working_dir / "config.json"
    if config_path.exists():
        return False
    working_dir.mkdir(parents=True, exist_ok=True)
    config_path.write_text("{}\n", encoding="utf-8")
    logger.info("Wrote minimal desktop config at %s", config_path)
    return True


def finish_desktop_defaults(working_dir: Path) -> None:
    """Copy default skills and markdown after the HTTP server is up."""
    try:
        from ..agents.utils import copy_md_files
        from ..cli.init_cmd import _sync_default_workspace_skills
        from ..config import load_config, save_config
        from ..constant import WORKING_DIR
    except Exception:
        logger.debug("desktop defaults imports failed", exc_info=True)
        return

    workspace = Path(working_dir or WORKING_DIR) / "workspaces" / "default"
    try:
        synced = _sync_default_workspace_skills(workspace)
        if synced:
            logger.info("Desktop first-run enabled %s default skill(s)", synced)
    except Exception:
        logger.warning("Desktop first-run skill sync failed", exc_info=True)

    try:
        config_path = working_dir / "config.json"
        config = load_config(config_path) if config_path.is_file() else None
        language = (config.agents.language if config else None) or "zh"
        copied = copy_md_files(
            language,
            skip_existing=True,
            workspace_dir=workspace,
        )
        if copied and config is not None:
            config.agents.installed_md_files_language = language
            save_config(config, config_path)
            logger.info("Desktop first-run copied md files: %s", ", ".join(copied))
    except Exception:
        logger.warning("Desktop first-run markdown copy failed", exc_info=True)
