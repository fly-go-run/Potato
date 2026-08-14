# -*- coding: utf-8 -*-
"""Tests for ToolCoordinator completion and offload lifecycle."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any, AsyncGenerator

import pytest
from agentscope.message import TextBlock, ToolResultBlock, ToolResultState
from agentscope.tool import ToolChunk, ToolResponse

from qwenpaw.runtime.tool_meta import build_qp_meta
from qwenpaw.tool_calls import (
    OffloadUnavailableError,
    ToolCoordinator,
    ToolCoordinatorMiddleware,
)
from qwenpaw.tool_calls._context import ToolCallContext
from qwenpaw.tool_calls._entry import ToolCallEntry
from qwenpaw.tool_calls._stream import ToolStream


@dataclass
class _ToolCall:
    id: str = "call-1"
    name: str = "test_tool"
    input: dict[str, Any] = field(default_factory=dict)


def _text_response(tool_call_id: str, text: str) -> ToolResponse:
    return ToolResponse(
        content=[TextBlock(type="text", text=text)],
        id=tool_call_id,
    )


def _tool_response_text_bytes(response: ToolResponse) -> int:
    return sum(
        len(block.text.encode("utf-8"))
        for block in response.content
        if getattr(block, "type", None) == "text"
    )


def _tool_result_output_text_bytes(block: ToolResultBlock) -> int:
    if isinstance(block.output, str):
        return len(block.output.encode("utf-8"))
    return sum(
        len(output.text.encode("utf-8"))
        for output in block.output
        if getattr(output, "type", None) == "text"
    )


async def _collect(
    iterator: AsyncGenerator[Any, None],
) -> list[Any]:
    events: list[Any] = []
    async for item in iterator:
        events.append(item)
    return events


def _tool_chunk(*, text: str, is_last: bool, qp=None) -> ToolChunk:
    metadata = {} if qp is None else {"qp": qp}
    return ToolChunk(
        content=[TextBlock(type="text", text=text)],
        is_last=is_last,
        state=(
            ToolResultState.SUCCESS
            if is_last
            else ToolResultState.RUNNING
        ),
        metadata=metadata,
    )


async def _wait_for_hint(
    coordinator: ToolCoordinator,
    session_id: str,
) -> Any:
    while True:
        hints = await coordinator.pop_pending_hints(session_id)
        if hints:
            return hints[0]
        await asyncio.sleep(0.01)


@pytest.mark.asyncio
async def test_non_terminal_qp_is_discarded_before_chunk_aggregation(caplog):
    coordinator = ToolCoordinator()
    tool_call = _ToolCall()
    stale_qp = build_qp_meta(
        "shell",
        True,
        {"sandboxed": False, "exit_code": 1},
    )

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        yield _tool_chunk(text="partial", is_last=False, qp=stale_qp)
        yield _tool_chunk(text="done", is_last=True)

    events = await _collect(
        coordinator.execute(
            tool_call=tool_call,
            next_handler=next_handler,
            session_id="session-1",
            agent_id="agent-1",
            root_session_id="root-1",
        ),
    )

    assert "qp" not in events[-1].metadata
    assert "discarded qp metadata from non-terminal chunk" in caplog.text


@pytest.mark.asyncio
async def test_terminal_qp_wins_after_non_terminal_qp_is_discarded():
    coordinator = ToolCoordinator()
    tool_call = _ToolCall()
    stale_qp = build_qp_meta(
        "shell",
        True,
        {"sandboxed": False, "exit_code": 1},
    )
    terminal_qp = build_qp_meta(
        "shell",
        True,
        {"sandboxed": False, "exit_code": 0},
    )

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        yield _tool_chunk(text="partial", is_last=False, qp=stale_qp)
        yield _tool_chunk(text="done", is_last=True, qp=terminal_qp)

    events = await _collect(
        coordinator.execute(
            tool_call=tool_call,
            next_handler=next_handler,
            session_id="session-1",
            agent_id="agent-1",
            root_session_id="root-1",
        ),
    )

    assert events[-1].metadata["qp"] == terminal_qp


@pytest.mark.asyncio
async def test_after_hook_transforms_final_response_and_blocks_caller():
    coordinator = ToolCoordinator()
    tool_call = _ToolCall(name="expanding_tool")
    after_started = asyncio.Event()
    release_after = asyncio.Event()

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        yield _text_response(tool_call.id, "small")

    async def after_hook(
        response: ToolResponse,
        ctx: ToolCallContext,
    ) -> ToolResponse:
        assert response.content[0].text == "small"
        after_started.set()
        await release_after.wait()
        return _text_response(ctx.tool_call_id, "x" * 2000)

    coordinator.hooks.register("expanding_tool", after=after_hook)
    task = asyncio.create_task(
        _collect(
            coordinator.execute(
                tool_call=tool_call,
                next_handler=next_handler,
                session_id="session-1",
                agent_id="agent-1",
                root_session_id="root-1",
            ),
        ),
    )

    await asyncio.wait_for(after_started.wait(), timeout=1)
    await asyncio.sleep(0)
    assert not task.done()

    release_after.set()
    events = await asyncio.wait_for(task, timeout=1)
    final = events[-1]

    assert isinstance(final, ToolResponse)
    assert _tool_response_text_bytes(final) == 2000


@pytest.mark.asyncio
async def test_middleware_caller_observes_coordinator_response():
    coordinator = ToolCoordinator()
    middleware = ToolCoordinatorMiddleware(
        coordinator=coordinator,
    )
    agent = type(
        "AgentStub",
        (),
        {
            "_request_context": {
                "session_id": "session-1",
                "agent_id": "agent-1",
                "root_session_id": "root-1",
            },
        },
    )()
    tool_call = _ToolCall()

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        yield _text_response(tool_call.id, "x" * 2000)

    events = await _collect(
        middleware.on_acting(
            agent,
            {"tool_call": tool_call},
            next_handler,
        ),
    )

    assert _tool_response_text_bytes(events[-1]) == 2000


@pytest.mark.asyncio
@pytest.mark.skip(reason="offload globally disabled pending fix")
async def test_background_completion_emits_hint():
    coordinator = ToolCoordinator(default_timeout_secs=0.001)
    tool_call = _ToolCall(id="call-bg", name="slow_tool")

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        await asyncio.sleep(0.02)
        yield _text_response(tool_call.id, "x" * 2000)

    events = await _collect(
        coordinator.execute(
            tool_call=tool_call,
            next_handler=next_handler,
            session_id="session-bg",
            agent_id="agent-1",
            root_session_id="root-1",
        ),
    )
    hint = await asyncio.wait_for(
        _wait_for_hint(coordinator, "session-bg"),
        timeout=1,
    )

    assert events[-1].metadata["offloaded"] is True
    assert hint.role == "assistant"
    tool_result = next(
        block
        for block in hint.content
        if getattr(block, "type", None) == "tool_result"
    )
    assert _tool_result_output_text_bytes(tool_result) == 2000


@pytest.mark.asyncio
async def test_caller_cancellation_does_not_cancel_background_task():
    # pylint: disable=protected-access
    bg_started = asyncio.Event()
    bg_can_finish = asyncio.Event()
    tool_call = _ToolCall(id="call-cancel", name="slow_tool")

    async def background() -> None:
        bg_started.set()
        await bg_can_finish.wait()

    bg_task = asyncio.create_task(background())
    entry = ToolCallEntry(
        ctx=ToolCallContext(
            tool_call_id=tool_call.id,
            tool_name=tool_call.name,
            session_id="session-cancel",
            agent_id="agent-1",
            root_session_id="root-1",
            started_at=0.0,
            deadline=None,
            cancel_event=asyncio.Event(),
        ),
        stream=ToolStream(
            tool_call_id=tool_call.id,
            session_id="session-cancel",
        ),
        final_response=ToolResponse(id=tool_call.id),
        background_task=bg_task,
    )

    waiter = asyncio.create_task(
        ToolCoordinator._await_background_task(entry),
    )
    await asyncio.wait_for(bg_started.wait(), timeout=1)
    await asyncio.sleep(0)

    waiter.cancel()
    with pytest.raises(asyncio.CancelledError):
        await waiter

    assert not bg_task.cancelled()
    assert not bg_task.done()

    bg_can_finish.set()
    await asyncio.wait_for(bg_task, timeout=1)


@pytest.mark.asyncio
async def test_same_provider_tool_id_is_isolated_by_session_and_agent():
    """Provider ids like ``call-1`` cannot collide across live sessions."""
    coordinator = ToolCoordinator()
    both_started = asyncio.Event()
    release = asyncio.Event()
    started: set[tuple[str, str]] = set()

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        started.add((tool_call.input["session"], tool_call.input["agent"]))
        if len(started) == 2:
            both_started.set()
        await release.wait()
        yield _text_response(tool_call.id, tool_call.input["session"])

    async def run(session_id: str, agent_id: str):
        return await _collect(
            coordinator.execute(
                tool_call=_ToolCall(
                    id="provider-call-1",
                    input={"session": session_id, "agent": agent_id},
                ),
                next_handler=next_handler,
                session_id=session_id,
                agent_id=agent_id,
                root_session_id=session_id,
            ),
        )

    first = asyncio.create_task(run("session-a", "agent-a"))
    second = asyncio.create_task(run("session-b", "agent-b"))
    await asyncio.wait_for(both_started.wait(), timeout=1)

    assert len(coordinator.list_entries()) == 2
    assert coordinator.list_entries(
        session_id="session-a",
        agent_id="agent-a",
    ) == [
        coordinator.get(
            "provider-call-1",
            session_id="session-a",
            agent_id="agent-a",
        ),
    ]
    assert coordinator.get("provider-call-1") is None
    entry_a = coordinator.get("provider-call-1", session_id="session-a")
    entry_b = coordinator.get("provider-call-1", session_id="session-b")
    assert entry_a is not None
    assert entry_b is not None

    assert await coordinator.cancel(
        "provider-call-1",
        session_id="session-a",
        agent_id="agent-a",
    )
    assert entry_a.ctx.cancel_event.is_set()
    assert not entry_b.ctx.cancel_event.is_set()

    release.set()
    await asyncio.wait_for(asyncio.gather(first, second), timeout=1)


@pytest.mark.asyncio
async def test_old_completion_cannot_remove_newer_same_scope_entry():
    """Identity-checked cleanup protects a replacement with the same key."""
    coordinator = ToolCoordinator()
    first_started = asyncio.Event()
    second_started = asyncio.Event()
    release_first = asyncio.Event()
    release_second = asyncio.Event()

    async def next_handler(
        tool_call: _ToolCall,
    ) -> AsyncGenerator[Any, None]:
        if tool_call.input["which"] == "first":
            first_started.set()
            await release_first.wait()
        else:
            second_started.set()
            await release_second.wait()
        yield _text_response(tool_call.id, tool_call.input["which"])

    async def run(which: str):
        return await _collect(
            coordinator.execute(
                tool_call=_ToolCall(
                    id="duplicate-in-one-session",
                    input={"which": which},
                ),
                next_handler=next_handler,
                session_id="session-a",
                agent_id="agent-a",
                root_session_id="session-a",
            ),
        )

    first = asyncio.create_task(run("first"))
    await asyncio.wait_for(first_started.wait(), timeout=1)
    second = asyncio.create_task(run("second"))
    await asyncio.wait_for(second_started.wait(), timeout=1)

    release_first.set()
    await asyncio.wait_for(first, timeout=1)
    still_live = coordinator.get(
        "duplicate-in-one-session",
        session_id="session-a",
        agent_id="agent-a",
    )
    assert still_live is not None
    assert not still_live.background_task.done()

    release_second.set()
    await asyncio.wait_for(second, timeout=1)


@pytest.mark.asyncio
async def test_request_offload_is_explicitly_unavailable():
    coordinator = ToolCoordinator()

    with pytest.raises(
        OffloadUnavailableError,
        match="temporarily unavailable",
    ):
        await coordinator.request_offload("any-call")
