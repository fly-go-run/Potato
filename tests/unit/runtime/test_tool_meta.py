"""Tests for the structured tool-result metadata contract."""

import pytest

from potato.runtime.tool_meta import (
    QP_META_MAX_BYTES,
    build_qp_meta,
)


def test_build_qp_meta_validates_kind_json_and_size():
    meta = build_qp_meta(
        "file_read",
        True,
        {
            "path": "/tmp/a",
            "bytes_read": 1,
            "line_start": 1,
            "line_end": 1,
            "total_lines": 1,
        },
    )
    assert meta == {
        "v": 1,
        "kind": "file_read",
        "ok": True,
        "data": {
            "path": "/tmp/a",
            "bytes_read": 1,
            "line_start": 1,
            "line_end": 1,
            "total_lines": 1,
        },
    }

    with pytest.raises(ValueError, match="kind"):
        build_qp_meta("unknown", True, {})
    with pytest.raises(ValueError, match="JSON-serializable"):
        build_qp_meta(
            "shell",
            False,
            {"sandboxed": False, "violation": object()},
        )
    with pytest.raises(ValueError, match="exceeds"):
        build_qp_meta(
            "web_search",
            False,
            {
                "backend": "hosted",
                "future_field": "x" * QP_META_MAX_BYTES,
            },
        )


@pytest.mark.parametrize(
    ("kind", "data", "missing_field"),
    [
        (
            "file_write",
            {
                "path": "/tmp/a",
                "byte_written": 1,
                "additions": 1,
                "deletions": 0,
                "created": True,
            },
            "bytes_written",
        ),
        (
            "file_edit",
            {
                "path": "/tmp/a",
                "replacement": 1,
                "additions": 1,
                "deletions": 1,
            },
            "replacements",
        ),
        (
            "file_read",
            {
                "path": "/tmp/a",
                "byte_read": 1,
                "line_start": 1,
                "line_end": 1,
                "total_lines": 1,
            },
            "bytes_read",
        ),
        (
            "shell",
            {"sandboxed": False, "exit_cod": 0},
            "exit_code",
        ),
        (
            "file_sent",
            {"path": "/tmp/a", "size_byte": 1, "attached": True},
            "size_bytes",
        ),
        (
            "web_search",
            {"back_end": "hosted"},
            "backend",
        ),
        (
            "batch",
            {
                "total": 1,
                "complete": 1,
                "failed": 0,
                "truncated": False,
            },
            "completed",
        ),
    ],
)
def test_build_qp_meta_rejects_misspelled_required_field(
    kind,
    data,
    missing_field,
):
    with pytest.raises(ValueError, match=missing_field):
        build_qp_meta(kind, True, data)


def test_build_qp_meta_warns_but_accepts_unknown_field(caplog):
    meta = build_qp_meta(
        "web_search",
        True,
        {"backend": "hosted", "future_field": "value"},
    )

    assert meta["data"]["future_field"] == "value"
    assert "unknown qp metadata data fields" in caplog.text
