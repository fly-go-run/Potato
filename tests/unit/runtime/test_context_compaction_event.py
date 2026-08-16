# -*- coding: utf-8 -*-
"""Structured SSE lifecycle for automatic context compaction."""

from __future__ import annotations

import pytest
from agentscope.event import CustomEvent

from potato.runtime.envelope import Envelope


async def _dump(stream):
    return [item.model_dump(mode="python") async for item in stream]


@pytest.mark.asyncio
async def test_context_compaction_is_one_progress_message() -> None:
    envelope = Envelope(session_id="session-1")
    started = CustomEvent(
        name="context_compaction",
        value={"operation_id": "compact-1", "status": "in_progress"},
    )
    completed = CustomEvent(
        name="context_compaction",
        value={"operation_id": "compact-1", "status": "completed"},
    )

    start_payloads = await _dump(envelope.translate_event(started))
    done_payloads = await _dump(envelope.translate_event(completed))

    assert [payload["object"] for payload in start_payloads] == [
        "message",
        "content",
    ]
    assert start_payloads[0]["type"] == "progress"
    assert start_payloads[0]["metadata"] == {
        "kind": "context_compaction",
        "phase": "in_progress",
    }
    assert [payload["object"] for payload in done_payloads] == [
        "content",
        "message",
    ]
    assert done_payloads[-1]["status"] == "completed"
    assert done_payloads[-1]["metadata"]["phase"] == "completed"
