"""Structured metadata contract for QwenPaw tool results."""

from __future__ import annotations

import json
from difflib import SequenceMatcher
from typing import Any, Literal, Mapping, TypedDict, cast

QP_META_VERSION = 1
QP_META_MAX_BYTES = 4 * 1024

ToolMetaKind = Literal[
    "file_write",
    "file_edit",
    "file_read",
    "shell",
    "file_sent",
    "web_search",
    "batch",
]

TOOL_META_KINDS = frozenset(
    {
        "file_write",
        "file_edit",
        "file_read",
        "shell",
        "file_sent",
        "web_search",
        "batch",
    },
)


class QpMeta(TypedDict):
    """Versioned, JSON-safe metadata attached to a terminal tool chunk."""

    v: Literal[1]
    kind: ToolMetaKind
    ok: bool
    data: dict[str, Any]


def _serialized_size(value: object) -> int:
    try:
        serialized = json.dumps(
            value,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    except (TypeError, ValueError) as exc:
        raise ValueError("qp metadata must be JSON-serializable") from exc
    return len(serialized.encode("utf-8"))


def validate_qp_meta(meta: object) -> QpMeta:
    """Validate and return one complete ``metadata['qp']`` value."""

    if not isinstance(meta, dict):
        raise ValueError("qp metadata must be a dict")
    if set(meta) != {"v", "kind", "ok", "data"}:
        raise ValueError("qp metadata must contain only v, kind, ok, and data")
    if meta["v"] != QP_META_VERSION:
        raise ValueError(f"unsupported qp metadata version: {meta['v']!r}")
    if meta["kind"] not in TOOL_META_KINDS:
        raise ValueError(f"unsupported qp metadata kind: {meta['kind']!r}")
    if not isinstance(meta["ok"], bool):
        raise ValueError("qp metadata ok must be a bool")
    if not isinstance(meta["data"], dict):
        raise ValueError("qp metadata data must be a dict")

    size = _serialized_size(meta)
    if size > QP_META_MAX_BYTES:
        raise ValueError(
            f"qp metadata exceeds {QP_META_MAX_BYTES} bytes: {size}",
        )
    return cast(QpMeta, meta)


def build_qp_meta(
    kind: ToolMetaKind | str,
    ok: bool,
    data: Mapping[str, Any],
) -> QpMeta:
    """Build a validated ``metadata['qp']`` value for a tool result."""

    if not isinstance(data, Mapping):
        raise ValueError("qp metadata data must be a mapping")
    meta = {
        "v": QP_META_VERSION,
        "kind": kind,
        "ok": ok,
        "data": dict(data),
    }
    return validate_qp_meta(meta)


def count_line_changes(before: str, after: str) -> tuple[int, int]:
    """Return line additions/deletions for two text snapshots.

    ``splitlines()`` makes a final line count as one whether or not it has a
    trailing newline. A newline-only EOF change therefore does not invent a
    line addition or deletion.
    """

    matcher = SequenceMatcher(
        None,
        before.splitlines(),
        after.splitlines(),
        autojunk=False,
    )
    additions = 0
    deletions = 0
    for tag, before_start, before_end, after_start, after_end in (
        matcher.get_opcodes()
    ):
        if tag in {"replace", "delete"}:
            deletions += before_end - before_start
        if tag in {"replace", "insert"}:
            additions += after_end - after_start
    return additions, deletions


def build_batch_qp_meta(
    *,
    ok: bool,
    total: int,
    completed: int,
    failed: int,
    steps: list[Mapping[str, Any]],
) -> QpMeta:
    """Build bounded batch metadata, retaining the earliest complete steps."""

    retained = [dict(step) for step in steps[:50]]
    truncated = len(retained) < len(steps)
    while True:
        try:
            return build_qp_meta(
                "batch",
                ok,
                {
                    "total": total,
                    "completed": completed,
                    "failed": failed,
                    "truncated": truncated,
                    "steps": retained,
                },
            )
        except ValueError as exc:
            if "exceeds" not in str(exc) or not retained:
                raise
            retained.pop()
            truncated = True


def assert_qp_terminal_chunk(chunk: object) -> None:
    """Assert that ``qp`` is valid and appears only on a final chunk.

    Tool producers and aggregation tests can call this at their boundary.
    Chunks without ``qp`` remain valid because structured metadata is optional.
    """

    metadata = getattr(chunk, "metadata", None)
    if not isinstance(metadata, dict) or "qp" not in metadata:
        return
    if getattr(chunk, "is_last", None) is not True:
        raise AssertionError("metadata['qp'] is only allowed on a final chunk")
    validate_qp_meta(metadata["qp"])
