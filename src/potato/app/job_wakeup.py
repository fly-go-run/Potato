# -*- coding: utf-8 -*-
# flake8: noqa: E501
"""Deliver background-job completion: inject if busy, followup if idle."""
from __future__ import annotations

import logging
from typing import Any, Callable

from agentscope.message import Msg, TextBlock

from ..runtime.jobs import JobSnapshot, get_job_registry
from ..tool_calls import ToolCoordinator

logger = logging.getLogger(__name__)

GetWorkspace = Callable[[str], Any]


def make_job_hint(snapshot: JobSnapshot) -> Msg:
    notice = (
        "<system-notification>\n"
        f"background job {snapshot.id} ({snapshot.kind}: {snapshot.label}) "
        f"finished [status: {snapshot.status}"
        f"{f' — {snapshot.detail}' if snapshot.detail else ''}]. "
        "Read its output with job_output.\n"
        "</system-notification>"
    )
    return Msg(
        name="system",
        role="assistant",
        content=[TextBlock(type="text", text=notice)],
    )


def completion_text(snapshot: JobSnapshot) -> str:
    return (
        f"background job {snapshot.id} ({snapshot.kind}: {snapshot.label}) "
        f"finished [status: {snapshot.status}"
        f"{f' — {snapshot.detail}' if snapshot.detail else ''}]. "
        "Read its output with job_output."
    )


def attach_job_wakeup(
    *,
    tool_coordinator: ToolCoordinator,
    get_workspace: GetWorkspace,
) -> None:
    """Register the default completion listener on the process job registry."""

    async def _on_done(snapshot: JobSnapshot) -> None:
        if snapshot.reported or not snapshot.owner_session:
            return
        try:
            await tool_coordinator.push_pending_hint(
                snapshot.owner_session,
                make_job_hint(snapshot),
            )
        except Exception:  # pylint: disable=broad-except
            logger.warning("failed to queue job hint for %s", snapshot.id, exc_info=True)
        if await _session_is_busy(snapshot, get_workspace):
            return
        if not get_job_registry().consume_wake(snapshot.owner_session):
            return
        await _followup_if_idle(snapshot, get_workspace)

    get_job_registry().on_job_done(_on_done)


async def _session_is_busy(snapshot: JobSnapshot, get_workspace: GetWorkspace) -> bool:
    if not snapshot.owner_agent:
        return False
    workspace = get_workspace(snapshot.owner_agent)
    if workspace is None:
        return False
    chat_mgr = getattr(workspace, "chat_manager", None)
    tracker = getattr(workspace, "task_tracker", None)
    if chat_mgr is None or tracker is None:
        return False
    chat_id = await chat_mgr.get_chat_id_by_session(
        snapshot.owner_session,
        snapshot.owner_channel or "console",
        snapshot.owner_user or None,
    )
    if not chat_id:
        return False
    return await tracker.get_status(chat_id) == "running"


async def _followup_if_idle(snapshot: JobSnapshot, get_workspace: GetWorkspace) -> None:
    if not snapshot.owner_agent:
        return
    workspace = get_workspace(snapshot.owner_agent)
    if workspace is None:
        return
    chat_mgr = getattr(workspace, "chat_manager", None)
    tracker = getattr(workspace, "task_tracker", None)
    channels = getattr(workspace, "channel_manager", None)
    if chat_mgr is None or tracker is None or channels is None:
        return
    chat_id = await chat_mgr.get_chat_id_by_session(
        snapshot.owner_session,
        snapshot.owner_channel or "console",
        snapshot.owner_user or None,
    )
    if not chat_id:
        return
    if await tracker.get_status(chat_id) == "running":
        return
    console = await channels.get_channel("console")
    if console is None:
        return
    chat = await chat_mgr.get_chat(chat_id)
    user_id = (chat.user_id if chat is not None else "") or snapshot.owner_user
    payload = {
        "channel_id": "console",
        "sender_id": user_id,
        "content_parts": [{"type": "text", "text": completion_text(snapshot)}],
        "meta": {
            "session_id": snapshot.owner_session,
            "request_context": {"source": "job_wakeup"},
        },
    }
    try:
        await tracker.attach_or_start(chat_id, payload, console.stream_one)
    except Exception:  # pylint: disable=broad-except
        logger.warning("idle followup failed for job %s", snapshot.id, exc_info=True)
