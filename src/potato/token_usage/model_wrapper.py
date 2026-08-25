# -*- coding: utf-8 -*-
"""Model wrapper that records token usage from LLM responses."""

import logging
from datetime import date, datetime, timezone
from typing import Any, AsyncGenerator, Literal

from agentscope.model import ChatModelBase
from agentscope.model._model_response import ChatResponse
from agentscope.model._model_usage import ChatUsage

from .buffer import _UsageEvent
from .manager import get_token_usage_manager

logger = logging.getLogger(__name__)


def _usage_get(usage: Any, name: str, default: Any = None) -> Any:
    """Read a field from ChatUsage or a raw SDK usage object.

    AgentScope ``ChatUsage`` is a ``dict`` subclass whose
    ``__getattr__`` is ``dict.__getitem__``. ``getattr(usage, name,
    default)`` therefore raises ``KeyError`` for missing aliases
    instead of returning *default* — that used to fail the whole
    turn after the model had already replied.
    """
    if isinstance(usage, dict):
        return usage.get(name, default)
    try:
        return getattr(usage, name, default)
    except (KeyError, AttributeError, TypeError):
        return default


def _cached_tokens_from_usage(usage: Any) -> int:
    """Read provider cache-hit tokens from a ChatUsage-like object.

    AgentScope maps OpenAI ``cached_tokens``, DeepSeek
    ``prompt_cache_hit_tokens``, and Anthropic cache-read tokens onto
    ``cache_input_tokens``. Fall back through a few aliases so a
    wrapper around a raw SDK usage object still records a hit.
    """
    for attr in (
        "cache_input_tokens",
        "cached_tokens",
        "prompt_cache_hit_tokens",
        "cache_read_input_tokens",
    ):
        parsed = _as_token_count(_usage_get(usage, attr))
        if parsed:
            return parsed
    details = _usage_get(usage, "prompt_tokens_details")
    if details is not None:
        value = _usage_get(details, "cached_tokens")
        parsed = _as_token_count(value)
        if parsed:
            return parsed
    return 0


def _as_token_count(value: Any) -> int:
    if isinstance(value, bool) or value is None:
        return 0
    if isinstance(value, int):
        return value if value > 0 else 0
    if isinstance(value, float):
        return int(value) if value > 0 else 0
    return 0


