# -*- coding: utf-8 -*-
"""Token usage tracking for LLM API calls."""

from .buffer import _UsageEvent
from .manager import (
    TokenUsageByModel,
    TokenUsageRecord,
    TokenUsageStats,
    TokenUsageSummary,
    get_token_usage_manager,
)
from .turn_usage import (
    TURN_USAGE_META_KEY,
    fmt_tokens,
    persist_turn_usage,
)


def __getattr__(name: str):
    # TokenRecordingModelWrapper imports agentscope.model (and through it
    # the whole tool/MCP stack). Load it on first use so the desktop backend
    # does not pay that on the startup path.
    if name == "TokenRecordingModelWrapper":
        from .model_wrapper import TokenRecordingModelWrapper

        return TokenRecordingModelWrapper
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    "TokenUsageByModel",
    "TokenUsageRecord",
    "TokenUsageStats",
    "TokenUsageSummary",
    "get_token_usage_manager",
    "TokenRecordingModelWrapper",
    "_UsageEvent",
    "fmt_tokens",
    "TURN_USAGE_META_KEY",
    "persist_turn_usage",
]
