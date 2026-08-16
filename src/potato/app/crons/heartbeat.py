# -*- coding: utf-8 -*-
"""
Heartbeat: run agent with HEARTBEAT.md as query at interval.
Uses config functions (get_heartbeat_config, get_heartbeat_query_path,
load_config) for paths and settings.
"""
from __future__ import annotations

import asyncio
import logging
import re
from datetime import datetime, time, timezone
from pathlib import Path
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError
from typing import Any, Dict, Optional

from ...agents.utils.file_handling import read_text_file_with_encoding_fallback
from ...config import (
    get_heartbeat_config,
    get_heartbeat_query_path,
    load_config,
)
from ...constant import (
    HEARTBEAT_FILE,
    HEARTBEAT_TARGET_LAST,
)
from ..channels.schema import DEFAULT_CHANNEL
from ..crons.models import _crontab_dow_to_name

logger = logging.getLogger(__name__)

# Pattern for "30m", "1h", "2h30m", "90s"
_EVERY_PATTERN = re.compile(
    r"^(?:(?P<hours>\d+)h)?(?:(?P<minutes>\d+)m)?(?:(?P<seconds>\d+)s)?$",
    re.IGNORECASE,
)

# 5-field cron: minute hour day month day_of_week
_CRON_FIELD_PATTERN = re.compile(
    r"^[\d\*\-/,]+$",
)
# DOW field accepts named abbreviations (mon-sun) per
# POSIX/APScheduler.  Supports: numeric cron chars, named
# abbreviations, or named ranges like mon-fri.
_DOW_NAMED = "(?:mon|tue|wed|thu|fri|sat|sun)"
_DOW_FIELD_PATTERN = re.compile(
    r"^(?:[\d\*\-/,]+|" + _DOW_NAMED + r"(?:-" + _DOW_NAMED + r")?(/[\d]+)?$)",
    re.IGNORECASE,
)


def is_cron_expression(every: str) -> bool:
    """Return True if *every* looks like a 5-field cron expression.

    The first four fields (minute, hour, day, month) accept only numeric
    cron characters.  The fifth field (day-of-week) additionally accepts
    the three-letter English abbreviations *mon*–*sun* which are valid in
    both POSIX cron and APScheduler's ``CronTrigger``.
    """
    parts = (every or "").strip().split()
    if len(parts) != 5:
        return False
    # First 4 fields: minute, hour, day, month — numeric only
    if not all(_CRON_FIELD_PATTERN.match(p) for p in parts[:4]):
        return False
    # 5th field: day-of-week — numeric OR named abbreviations
    return bool(_DOW_FIELD_PATTERN.match(parts[4]))


def parse_heartbeat_cron(every: str) -> tuple:
    """Parse and normalize a 5-field cron string.

    Returns (minute, hour, day, month, dow).
    """
    parts = every.strip().split()
    if len(parts) == 5:
        parts[4] = _crontab_dow_to_name(parts[4])
    return tuple(parts)


def parse_heartbeat_every(every: str) -> int:
    """Parse interval string (e.g. '30m', '1h') to total seconds.

    Note: cron expressions should be detected via ``is_cron_expression``
    *before* calling this function.
    """
    every = (every or "").strip()
    if not every:
        return 30 * 60  # default 30 min
    m = _EVERY_PATTERN.match(every)
    if not m:
        logger.warning("heartbeat every=%r invalid, using 30m", every)
        return 30 * 60
    hours = int(m.group("hours") or 0)
    minutes = int(m.group("minutes") or 0)
    seconds = int(m.group("seconds") or 0)
    total = hours * 3600 + minutes * 60 + seconds
    if total <= 0:
        return 30 * 60
    return total


