# -*- coding: utf-8 -*-
"""Unit tests for ``console._extract_placeholder_name``.

The console handler picks an immediate placeholder name for a new chat
from the first content part. Shapes match the agentscope content-block
formats (``{"type": "text", "text": "..."}`` dicts, ``TextBlock``-like
objects with ``.text``, raw strings, and non-text/media blocks). These
tests pin that mapping so a future shape change cannot silently produce
labels like ``{"type": ...`` in the session drawer (regression for PR #3).
"""
from __future__ import annotations

import asyncio
from types import SimpleNamespace

import pytest
from fastapi import HTTPException

import potato.app.routers.console as console_router
from potato.app.routers.console import _extract_placeholder_name


class _TextBlock:
    """Stand-in for an agentscope ``TextBlock`` (object with ``.text``)."""

    def __init__(self, text: str) -> None:
        self.text = text


def test_no_content_parts_returns_new_chat() -> None:
    name, first_text = _extract_placeholder_name([])
    assert name == "New Chat"
    assert first_text == ""


def test_string_content_part() -> None:
    name, first_text = _extract_placeholder_name(["Hello, world!"])
    assert name == "Hello, wor"
    assert first_text == "Hello, world!"


def test_dict_text_block() -> None:
    """``{"type": "text", "text": "..."}`` is the agentscope text block.

    Without the dict-aware branch this would fall through to
    ``str(content)`` and produce a placeholder like ``{'type': ...``.
    """
    parts = [{"type": "text", "text": "What's the weather today?"}]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "What's the"
    assert first_text == "What's the weather today?"


def test_dict_without_text_key_is_treated_as_media() -> None:
    """Image/audio dict blocks lack a ``text`` field and should not produce
    JSON-shaped placeholders."""
    parts = [{"type": "image", "image": {"url": "x.png"}}]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "Media Message"
    assert first_text == ""


def test_dict_with_non_string_text_is_treated_as_media() -> None:
    parts = [{"type": "text", "text": 123}]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "Media Message"
    assert first_text == ""


def test_object_with_text_attribute() -> None:
    parts = [_TextBlock("Plan a trip to Tokyo next week")]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "Plan a tri"
    assert first_text == "Plan a trip to Tokyo next week"


def test_object_with_empty_text_attribute_is_media() -> None:
    parts = [_TextBlock("")]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "Media Message"
    assert first_text == ""


def test_unknown_shape_is_treated_as_media() -> None:
    """Unknown blocks must NOT be ``str(...)``-coerced into a placeholder."""
    parts = [object()]
    name, first_text = _extract_placeholder_name(parts)
    assert name == "Media Message"
    assert first_text == ""


def test_falsy_first_part_is_media() -> None:
    name, first_text = _extract_placeholder_name([None])
    assert name == "Media Message"
    assert first_text == ""


@pytest.mark.asyncio
async def test_new_message_during_active_run_returns_conflict(
    monkeypatch,
) -> None:
    queue: asyncio.Queue = asyncio.Queue()
    detached: list[tuple[str, asyncio.Queue]] = []

    class Tracker:
        async def attach_or_start(  # pylint: disable=unused-argument
            self,
            run_key,
            payload,
            stream_fn,
        ):
            assert run_key == "chat-1"
            return queue, False

        async def detach_subscriber(self, run_key, subscriber):
            detached.append((run_key, subscriber))

    channel = SimpleNamespace(
        resolve_session_id=lambda **_kwargs: "console:user-1",
        stream_one=lambda _payload: None,
    )
    workspace = SimpleNamespace(
        channel_manager=SimpleNamespace(
            get_channel=lambda _name: None,
        ),
        chat_manager=SimpleNamespace(),
        task_tracker=Tracker(),
    )

    async def get_channel(_name):
        return channel

    async def get_or_create_chat(*_args, **_kwargs):
        return SimpleNamespace(id="chat-1", name="Existing chat")

    workspace.channel_manager.get_channel = get_channel
    workspace.chat_manager.get_or_create_chat = get_or_create_chat

    async def get_workspace(_request):
        return workspace

    monkeypatch.setattr(console_router, "get_agent_for_request", get_workspace)

    request_data = {
        "channel": "console",
        "user_id": "user-1",
        "session_id": "session-1",
        "input": [{"content": [{"type": "text", "text": "second"}]}],
    }
    with pytest.raises(HTTPException) as exc_info:
        await console_router.post_console_chat(request_data, object())

    assert exc_info.value.status_code == 409
    assert "reconnect=true" in exc_info.value.detail
    assert detached == [("chat-1", queue)]
