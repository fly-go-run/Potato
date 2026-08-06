# -*- coding: utf-8 -*-
# pylint: disable=protected-access
from __future__ import annotations

import logging
from datetime import datetime
from types import SimpleNamespace
from typing import Any

import openai
import pytest
from agentscope.model import OpenAIResponseModel

from qwenpaw.providers.openai_response_provider import (
    OpenAIResponseProvider,
    ResponsesStreamError,
)
from qwenpaw.providers.provider import ModelInfo


def _provider(models: list[ModelInfo] | None = None) -> OpenAIResponseProvider:
    return OpenAIResponseProvider(
        id="openai-response",
        name="OpenAI Responses",
        base_url="https://api.openai.com/v1",
        api_key="sk-test",
        chat_model="OpenAIResponseModel",
        models=models or [],
    )


class _FakeResponses:
    """Records the kwargs that would hit ``POST /v1/responses``."""

    def __init__(self, captured: dict) -> None:
        self._captured = captured

    async def create(self, **kwargs: Any) -> Any:
        self._captured.clear()
        self._captured.update(kwargs)
        return _events([])


def _fake_client(captured: dict):
    def factory(**_: Any) -> Any:
        return SimpleNamespace(responses=_FakeResponses(captured))

    return factory


async def _events(items: list[Any]):
    for item in items:
        yield item


async def test_summary_limit_is_adapted_for_responses_api(
    monkeypatch,
) -> None:
    captured: dict = {}

    async def fake_call_api(self, *args, **kwargs):
        del self, args
        captured.update(kwargs)
        return "ok"

    monkeypatch.setattr(OpenAIResponseModel, "_call_api", fake_call_api)
    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")

    result = await model._call_api(
        "gpt-5",
        [],
        max_tokens=256,
        disable_thinking=True,
    )

    assert result == "ok"
    assert captured["max_output_tokens"] == 256
    assert "max_tokens" not in captured


async def test_reasoning_effort_is_sent_as_reasoning_block(
    monkeypatch,
) -> None:
    """``responses.create`` has no ``reasoning_effort`` keyword at all."""
    captured: dict = {}
    monkeypatch.setattr(openai, "AsyncClient", _fake_client(captured))
    provider = _provider(
        [ModelInfo(id="gpt-5", name="GPT-5", reasoning_effort="high")],
    )
    model = provider.get_chat_model_instance("gpt-5")

    await model._call_api("gpt-5", [])

    assert captured["reasoning"] == {"effort": "high"}
    assert "reasoning_effort" not in captured


async def test_provider_specific_effort_level_survives(monkeypatch) -> None:
    """Levels outside agentscope's Literal (e.g. DeepSeek's "max") pass."""
    captured: dict = {}
    monkeypatch.setattr(openai, "AsyncClient", _fake_client(captured))
    provider = _provider(
        [ModelInfo(id="ds", name="DeepSeek", reasoning_effort="max")],
    )
    model = provider.get_chat_model_instance("ds")

    await model._call_api("ds", [])

    assert captured["reasoning"] == {"effort": "max"}


async def test_disable_thinking_suppresses_reasoning(monkeypatch) -> None:
    captured: dict = {}
    monkeypatch.setattr(openai, "AsyncClient", _fake_client(captured))
    provider = _provider(
        [ModelInfo(id="gpt-5", name="GPT-5", reasoning_effort="high")],
    )
    model = provider.get_chat_model_instance("gpt-5")

    await model._call_api("gpt-5", [], disable_thinking=True)

    # ``NOT_GIVEN`` is the SDK sentinel for "leave the key out of the body";
    # an explicit ``None`` would serialize as ``"reasoning": null``.
    assert captured["reasoning"] is openai.NOT_GIVEN


async def test_chat_completions_kwargs_are_demoted_to_extra_body(
    monkeypatch,
) -> None:
    captured: dict = {}
    monkeypatch.setattr(openai, "AsyncClient", _fake_client(captured))
    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")

    await model._call_api(
        "gpt-5",
        [],
        frequency_penalty=0.5,
        temperature=0.2,
    )

    assert captured["extra_body"] == {"frequency_penalty": 0.5}
    assert "frequency_penalty" not in captured
    # Keywords the Responses API does accept stay at the top level.
    assert captured["temperature"] == 0.2


