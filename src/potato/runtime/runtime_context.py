# -*- coding: utf-8 -*-
"""Append-only runtime-context snapshots for prefix-cache stability.

Date, session, cwd, model name, and driver policy used to live in the
system prompt. Any change there invalidates tools plus the whole
conversation for providers that cache by exact prefix (DeepSeek,
OpenAI). Codex and DeepSeek Harness keep the system prefix fixed and
append a user-role snapshot only when the bytes change.
"""

from __future__ import annotations

from typing import Any, Iterable

from agentscope.message import UserMsg

from ..constant import (
    EXTERNAL_USER_QUERY_MESSAGE_TAG,
    POTATO_MESSAGE_TAG_KEY,
    RUNTIME_CONTEXT_MESSAGE_TAG,
    SCROLL_MEMORY_MESSAGE_TAG,
    SYNTHETIC_USER_MESSAGE_TAGS,
    get_message_tag,
)

RUNTIME_CONTEXT_OPEN = "<runtime_context>"
RUNTIME_CONTEXT_CLOSE = "</runtime_context>"
_SUPERSEDE_LINE = (
    "The previous environment snapshot is superseded. Use this one."
)


def build_runtime_context_snapshot(
    env_context: str | None = None,
    driver_hints: Iterable[str] | None = None,
) -> str:
    """Render the canonical snapshot body wrapped in stable tags."""
    parts: list[str] = []
    env = (env_context or "").strip()
    if env:
        parts.append(env)
    hints = "\n\n".join(
        str(hint).strip() for hint in (driver_hints or ()) if hint
    )
    if hints:
        parts.append(hints)
    body = "\n\n".join(parts).strip()
    if not body:
        return ""
    return f"{RUNTIME_CONTEXT_OPEN}\n{body}\n{RUNTIME_CONTEXT_CLOSE}"


def snapshot_body(text: str | None) -> str:
    """Return the comparable inner body, ignoring wrap and supersession."""
    body = (text or "").strip()
    if body.startswith(RUNTIME_CONTEXT_OPEN):
        body = body[len(RUNTIME_CONTEXT_OPEN) :].lstrip("\n")
    if body.endswith(RUNTIME_CONTEXT_CLOSE):
        body = body[: -len(RUNTIME_CONTEXT_CLOSE)].rstrip("\n")
    body = body.strip()
    if body.startswith(_SUPERSEDE_LINE):
        body = body[len(_SUPERSEDE_LINE) :].lstrip()
    return body


def is_runtime_context_message(msg: Any) -> bool:
    metadata = getattr(msg, "metadata", None)
    return get_message_tag(metadata) == RUNTIME_CONTEXT_MESSAGE_TAG


def last_runtime_context_body(messages: Iterable[Any]) -> str | None:
    for msg in reversed(list(messages)):
        if is_runtime_context_message(msg):
            text = ""
            if hasattr(msg, "get_text_content"):
                text = msg.get_text_content() or ""
            elif isinstance(getattr(msg, "content", None), str):
                text = msg.content
            return snapshot_body(text)
    return None


def _latest_external_user_index(messages: list[Any]) -> int | None:
    tagged: int | None = None
    fallback: int | None = None
    for index in range(len(messages) - 1, -1, -1):
        msg = messages[index]
        if getattr(msg, "role", None) != "user":
            continue
        tag = get_message_tag(getattr(msg, "metadata", None))
        if tag == EXTERNAL_USER_QUERY_MESSAGE_TAG:
            tagged = index
            break
        if (
            fallback is None
            and tag != SCROLL_MEMORY_MESSAGE_TAG
            and tag not in SYNTHETIC_USER_MESSAGE_TAGS
        ):
            fallback = index
    return tagged if tagged is not None else fallback


def ensure_runtime_context_snapshot(
    messages: list[Any],
    snapshot: str,
) -> bool:
    """Insert *snapshot* before the latest user query when it changed.

    Returns True when a new message was inserted. Unchanged snapshots
    are left in place so the request prefix stays an append-only
    extension of the previous turn.
    """
    snapshot = (snapshot or "").strip()
    if not snapshot:
        return False
    wanted = snapshot_body(snapshot)
    if not wanted:
        return False
    last = last_runtime_context_body(messages)
    if last == wanted:
        return False

    text = snapshot
    if last is not None:
        inner = snapshot_body(snapshot)
        text = (
            f"{RUNTIME_CONTEXT_OPEN}\n"
            f"{_SUPERSEDE_LINE}\n\n"
            f"{inner}\n"
            f"{RUNTIME_CONTEXT_CLOSE}"
        )
    msg = UserMsg(
        name="runtime",
        content=text,
        metadata={POTATO_MESSAGE_TAG_KEY: RUNTIME_CONTEXT_MESSAGE_TAG},
    )
    index = _latest_external_user_index(messages)
    if index is None:
        messages.append(msg)
    else:
        messages.insert(index, msg)
    return True
