"""Tool metadata transport paths."""

from types import SimpleNamespace

import pytest
from agentscope.agent import Agent
from agentscope.event import EventType
from agentscope.message import (
    Msg,
    TextBlock,
    ToolCallBlock,
    ToolCallState,
    ToolResultBlock,
)
from agentscope.tool import FunctionTool, Toolkit, ToolChunk

from qwenpaw.app.chats.utils import agentscope_msg_to_message
from qwenpaw.runtime.envelope import Envelope
from qwenpaw.runtime.runtime import Runtime
from qwenpaw.runtime.tool_meta import build_qp_meta
from qwenpaw.tool_calls import ToolCoordinator, ToolCoordinatorMiddleware


class _TokenCountingModel:
    async def count_tokens(self, *_args, **_kwargs):
        return 1


def test_cancelled_tool_result_has_no_qp_metadata():
    call = ToolCallBlock(id="call", name="read_file", input="{}")
    msg = Msg(name="agent", role="assistant", content=[call])
    agent = SimpleNamespace(
        name="agent",
        state=SimpleNamespace(context=[msg]),
    )

    closed = Runtime._close_dangling_tool_calls(agent, Envelope())

    assert closed == 1
    result = msg.content[-1]
    assert isinstance(result, ToolResultBlock)
    assert result.metadata == {}
    assert "qp" not in result.metadata


@pytest.mark.asyncio
async def test_qp_meta_survives_real_tool_event_envelope_and_history_chain():
    qp = build_qp_meta(
        "file_write",
        True,
        {
            "path": "/tmp/result.txt",
            "bytes_written": 2,
            "additions": 1,
            "deletions": 0,
            "created": True,
        },
    )

    async def stub_write_file() -> ToolChunk:
        return ToolChunk(
            content=[TextBlock(text="ok")],
            metadata={"qp": qp},
        )

    agent = Agent(
        name="agent",
        system_prompt="",
        model=_TokenCountingModel(),
        toolkit=Toolkit(
            tools=[
                FunctionTool(
                    stub_write_file,
                    name="write_file",
                    description="Write a test file result.",
                ),
            ],
        ),
        middlewares=[
            ToolCoordinatorMiddleware(ToolCoordinator()),
        ],
    )
    agent._request_context = {  # pylint: disable=protected-access
        "session_id": "session-1",
        "agent_id": "agent-1",
        "root_session_id": "root-1",
    }
    tool_call = ToolCallBlock(
        id="call-1",
        name="write_file",
        input="{}",
        state=ToolCallState.ALLOWED,
    )

    events = [
        event
        async for event in agent._execute_tool_call(  # pylint: disable=protected-access
            tool_call,
        )
    ]
    end_event = next(
        event
        for event in events
        if event.type == EventType.TOOL_RESULT_END
    )
    assert end_event.metadata["qp"] == qp

    envelope = Envelope()
    payloads = []
    for event in events:
        payloads.extend(
            [
                item.model_dump(mode="python")
                async for item in envelope.translate_event(event)
            ],
        )
    final_output = next(
        payload["data"]
        for payload in payloads
        if isinstance(payload.get("data"), dict)
        and payload["data"].get("meta") == qp
    )
    assert final_output["meta"] == qp

    [history_message] = agentscope_msg_to_message(agent.state.context[-1])
    assert history_message.content[0].data["meta"] == qp
