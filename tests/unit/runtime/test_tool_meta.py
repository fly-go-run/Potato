"""Tests for the structured tool-result metadata contract."""

import pytest

from qwenpaw.runtime.tool_meta import (
    QP_META_MAX_BYTES,
    build_qp_meta,
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
