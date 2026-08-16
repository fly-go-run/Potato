# -*- coding: utf-8 -*-
"""Prefix-stable runtime-context snapshots."""

from __future__ import annotations

from types import SimpleNamespace

import pytest
from agentscope.message import UserMsg

from potato.constant import (
    EXTERNAL_USER_QUERY_MESSAGE_TAG,
    POTATO_MESSAGE_TAG_KEY,
    RUNTIME_CONTEXT_MESSAGE_TAG,
)
from potato.runtime.runtime_context import (
    build_runtime_context_snapshot,
    ensure_runtime_context_snapshot,
    last_runtime_context_body,
    snapshot_body,
)


def _user(text: str, *, external: bool = True) -> UserMsg:
    metadata = (
        {POTATO_MESSAGE_TAG_KEY: EXTERNAL_USER_QUERY_MESSAGE_TAG}
        if external
        else {}
    )
    return UserMsg(name="user", content=text, metadata=metadata)


def test_build_snapshot_wraps_env_and_hints():
    text = build_runtime_context_snapshot(
        "====================\n- Current date: 2026-08-16 UTC\n====================",
        ["Driver policy: ask"],
    )
    assert text.startswith("<runtime_context>")
    assert text.endswith("</runtime_context>")
    assert "Current date: 2026-08-16" in text
    assert "Driver policy: ask" in text


def test_empty_inputs_yield_no_snapshot():
    assert build_runtime_context_snapshot("", []) == ""
    assert build_runtime_context_snapshot(None, None) == ""


def test_first_turn_inserts_snapshot_before_user():
    messages = [_user("hello 1")]
    snapshot = build_runtime_context_snapshot("- Session ID: s1")
    assert ensure_runtime_context_snapshot(messages, snapshot) is True
    assert len(messages) == 2
    assert messages[0].metadata[POTATO_MESSAGE_TAG_KEY] == (
        RUNTIME_CONTEXT_MESSAGE_TAG
    )
    assert messages[1].get_text_content() == "hello 1"
    assert last_runtime_context_body(messages) == "- Session ID: s1"


def test_unchanged_snapshot_is_not_rewritten():
    messages = [_user("hello 1")]
    snapshot = build_runtime_context_snapshot("- Session ID: s1")
    ensure_runtime_context_snapshot(messages, snapshot)
    first_id = messages[0].id
    prefix = list(messages)
    assert ensure_runtime_context_snapshot(messages, snapshot) is False
    assert [msg.id for msg in messages] == [msg.id for msg in prefix]
    assert messages[0].id == first_id


def test_changed_snapshot_appends_and_keeps_old_prefix():
    messages = [_user("hello 1")]
    first = build_runtime_context_snapshot("- Current date: 2026-08-16")
    ensure_runtime_context_snapshot(messages, first)
    messages.append(UserMsg(name="assistant", content="hi"))
    messages.append(_user("hello 2"))

    second = build_runtime_context_snapshot("- Current date: 2026-08-17")
    assert ensure_runtime_context_snapshot(messages, second) is True

    assert snapshot_body(messages[0].get_text_content()) == (
        "- Current date: 2026-08-16"
    )
    assert messages[1].get_text_content() == "hello 1"
    assert messages[2].get_text_content() == "hi"
    assert "superseded" in (messages[3].get_text_content() or "")
    assert snapshot_body(messages[3].get_text_content()) == (
        "- Current date: 2026-08-17"
    )
    assert messages[4].get_text_content() == "hello 2"


def test_untagged_user_still_gets_snapshot_before_it():
    messages = [UserMsg(name="user", content="untagged")]
    snapshot = build_runtime_context_snapshot("- OS: macOS")
    assert ensure_runtime_context_snapshot(messages, snapshot) is True
    assert messages[0].metadata[POTATO_MESSAGE_TAG_KEY] == (
        RUNTIME_CONTEXT_MESSAGE_TAG
    )
    assert messages[1].get_text_content() == "untagged"


@pytest.mark.asyncio
async def test_middleware_inserts_snapshot_before_reply():
    from potato.agents.middlewares import RuntimeContextMiddleware

    snapshot = build_runtime_context_snapshot("- Session ID: s-mw")
    agent = SimpleNamespace(
        _request_context={"runtime_context_snapshot": snapshot},
        state=SimpleNamespace(context=[_user("hello")]),
    )
    seen: list = []

    async def next_handler(**kwargs):
        del kwargs
        seen.append(list(agent.state.context))
        if False:
            yield None

    mw = RuntimeContextMiddleware(snapshot=snapshot)
    async for _ in mw.on_reply(agent, {}, next_handler):
        pass

    context = seen[0]
    assert context[0].metadata[POTATO_MESSAGE_TAG_KEY] == (
        RUNTIME_CONTEXT_MESSAGE_TAG
    )
    assert last_runtime_context_body(context) == "- Session ID: s-mw"
