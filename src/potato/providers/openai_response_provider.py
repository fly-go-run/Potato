# -*- coding: utf-8 -*-
"""An OpenAI Responses API provider implementation."""

from __future__ import annotations

import inspect
import logging
from typing import Any, AsyncIterator

from agentscope.model import OpenAIResponseModel

from .capping_formatter import _CappingOpenAIResponseFormatter
from .openai_provider import OpenAIProvider

logger = logging.getLogger(__name__)

# Mirrors ``AsyncResponses.create`` as of openai-python 2.33.  Only used
# when signature introspection fails, so a future SDK layout change
# degrades to "a few extra keys land in extra_body" instead of breaking
# every request.
_FALLBACK_RESPONSES_PARAMS = frozenset(
    {
        "background",
        "context_management",
        "conversation",
        "extra_body",
        "extra_headers",
        "extra_query",
        "include",
        "input",
        "instructions",
        "max_output_tokens",
        "max_tool_calls",
        "metadata",
        "model",
        "parallel_tool_calls",
        "previous_response_id",
        "prompt",
        "prompt_cache_key",
        "prompt_cache_retention",
        "reasoning",
        "safety_identifier",
        "service_tier",
        "store",
        "stream",
        "stream_options",
        "temperature",
        "text",
        "timeout",
        "tool_choice",
        "tools",
        "top_logprobs",
        "top_p",
        "truncation",
        "user",
    },
)

_responses_params_cache: frozenset[str] | None = None


def _responses_create_params() -> frozenset[str]:
    """Return the keyword names ``responses.create`` actually accepts.

    The OpenAI SDK's ``create`` is fully typed with no ``**kwargs``, so any
    unknown keyword raises ``TypeError`` before a request is ever sent.
    """
    global _responses_params_cache  # pylint: disable=global-statement
    if _responses_params_cache is None:
        try:
            from openai.resources.responses import AsyncResponses

            names = {
                param.name
                for param in inspect.signature(
                    AsyncResponses.create,
                ).parameters.values()
            }
            names.discard("self")
            _responses_params_cache = frozenset(names)
        except Exception:  # pragma: no cover - defensive
            _responses_params_cache = _FALLBACK_RESPONSES_PARAMS
    return _responses_params_cache


def _demote_unsupported_kwargs(kwargs: dict[str, Any]) -> dict[str, Any]:
    """Move Chat-Completions-style keywords into ``extra_body``.

    Provider ``generate_kwargs`` are shared across protocols, so a user who
    configured ``frequency_penalty``/``stop``/``response_format`` for the
    Chat Completions API would otherwise crash every Responses API call
    with a ``TypeError``.  Demoting keeps the request alive *and* still
    forwards the field, which third-party gateways may well understand.
    """
    allowed = _responses_create_params()
    demoted = {key: kwargs[key] for key in kwargs if key not in allowed}
    if not demoted:
        return kwargs

    result = {key: value for key, value in kwargs.items() if key in allowed}
    body = dict(result.get("extra_body") or {})
    for key, value in demoted.items():
        body.setdefault(key, value)
    result["extra_body"] = body
    logger.debug(
        "Responses API: moved unsupported kwargs into extra_body: %s",
        sorted(demoted),
    )
    return result


# Responses API error codes report failures that the HTTP layer already
# classifies as transient elsewhere.  Mapping them back to a status code is
# what lets ``retry_chat_model`` retry them and notify the rate limiter.
_ERROR_CODE_STATUS = {
    "rate_limit_exceeded": 429,
    "server_error": 500,
}


class ResponsesStreamError(RuntimeError):
    """A Responses API stream ended with ``response.failed`` / ``error``.

    agentscope's stream parser only reacts to ``response.completed``, so
    without this the caller sees an empty answer instead of an error.
    """

    def __init__(self, message: str, body: dict | None = None) -> None:
        super().__init__(message)
        # ``retry_chat_model._extract_status_code`` reads ``body`` to decide
        # whether a failure is transient.
        self.body = body


