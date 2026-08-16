# -*- coding: utf-8 -*-
"""Generic controls for background jobs (DeepSeek Harness-shaped)."""
from __future__ import annotations

from agentscope.message import TextBlock, ToolResultState
from agentscope.tool import ToolChunk

from ...runtime.jobs import (
    JobAccessError,
    coerce_bool,
    get_job_registry,
)
from ...runtime.tool_registry import tool_descriptor
from ...tool_calls import get_call_context


def _session_id() -> str:
    ctx = get_call_context()
    if ctx is None:
        return ""
    return ctx.root_session_id or ctx.session_id


def _text(message: str, *, ok: bool = True) -> ToolChunk:
    return ToolChunk(
        is_last=True,
        state=ToolResultState.SUCCESS if ok else ToolResultState.ERROR,
        content=[TextBlock(type="text", text=message)],
    )


def _status_line(snapshot) -> str:
    extra = f" — {snapshot.detail}" if snapshot.detail else ""
    return f"[status: {snapshot.status}{extra}]"


@tool_descriptor(
    async_execution=True,
    tool_type="internal",
    ui_description="Read background job output",
    ui_icon="📋",
)
async def job_output(
    job_id: str,
    wait: bool | str | int = False,
    timeout_ms: float = 30000,
) -> ToolChunk:
    """Read a background job started with run_in_background.

    Stream jobs return only output since the previous read. Every
    response ends with `[status: ...]`. Set wait=true only when you
    are blocked on this job; a timed-out wait returns [status: running]
    and leaves the job alive.
    """
    if not (job_id or "").strip():
        return _text("Error: job_id is required", ok=False)
    try:
        blocking = coerce_bool(wait, default=False, field_name="wait")
    except ValueError as exc:
        return _text(f"Error: {exc}", ok=False)
    try:
        timeout = float(timeout_ms)
    except (TypeError, ValueError):
        timeout = 30000.0
    registry = get_job_registry()
    session_id = _session_id()
    try:
        if blocking:
            snapshot = await registry.wait(
                job_id.strip(),
                timeout,
                session_id=session_id,
            )
            text, snapshot = await registry.read(
                job_id.strip(),
                session_id=session_id,
            )
        else:
            text, snapshot = await registry.read(
                job_id.strip(),
                session_id=session_id,
            )
    except KeyError as exc:
        return _text(f"Error: {exc}", ok=False)
    except JobAccessError as exc:
        return _text(f"Error: {exc}", ok=False)
    except ValueError as exc:
        return _text(f"Error: {exc}", ok=False)
    body = text if text else "(no new output)"
    return _text(f"{body}\n{_status_line(snapshot)}")


@tool_descriptor(
    async_execution=True,
    tool_type="internal",
    ui_description="List background jobs",
    ui_icon="📋",
)
async def job_list() -> ToolChunk:
    """List background jobs visible in this session."""
    jobs = await get_job_registry().list(session_id=_session_id())
    if not jobs:
        return _text("(no background jobs)")
    lines = [
        f"{job.id} [{job.kind}] {job.status} — {job.label}"
        for job in jobs
    ]
    return _text("\n".join(lines))


@tool_descriptor(
    async_execution=True,
    tool_type="internal",
    ui_description="Stop a background job",
    ui_icon="🛑",
)
async def job_kill(job_id: str) -> ToolChunk:
    """Request cancellation of a background job. Already-finished jobs
    report their existing status.
    """
    if not (job_id or "").strip():
        return _text("Error: job_id is required", ok=False)
    try:
        outcome = await get_job_registry().kill(
            job_id.strip(),
            session_id=_session_id(),
        )
    except KeyError as exc:
        return _text(f"Error: {exc}", ok=False)
    except JobAccessError as exc:
        return _text(f"Error: {exc}", ok=False)
    if outcome == "already-finished":
        return _text(f"job {job_id.strip()} already finished")
    return _text(f"requested cancellation of job {job_id.strip()}")