class TokenRecordingModelWrapper(ChatModelBase):
    """Wraps a ChatModelBase to record token usage on each call."""

    _usage_by_session: dict[str, dict[str, Any]] = {}

    def __init__(
        self,
        provider_id: str,
        model: ChatModelBase,
        compact_threshold: float | None = None,
    ) -> None:
        # agentscope 2.0 ChatModelBase requires credential/model/parameters.
        # Forward the wrapped model's own values so the base attributes stay
        # consistent (some downstream code reads ``self.model`` for logging).
        super().__init__(
            credential=getattr(model, "credential", None),
            model=getattr(model, "model", "unknown"),
            parameters=getattr(model, "parameters", None)
            or ChatModelBase.Parameters(),
            stream=getattr(model, "stream", True),
            context_size=getattr(model, "context_size", 32768),
        )
        self._model = model
        self._provider_id = provider_id
        # Auto-compaction threshold (fraction of the window) for the UI, or
        # None when compaction is disabled/unknown.
        self._compact_threshold = compact_threshold
        # Last local heuristic passed to count_tokens, and the last trusted
        # provider input_tokens paired with that heuristic. Used to calibrate
        # later estimates after the API reports real usage.
        self._last_heuristic = 0
        self._token_anchor: tuple[int, int] | None = None

    def _record_usage(self, usage: ChatUsage | None) -> None:
        """Enqueue a usage event synchronously — never blocks the caller."""
        try:
            self._record_usage_unchecked(usage)
        except Exception:
            # Bookkeeping must not turn a finished reply into a red banner.
            logger.exception("token usage recording failed")

    def _record_usage_unchecked(self, usage: ChatUsage | None) -> None:
        if usage is None:
            return
        pt = _usage_get(usage, "input_tokens", 0) or 0
        ct = _usage_get(usage, "output_tokens", 0) or 0
        cached = _cached_tokens_from_usage(usage)
        if pt <= 0 and ct <= 0:
            return
        if pt > 0 and self._last_heuristic > 0:
            self._token_anchor = (self._last_heuristic, pt)

        event = _UsageEvent(
            provider_id=self._provider_id,
            model_name=self.model,
            prompt_tokens=pt,
            completion_tokens=ct,
            date_str=date.today().isoformat(),
            now_iso=datetime.now(tz=timezone.utc).isoformat(
                timespec="seconds",
            ),
        )
        # Fire-and-forget: synchronous put_nowait, ~100 ns, no await needed.
        get_token_usage_manager().enqueue(event)

        usage_data = {
            "provider_id": self._provider_id,
            "model_name": self.model,
            "prompt_tokens": pt,
            "completion_tokens": ct,
            "cached_tokens": cached,
            "total_tokens": pt + ct,
            # Context window of the wrapped model, so the UI can show how full
            # the *current* context is (prompt_tokens / context_size), distinct
            # from the cumulative session totals. 0 = unknown.
            "context_size": int(getattr(self._model, "context_size", 0) or 0),
            # Auto-compaction threshold (fraction of the window) so the UI can
            # mark where context gets evicted. None = disabled/unknown.
            "compact_threshold": self._compact_threshold,
        }
        self._store_usage(usage_data)

    @classmethod
    def pop_usage_for_session(cls, session_id: str) -> dict[str, Any] | None:
        return cls._usage_by_session.pop(session_id, None)

    def _store_usage(self, usage: dict[str, Any] | None) -> None:
        from ..app.agent_context import get_current_session_id

        session_id = get_current_session_id()
        if session_id and usage:
            TokenRecordingModelWrapper._usage_by_session[session_id] = usage

    async def generate_structured_output(
        self,
        *args: Any,
        **kwargs: Any,
    ) -> Any:
        result = await self._model.generate_structured_output(*args, **kwargs)
        self._record_usage(getattr(result, "usage", None))
        return result

    async def __call__(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
        tool_choice: Literal["auto", "none", "required"] | str | None = None,
        **kwargs: Any,
    ) -> ChatResponse | AsyncGenerator[ChatResponse, None]:
        # agentscope 2.0 routes structured output through
        # ``generate_structured_output`` instead of a ``__call__`` kwarg, and
        # provider SDKs (anthropic, openai) reject unknown kwargs. Drop the
        # 1.x ``structured_model`` if a caller still passes it.
        kwargs.pop("structured_model", None)

        # Fix: Omit tool_choice="auto" for vLLM compatibility
        # vLLM without --enable-auto-tool-choice will reject requests when
        # tool_choice="auto" is present, even if tools are provided.
        # By omitting tool_choice when it's "auto", we bypass the check
        # while keeping tools available for correct tool calling behavior.
        if tool_choice == "auto":
            tool_choice = None

        result = await self._model(
            messages=messages,
            tools=tools,
            tool_choice=tool_choice,
            **kwargs,
        )

        if isinstance(result, AsyncGenerator):
            return self._wrap_stream(result)
        self._record_usage(getattr(result, "usage", None))
        return result

    async def count_tokens(
        self,
        messages: list,
        tools: list | None = None,
    ) -> int:
        """Estimate tokens, calibrated by the last provider usage report."""
        heuristic = await self._model.count_tokens(messages, tools)
        self._last_heuristic = int(heuristic or 0)
        return self._calibrate(self._last_heuristic)

    def _calibrate(self, heuristic: int) -> int:
        """Shift the local heuristic by the last API-reported input count.

        After compaction the live prompt shrinks; drop the stale anchor
        instead of subtracting a huge delta from the previous request.
        """
        if heuristic <= 0:
            return 0
        anchor = self._token_anchor
        if anchor is None:
            return heuristic
        prev_heuristic, prev_api = anchor
        if prev_heuristic <= 0 or prev_api <= 0:
            return heuristic
        if heuristic < int(prev_heuristic * 0.7):
            self._token_anchor = None
            return heuristic
        return max(1, prev_api + (heuristic - prev_heuristic))

    async def _wrap_stream(
        self,
        stream: AsyncGenerator[ChatResponse, None],
    ) -> AsyncGenerator[ChatResponse, None]:
        last_usage: ChatUsage | None = None
        async for chunk in stream:
            if getattr(chunk, "usage", None) is not None:
                last_usage = chunk.usage
            yield chunk
        self._record_usage(last_usage)
