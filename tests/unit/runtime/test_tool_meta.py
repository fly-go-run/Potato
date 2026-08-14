"""Tests for the structured tool-result metadata contract."""

import pytest
from agentscope.message import TextBlock, ToolResultState
from agentscope.tool import ToolChunk, ToolResponse

from qwenpaw.runtime.tool_meta import (
    QP_META_MAX_BYTES,
    assert_qp_terminal_chunk,
    build_qp_meta,
    validate_qp_meta,
)


def _chunk(*, text: str, is_last: bool, qp=None) -> ToolChunk:
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


def test_build_qp_meta_validates_kind_json_and_size():
    meta = build_qp_meta("file_read", True, {"path": "/tmp/a"})
    assert meta == {
        "v": 1,
        "kind": "file_read",
        "ok": True,
        "data": {"path": "/tmp/a"},
    }

    with pytest.raises(ValueError, match="kind"):
        build_qp_meta("unknown", True, {})
    with pytest.raises(ValueError, match="JSON-serializable"):
        build_qp_meta("file_read", True, {"bad": object()})
    with pytest.raises(ValueError, match="exceeds"):
        build_qp_meta(
            "file_read",
            True,
            {"text": "x" * QP_META_MAX_BYTES},
        )


def test_qp_is_rejected_on_non_terminal_chunk():
    qp = build_qp_meta("shell", True, {"sandboxed": False, "exit_code": 0})
    with pytest.raises(AssertionError, match="final chunk"):
        assert_qp_terminal_chunk(_chunk(text="partial", is_last=False, qp=qp))


@pytest.mark.parametrize("chunk_count", [2, 3])
def test_append_chunk_keeps_the_terminal_qp_value(chunk_count):
    terminal_qp = build_qp_meta(
        "shell",
        True,
        {"sandboxed": False, "exit_code": 0},
    )
    chunks = [
        _chunk(text=f"part-{index}", is_last=False)
        for index in range(chunk_count - 1)
    ]
    chunks.append(_chunk(text="done", is_last=True, qp=terminal_qp))

    response = ToolResponse()
    for chunk in chunks:
        assert_qp_terminal_chunk(chunk)
        response.append_chunk(chunk)

    assert validate_qp_meta(response.metadata["qp"]) == terminal_qp