async def guarded_response_stream(response: Any) -> AsyncIterator[Any]:
    """Re-yield Responses API stream events, raising on terminal failures.

    A Responses stream ends with ``response.completed``,
    ``response.incomplete`` or ``response.failed`` — there is no ``[DONE]``
    sentinel.  Only the first is handled by agentscope's parser, so the
    other two would otherwise look like a silent empty answer.
    """
    async for event in response:
        event_type = getattr(event, "type", "") or ""
        if event_type in ("response.failed", "error"):
            error = getattr(
                getattr(event, "response", None),
                "error",
                None,
            ) or getattr(event, "error", event)
            message = (
                getattr(error, "message", None)
                or str(error)
                or "Responses API stream failed"
            )
            code = getattr(error, "code", None)
            payload: dict[str, Any] = {"code": code, "message": message}
            status = _ERROR_CODE_STATUS.get(str(code))
            if status is not None:
                payload["status_code"] = status
            raise ResponsesStreamError(
                f"Responses API stream failed: {message}",
                body={"error": payload},
            )
        if event_type == "response.incomplete":
            reason = getattr(
                getattr(
                    getattr(event, "response", None),
                    "incomplete_details",
                    None,
                ),
                "reason",
                "unknown",
            )
            logger.warning(
                "Responses API stream ended incomplete (reason=%s); "
                "the answer is truncated",
                reason,
            )
        yield event


