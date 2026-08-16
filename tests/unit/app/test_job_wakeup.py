# -*- coding: utf-8 -*-
"""Idle followup vs busy inject for job completion."""
from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

from potato.app.job_wakeup import attach_job_wakeup
from potato.runtime.jobs import (
    JobHooks,
    JobOutcome,
    JobRegistry,
    JobStart,
    set_job_registry,
)
from potato.tool_calls import ToolCoordinator


async def test_idle_owner_is_woken_once():
    registry = JobRegistry()
    set_job_registry(registry)
    coordinator = ToolCoordinator()
    tracker = MagicMock()
    tracker.get_status = AsyncMock(return_value="idle")
    tracker.attach_or_start = AsyncMock(return_value=(MagicMock(), True))
    chat_mgr = MagicMock()
    chat_mgr.get_chat_id_by_session = AsyncMock(return_value="chat-1")
    chat_mgr.get_chat = AsyncMock(return_value=MagicMock(user_id="u1"))
    console = MagicMock()
    console.stream_one = AsyncMock()
    channels = MagicMock()
    channels.get_channel = AsyncMock(return_value=console)
    workspace = MagicMock(
        chat_manager=chat_mgr,
        task_tracker=tracker,
        channel_manager=channels,
    )
    attach_job_wakeup(
        tool_coordinator=coordinator,
        get_workspace=lambda _aid: workspace,
    )

    done = __import__("asyncio").get_running_loop().create_future()

    def run() -> JobHooks:
        return JobHooks(cancel=lambda: None, done=done, read_output=lambda: "")

    job_id = await registry.start(
        JobStart(
            kind="bash",
            label="echo",
            run=run,
            owner_session="sess-1",
            owner_agent="default",
            owner_user="u1",
        ),
    )
    done.set_result(JobOutcome(status="completed", detail="exit code: 0"))
    await __import__("asyncio").sleep(0)
    await __import__("asyncio").sleep(0)
    hints = await coordinator.pop_pending_hints("sess-1")
    assert hints
    tracker.attach_or_start.assert_awaited()
    assert job_id
    set_job_registry(None)


async def test_busy_owner_is_not_woken():
    registry = JobRegistry()
    set_job_registry(registry)
    coordinator = ToolCoordinator()
    tracker = MagicMock()
    tracker.get_status = AsyncMock(return_value="running")
    tracker.attach_or_start = AsyncMock()
    chat_mgr = MagicMock()
    chat_mgr.get_chat_id_by_session = AsyncMock(return_value="chat-1")
    workspace = MagicMock(
        chat_manager=chat_mgr,
        task_tracker=tracker,
        channel_manager=MagicMock(),
    )
    attach_job_wakeup(
        tool_coordinator=coordinator,
        get_workspace=lambda _aid: workspace,
    )
    done = __import__("asyncio").get_running_loop().create_future()

    def run() -> JobHooks:
        return JobHooks(cancel=lambda: None, done=done, read_output=lambda: "")

    await registry.start(
        JobStart(
            kind="bash",
            label="echo",
            run=run,
            owner_session="sess-2",
            owner_agent="default",
        ),
    )
    done.set_result(JobOutcome(status="completed", detail="exit code: 0"))
    await __import__("asyncio").sleep(0)
    await __import__("asyncio").sleep(0)
    tracker.attach_or_start.assert_not_called()
    hints = await coordinator.pop_pending_hints("sess-2")
    assert hints
    set_job_registry(None)