def _in_active_hours(active_hours: Any) -> bool:
    """Return True if the current time in user timezone is within
    [start, end].
    """
    if (
        not active_hours
        or not hasattr(active_hours, "start")
        or not hasattr(active_hours, "end")
    ):
        return True
    try:
        start_parts = active_hours.start.strip().split(":")
        end_parts = active_hours.end.strip().split(":")
        start_t = time(
            int(start_parts[0]),
            int(start_parts[1]) if len(start_parts) > 1 else 0,
        )
        end_t = time(
            int(end_parts[0]),
            int(end_parts[1]) if len(end_parts) > 1 else 0,
        )
    except (ValueError, IndexError, AttributeError):
        return True
    user_tz = load_config().user_timezone or "UTC"
    try:
        now = datetime.now(ZoneInfo(user_tz)).time()
    except (ZoneInfoNotFoundError, KeyError):
        logger.warning(
            "Invalid timezone %r in config, falling back to UTC"
            " for heartbeat active hours check.",
            user_tz,
        )
        now = datetime.now(timezone.utc).time()
    if start_t <= end_t:
        return start_t <= now <= end_t
    return now >= start_t or now <= end_t


# pylint: disable=too-many-branches,too-many-statements
async def run_heartbeat_once(
    *,
    workspace: Any,
    channel_manager: Any,
    agent_id: Optional[str] = None,
    workspace_dir: Optional[Path] = None,
) -> None:
    """Run one heartbeat: read HEARTBEAT.md, run agent, optionally
    dispatch to last channel (target=last).
    """
    from ...config.config import load_agent_config

    hb = get_heartbeat_config(agent_id)
    if not _in_active_hours(hb.active_hours):
        logger.debug("heartbeat skipped: outside active hours")
        return
    timeout_seconds = hb.timeout_seconds

    # Use workspace_dir if provided, otherwise fall back to global path
    if workspace_dir:
        path = Path(workspace_dir) / HEARTBEAT_FILE
    else:
        path = get_heartbeat_query_path()

    if not path.is_file():
        logger.debug("heartbeat skipped: no file at %s", path)
        return

    query_text = read_text_file_with_encoding_fallback(path).strip()
    if not query_text:
        logger.debug("heartbeat skipped: empty query file")
        return

    # Build request: single user message with query text
    req: Dict[str, Any] = {
        "input": [
            {
                "role": "user",
                "content": [{"type": "text", "text": query_text}],
            },
        ],
        "session_id": "main",
        "user_id": "main",
        "channel": DEFAULT_CHANNEL,
        "request_context": {"source": "heartbeat"},
    }

    # Get last_dispatch from agent config if agent_id provided
    last_dispatch = None
    if agent_id:
        try:
            agent_config = load_agent_config(agent_id)
            last_dispatch = agent_config.last_dispatch
        except Exception:
            pass
    else:
        # Legacy: try root config
        config = load_config()
        last_dispatch = config.last_dispatch

    target = (hb.target or "").strip().lower()
    if target == HEARTBEAT_TARGET_LAST and last_dispatch:
        ld = last_dispatch
        if ld.channel and (ld.user_id or ld.session_id):

            async def _run_and_dispatch() -> None:
                async for event in workspace.stream_query(req):
                    await channel_manager.send_event(
                        channel=ld.channel,
                        user_id=ld.user_id,
                        session_id=ld.session_id,
                        event=event,
                        meta={},
                    )

            try:
                await asyncio.wait_for(
                    _run_and_dispatch(),
                    timeout=timeout_seconds,
                )
            except asyncio.TimeoutError:
                logger.warning(
                    "heartbeat run timed out after %ss",
                    timeout_seconds,
                )
            return

    # target main or no last_dispatch: run agent only, no dispatch
    async def _run_without_dispatch() -> None:
        async for _ in workspace.stream_query(req):
            pass

    try:
        await asyncio.wait_for(
            _run_without_dispatch(),
            timeout=timeout_seconds,
        )
    except asyncio.TimeoutError:
        logger.warning(
            "heartbeat run timed out after %ss",
            timeout_seconds,
        )