class OpenAIResponseModelCompat(OpenAIResponseModel):
    """OpenAIResponseModel with extra-kwargs injection and tool schema
    sanitization.

    * ``extra_generate_kwargs`` — merged into every ``_call_api`` call
      (provider-level kwargs like ``extra_body``).
    * ``_format_tools`` — sanitizes boolean JSON Schema values that strict
      providers reject (same fix as ``OpenAIChatModelCompat``).
    * unknown keywords are demoted into ``extra_body`` instead of raising.
    * terminal ``response.failed`` events are surfaced as exceptions.
    """

    def __init__(
        self,
        *,
        extra_generate_kwargs: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        self._extra_generate_kwargs = extra_generate_kwargs or {}
        super().__init__(**kwargs)

    async def _call_api(
        self,
        model_name: str,
        messages: Any,
        tools: list[dict] | None = None,
        tool_choice: Any | None = None,
        **generate_kwargs: Any,
    ) -> Any:
        disable_thinking = bool(generate_kwargs.pop("disable_thinking", False))
        max_tokens = generate_kwargs.pop("max_tokens", None)
        if (
            max_tokens is not None
            and "max_output_tokens" not in generate_kwargs
        ):
            generate_kwargs["max_output_tokens"] = max_tokens
        merged = {**self._extra_generate_kwargs, **generate_kwargs}
        from ..app.agent_context import get_current_session_id
        from .prompt_cache import apply_prompt_cache_key

        apply_prompt_cache_key(
            merged,
            session_id=get_current_session_id(),
        )
        if disable_thinking:
            # Internal utility calls opt out of reasoning.  ``NOT_GIVEN``
            # drops the key from the request body entirely; an explicit
            # ``None`` would serialize as ``"reasoning": null``, which
            # strict gateways reject.  Effort values are not universally
            # shared across providers, so suppressing beats guessing at a
            # "none" level the endpoint may not know.
            from openai import NOT_GIVEN

            merged["reasoning"] = NOT_GIVEN
        merged = _demote_unsupported_kwargs(merged)
        return await super()._call_api(
            model_name,
            messages,
            tools,
            tool_choice,
            **merged,
        )

    def _format_tools(
        self,
        tools: list[dict] | None,
        tool_choice: Any | None,
    ) -> tuple[list[dict] | None, Any]:
        from .openai_chat_model_compat import _sanitize_tool_schemas

        if tools:
            tools = _sanitize_tool_schemas(tools)
        return super()._format_tools(tools, tool_choice)

    def _parse_stream_response(
        self,
        start_datetime: Any,
        response: Any,
    ) -> Any:
        """Delegate to the base parser over a failure-aware event stream."""
        return super()._parse_stream_response(
            start_datetime,
            guarded_response_stream(response),
        )


class OpenAIResponseProvider(OpenAIProvider):
    """Provider that uses the OpenAI Responses API instead of Chat Completions.

    Inherits connection/discovery logic from ``OpenAIProvider`` but
    creates ``OpenAIResponseModel`` instances via ``get_chat_model_instance``.
    The ``check_model_connection`` method uses the Responses API endpoint.

    Multimodal probing (``_probe_image_support`` / ``_probe_video_support``)
    is inherited from ``OpenAIProvider`` and uses the Chat Completions
    endpoint.  This works for OpenAI (which supports both APIs) and fails
    gracefully (returns "probe inconclusive") for third-party providers
    that only expose the Response API.
    """

    async def check_connection(self, timeout: float = 5) -> tuple[bool, str]:
        """Check reachability without assuming ``GET /models`` exists.

        Responses-only endpoints — notably codex's ``responses-api-proxy``,
        which forwards ``POST /v1/responses`` and 403s everything else —
        would fail the inherited model-listing probe even when perfectly
        usable.  Probe the actual protocol whenever a model is known.
        """
        model_id = next(
            (
                model.id
                for model in (*self.models, *self.extra_models)
                if model.id.strip()
            ),
            "",
        )
        if model_id:
            return await self.check_model_connection(model_id, timeout)

        ok, msg = await super().check_connection(timeout)
        if ok:
            return True, ""
        return (
            False,
            f"{msg} (this endpoint may only expose POST /responses — "
            "add a model manually and test again)",
        )

    async def check_model_connection(
        self,
        model_id: str,
        timeout: float = 5,
    ) -> tuple[bool, str]:
        """Check if a model is reachable via the Responses API."""
        from openai import APIError

        model_id = (model_id or "").strip()
        if not model_id:
            return False, "Empty model ID"

        try:
            client = self._client(timeout=timeout)
            res = await client.responses.create(
                model=model_id,
                input="ping",
                timeout=timeout,
                max_output_tokens=20,
                stream=True,
            )
            # Read the whole probe stream (bounded to 20 output tokens)
            # rather than stopping at the first event: gateways commonly
            # answer 200 and only then emit ``response.failed``, which a
            # stop-at-``response.created`` probe would report as success.
            try:
                async for _ in guarded_response_stream(res):
                    pass
            finally:
                await res.close()
            return True, ""
        except ResponsesStreamError as exc:
            return False, str(exc)
        except APIError as exc:
            detail = str(exc) or getattr(exc, "message", "")
            return (
                False,
                f"API error when connecting to model '{model_id}': {detail}",
            )
        except Exception:
            return (
                False,
                f"Unknown exception when connecting to model '{model_id}'",
            )

    def get_chat_model_instance(self, model_id: str) -> Any:
        from agentscope.credential import OpenAICredential

        credential = OpenAICredential(
            id=f"potato-{self.id}",
            api_key=self.api_key,
            base_url=self.base_url,
        )

        merged_headers = self._build_default_headers()
        gen_kwargs = self.get_effective_generate_kwargs(model_id)
        self._apply_thinking_config(model_id, gen_kwargs)
        # The inherited hook writes a Chat-Completions-style
        # ``reasoning_effort``; the Responses API takes ``reasoning.effort``
        # instead and ``responses.create`` has no such keyword at all.
        # Translate here rather than through ``Parameters`` so
        # provider-specific levels (e.g. DeepSeek's "max") are not rejected
        # by agentscope's stricter Literal.
        reasoning_effort = gen_kwargs.pop("reasoning_effort", None)
        if reasoning_effort:
            gen_kwargs.setdefault("reasoning", {"effort": reasoning_effort})
        parameters = OpenAIResponseModel.Parameters(
            max_tokens=gen_kwargs.pop("max_tokens", None),
            temperature=gen_kwargs.pop("temperature", None),
        )

        client_kwargs: dict[str, Any] = {}
        if merged_headers:
            client_kwargs["default_headers"] = merged_headers

        return OpenAIResponseModelCompat(
            credential=credential,
            model=model_id,
            parameters=parameters,
            stream=True,
            context_size=self._get_context_size(model_id),
            client_kwargs=client_kwargs,
            extra_generate_kwargs=gen_kwargs or None,
            formatter=_CappingOpenAIResponseFormatter(
                max_bytes=self.max_inline_media_bytes,
            ),
        )
