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
import json
from types import SimpleNamespace

import pytest

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
async def test_new_message_during_active_run_is_queued(
    monkeypatch,
) -> None:
    detached: list[tuple[str, asyncio.Queue]] = []

    class Tracker:
        async def get_status(self, run_key):
            assert run_key == "chat-1"
            return "running"

        async def enqueue(self, run_key, payload):
            assert run_key == "chat-1"
            assert payload is not None
            return 1

        async def attach_or_start(  # pylint: disable=unused-argument
            self,
            run_key,
            payload,
            stream_fn,
        ):
            raise AssertionError("should enqueue, not start")

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
    response = await console_router.post_console_chat(request_data, object())

    assert response.status_code == 202
    assert json.loads(response.body) == {"queued": True, "position": 1}
    assert detached == []


@pytest.mark.asyncio
async def test_enqueue_miss_starts_a_new_run(monkeypatch) -> None:
    started: list[object] = []

    class Tracker:
        async def get_status(self, run_key):
            assert run_key == "chat-1"
            return "running"

        async def enqueue(self, run_key, payload):
            assert run_key == "chat-1"
            assert payload is not None
            return None

        async def attach_or_start(self, run_key, payload, stream_fn):
            started.append((run_key, payload, stream_fn))
            return asyncio.Queue(), True

        async def detach_subscriber(self, run_key, subscriber):
            raise AssertionError("new run should keep the subscriber")

        async def stream_from_queue(self, queue, run_key):
            if False:  # pragma: no cover
                yield ""

    channel = SimpleNamespace(
        resolve_session_id=lambda **_kwargs: "console:user-1",
        stream_one=lambda _payload: None,
    )
    workspace = SimpleNamespace(
        channel_manager=SimpleNamespace(get_channel=lambda _name: None),
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
        "input": [{"content": [{"type": "text", "text": "retry"}]}],
    }
    response = await console_router.post_console_chat(request_data, object())

    assert getattr(response, "media_type", None) == "text/event-stream"
    assert len(started) == 1
