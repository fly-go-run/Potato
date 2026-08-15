"""Tool-call output transport preserves terminal structured metadata."""

import json
from types import SimpleNamespace

import pytest
from agentscope.message import TextBlock, ToolResultState
from agentscope.tool import ToolChunk

from qwenpaw.app.routers.tool_calls import get_output, stream_output


def _request(entry):
    coordinator = SimpleNamespace(get=lambda *_args, **_kwargs: entry)
    services = SimpleNamespace(tool_coordinator=coordinator)
    return SimpleNamespace(
        app=SimpleNamespace(state=SimpleNamespace(app_services=services)),
        state=SimpleNamespace(agent_id=None),
        headers={},
    )


@pytest.mark.asyncio
async def test_output_includes_final_qp_meta():
    qp = {
        "v": 1,
        "kind": "shell",
        "ok": True,
        "data": {"sandboxed": False, "exit_code": 0},
    }
    response = ToolChunk(
        content=[TextBlock(type="text", text="unchanged")],
        state=ToolResultState.SUCCESS,
        metadata={"qp": qp},
    )
    entry = SimpleNamespace(
        final_response=response,
        stream=SimpleNamespace(is_closed=True),
        end_state="success",
    )

    output = await get_output("session", "call", _request(entry))

    assert output["meta"] == qp
    assert output["content"][0]["text"] == "unchanged"


@pytest.mark.asyncio
async def test_stream_keeps_raw_chunk_metadata_and_terminal_uniqueness():
    qp = {
        "v": 1,
        "kind": "shell",
        "ok": True,
        "data": {"sandboxed": False, "exit_code": 0},
    }
    chunks = [
        ToolChunk(
            content=[TextBlock(type="text", text="partial")],
            is_last=False,
        ),
        ToolChunk(
            content=[TextBlock(type="text", text="done")],
            is_last=True,
            metadata={"qp": qp},
        ),
    ]

    class _Stream:
        is_closed = True

        async def subscribe(self):
            for chunk in chunks:
                yield chunk

    entry = SimpleNamespace(stream=_Stream())
    response = await stream_output("session", "call", _request(entry))
    payloads = []
    async for item in response.body_iterator:
        payloads.append(json.loads(item.removeprefix("data: ")))

    assert "qp" not in payloads[0]["data"]["metadata"]
    assert payloads[1]["data"]["metadata"]["qp"] == qp
    assert payloads[2] == {"type": "done"}
