"""Structured metadata produced by file-send and batch tools."""

import json

import pytest
from agentscope.message import TextBlock

from qwenpaw.agents.tools.run_tool_batch import _build_batch_response
from qwenpaw.agents.tools.send_file import send_file_to_user
from qwenpaw.runtime.tool_meta import QP_META_MAX_BYTES, build_batch_qp_meta


@pytest.mark.asyncio
async def test_file_sent_metadata_success_and_failure_schema(tmp_path):
    path = tmp_path / "artifact.bin"
    path.write_bytes(b"payload")

    success = await send_file_to_user(str(path))
    assert success.metadata["qp"] == {
        "v": 1,
        "kind": "file_sent",
        "ok": True,
        "data": {
            "path": str(path),
            "size_bytes": 7,
            "attached": True,
        },
    }

    missing_path = str(tmp_path / "missing.bin")
    failure = await send_file_to_user(missing_path)
    # The legacy SUCCESS state is intentionally unchanged; qp is semantic.
    assert failure.metadata["qp"] == {
        "v": 1,
        "kind": "file_sent",
        "ok": False,
        "data": {"path": missing_path},
    }


def test_batch_metadata_success_and_failure_schema():
    success = _build_batch_response(
        [{"tool_name": "read_file"}, {"tool_name": "write_file"}],
        [
            {"step": 0, "tool_name": "read_file", "ok": True},
            {"step": 1, "tool_name": "write_file", "ok": True},
        ],
        [],
    )
    assert success.metadata["qp"] == {
        "v": 1,
        "kind": "batch",
        "ok": True,
        "data": {
            "total": 2,
            "completed": 2,
            "failed": 0,
            "truncated": False,
            "steps": [
                {"tool": "read_file", "ok": True},
                {"tool": "write_file", "ok": True},
            ],
        },
    }

    failure = _build_batch_response(
        [{"tool_name": "read_file"}],
        [{"step": 0, "tool_name": "read_file", "ok": False}],
        [TextBlock(type="text", text="kept")],
    )
    assert failure.metadata["qp"]["ok"] is False
    assert set(failure.metadata["qp"]["data"]) == {
        "total",
        "completed",
        "failed",
        "truncated",
        "steps",
    }


def test_batch_metadata_respects_50_step_and_4kb_boundaries():
    within = build_batch_qp_meta(
        ok=True,
        total=50,
        completed=50,
        failed=0,
        steps=[{"tool": "x" * 50, "ok": True} for _ in range(50)],
    )
    assert len(within["data"]["steps"]) == 50
    assert within["data"]["truncated"] is False

    over = build_batch_qp_meta(
        ok=True,
        total=50,
        completed=50,
        failed=0,
        steps=[{"tool": "x" * 60, "ok": True} for _ in range(50)],
    )
    serialized = json.dumps(
        over,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    assert len(serialized) <= QP_META_MAX_BYTES
    assert len(over["data"]["steps"]) < 50
    assert over["data"]["truncated"] is True

    capped = build_batch_qp_meta(
        ok=True,
        total=51,
        completed=51,
        failed=0,
        steps=[{"tool": "x", "ok": True} for _ in range(51)],
    )
    assert len(capped["data"]["steps"]) == 50
    assert capped["data"]["truncated"] is True