async def test_explicit_extra_body_wins_over_demoted_keys(
    monkeypatch,
) -> None:
    captured: dict = {}
    monkeypatch.setattr(openai, "AsyncClient", _fake_client(captured))
    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")

    await model._call_api(
        "gpt-5",
        [],
        frequency_penalty=0.5,
        extra_body={"frequency_penalty": 0.9, "custom": 1},
    )

    assert captured["extra_body"] == {"frequency_penalty": 0.9, "custom": 1}


async def test_failed_stream_raises_instead_of_returning_empty() -> None:
    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")
    failed = SimpleNamespace(
        type="response.failed",
        response=SimpleNamespace(
            error=SimpleNamespace(code="server_error", message="boom"),
        ),
    )

    stream = model._parse_stream_response(datetime.now(), _events([failed]))

    with pytest.raises(ResponsesStreamError, match="boom"):
        async for _ in stream:
            pass


async def test_incomplete_stream_warns_but_does_not_raise(caplog) -> None:
    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")
    incomplete = SimpleNamespace(
        type="response.incomplete",
        response=SimpleNamespace(
            incomplete_details=SimpleNamespace(reason="max_output_tokens"),
        ),
    )

    stream = model._parse_stream_response(
        datetime.now(),
        _events([incomplete]),
    )

    with caplog.at_level(logging.WARNING):
        async for _ in stream:
            pass

    assert "max_output_tokens" in caplog.text


async def test_failed_stream_maps_transient_codes_to_status() -> None:
    """So ``retry_chat_model`` can retry / report a 429 like any other."""
    from qwenpaw.providers.retry_chat_model import _extract_status_code

    provider = _provider()
    model = provider.get_chat_model_instance("gpt-5")
    failed = SimpleNamespace(
        type="response.failed",
        response=SimpleNamespace(
            error=SimpleNamespace(
                code="rate_limit_exceeded",
                message="slow down",
            ),
        ),
    )

    stream = model._parse_stream_response(datetime.now(), _events([failed]))

    with pytest.raises(ResponsesStreamError) as excinfo:
        async for _ in stream:
            pass
    assert _extract_status_code(excinfo.value) == 429


async def test_model_probe_rejects_a_stream_that_fails_after_200(
    monkeypatch,
) -> None:
    """Gateways answer 200 first and only then emit ``response.failed``."""
    failed = SimpleNamespace(
        type="response.failed",
        response=SimpleNamespace(
            error=SimpleNamespace(code="server_error", message="nope"),
        ),
    )

    class _Stream:
        def __init__(self) -> None:
            self.closed = False

        def __aiter__(self):
            return _events(
                [SimpleNamespace(type="response.created"), failed],
            )

        async def close(self) -> None:
            self.closed = True

    stream = _Stream()

    class _Responses:
        async def create(self, **_: Any) -> Any:
            return stream

    monkeypatch.setattr(
        OpenAIResponseProvider,
        "_client",
        lambda self, timeout=5: SimpleNamespace(responses=_Responses()),
    )
    provider = _provider()

    ok, message = await provider.check_model_connection("gpt-5")

    assert ok is False
    assert "nope" in message
    assert stream.closed is True


async def test_check_connection_probes_responses_when_model_known(
    monkeypatch,
) -> None:
    """Endpoints that only expose POST /responses must still verify."""
    provider = _provider([ModelInfo(id="gpt-5", name="GPT-5")])
    probed: list[str] = []

    async def fake_check_model_connection(self, model_id, timeout=5):
        del self, timeout
        probed.append(model_id)
        return True, ""

    monkeypatch.setattr(
        OpenAIResponseProvider,
        "check_model_connection",
        fake_check_model_connection,
    )

    assert await provider.check_connection() == (True, "")
    assert probed == ["gpt-5"]
