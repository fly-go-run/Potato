"""Tool metadata paths outside the main envelope."""

from types import SimpleNamespace

from agentscope.message import Msg, ToolCallBlock, ToolResultBlock

from qwenpaw.runtime.envelope import Envelope
from qwenpaw.runtime.runtime import Runtime


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
