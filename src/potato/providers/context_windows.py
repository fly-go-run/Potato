# -*- coding: utf-8 -*-
"""Provider-scoped catalog of known model context windows (input tokens).

The compaction trigger scales with ``model.context_size``
(= ``ModelInfo.max_input_length``), but built-in provider catalogs never set
that field, so every model used to inherit the 128k default — a 1M-context
model compacted exactly like a 128k one. This table supplies real windows
only for official built-in providers and the configured private providers
used by this installation. Model names are not globally portable: a gateway
may expose the same name with a different limit, so an unlisted provider
keeps the default unless it has an explicit per-model override.

:func:`resolve_context_window` is the single resolution entry point — both
the compaction path (``Provider._get_context_size`` → ``model.context_size``)
and the display/usage path (``config.get_model_max_input_length``) go through
it, so what the UI reports and when compression fires can never diverge.
Precedence:

1. an explicit per-model ``max_input_length`` configured by the user;
2. this provider-scoped catalog;
3. :data:`DEFAULT_CONTEXT_WINDOW` (128k).

Values are deliberately CONSERVATIVE: a too-small window merely compacts
earlier, while a too-large one lets the live context grow past what the API
accepts and requests start failing. When a family's window varies by
snapshot, the safe lower documented bound is listed (e.g. ``claude-*`` is
200k — the 1M variant is an opt-in beta header the user can express via a
per-model override).

Matching is case-insensitive substring-at-a-word-boundary within the
selected provider. For example, ``gpt-5.6-luna`` resolves for ``sub2api``
but not for an unrelated provider with the same model id. The longest
pattern wins.
"""

from __future__ import annotations

# The fallback window when nothing else resolves. Also the default of
# ``ModelInfo.max_input_length``. ``ModelInfo.max_input_length_configured``
# keeps an explicit user setting distinguishable from this default, including
# when the user intentionally chooses exactly 128k.
DEFAULT_CONTEXT_WINDOW = 128 * 1024

# (provider id -> (model pattern, max input tokens)). Keep this deliberately
# narrow: official built-ins and configured private providers inherit
# documented cloud windows. Custom providers can still set an explicit
# per-model value in the UI.
_CONTEXT_WINDOWS_BY_PROVIDER: dict[str, tuple[tuple[str, int], ...]] = {
    # The configured sub2api models currently used by Potato are GPT-5.6
    # variants. Do not apply this to every OpenAI-compatible provider.
    "sub2api": (("gpt-5.6", 1_050_000),),
    # Official DeepSeek V4 models expose a 1M input window. The legacy
    # compatibility aliases route to V4 on the current official API.
    "deepseek": (
        ("deepseek-v4", 1_000_000),
        ("deepseek-chat", 1_000_000),
        ("deepseek-reasoner", 1_000_000),
        # V3.2 remains a 128K model.
        ("deepseek-v3.2", 131_072),
    ),
    # Same catalog, served over the Responses API.
    "deepseek-response": (
        ("deepseek-v4", 1_000_000),
        ("deepseek-chat", 1_000_000),
        ("deepseek-reasoner", 1_000_000),
        ("deepseek-v3.2", 131_072),
    ),
    # DashScope and the Aliyun plans expose a curated set of official models.
    # These are intentionally separate from user-configured OpenAI-compatible
    # providers, whose model aliases and limits cannot be inferred safely.
    "dashscope": (
        ("qwen3.7-max", 1_000_000),
        ("qwen3.7-plus", 1_000_000),
        ("qwen3.6-plus", 1_000_000),
        ("deepseek-v4", 1_000_000),
        ("glm-5.2", 1_000_000),
    ),
    "aliyun-tokenplan": (
        ("qwen3.7-max", 1_000_000),
        ("qwen3.7-plus", 1_000_000),
        ("qwen3.6-plus", 1_000_000),
        ("deepseek-v4", 1_000_000),
        ("glm-5.2", 1_000_000),
        ("kimi-k2", 262_144),
    ),
    "aliyun-tokenplan-intl": (
        ("qwen3.7-max", 1_000_000),
        ("qwen3.7-plus", 1_000_000),
        ("qwen3.6-plus", 1_000_000),
        ("deepseek-v4", 1_000_000),
        ("glm-5.2", 1_000_000),
        ("kimi-k2", 262_144),
    ),
    "aliyun-codingplan": (
        ("qwen3.6-plus", 1_000_000),
        ("glm-5.2", 1_000_000),
        ("qwen3-coder-plus", 1_000_000),
        ("qwen3-coder", 262_144),
        ("qwen3-max", 262_144),
        ("kimi-k2", 262_144),
    ),
    "aliyun-codingplan-intl": (
        ("qwen3.6-plus", 1_000_000),
        ("glm-5.2", 1_000_000),
        ("qwen3-coder-plus", 1_000_000),
        ("qwen3-coder", 262_144),
        ("qwen3-max", 262_144),
        ("kimi-k2", 262_144),
    ),
    # Zhipu's current built-in catalog shares the documented GLM-5.2 limit
    # across the regional and coding-plan endpoints.
    "zhipu-cn": (("glm-5.2", 1_000_000),),
    "zhipu-cn-codingplan": (("glm-5.2", 1_000_000),),
    "zhipu-intl": (("glm-5.2", 1_000_000),),
    "zhipu-intl-codingplan": (("glm-5.2", 1_000_000),),
    # OpenAI's official Chat Completions, Responses, and Azure catalogs use
    # the same model-family limits for the models that are bundled here.
    "openai": (
        ("gpt-4.1", 1_047_576),
        ("gpt-5.2", 272_000),
        ("gpt-5-mini", 272_000),
        ("gpt-5-nano", 272_000),
        ("o4-mini", 200_000),
        ("o3", 200_000),
    ),
    "openai-response": (
        ("gpt-4.1", 1_047_576),
        ("gpt-5.2", 272_000),
        ("gpt-5-mini", 272_000),
        ("gpt-5-nano", 272_000),
        ("o4-mini", 200_000),
        ("o3", 200_000),
    ),
    "azure-openai": (
        ("gpt-4.1", 1_047_576),
        ("gpt-5-chat", 272_000),
        ("gpt-5-mini", 272_000),
        ("gpt-5-nano", 272_000),
        ("o4-mini", 200_000),
        ("o3", 200_000),
    ),
    "kimi-cn": (("kimi-k2", 262_144),),
    "kimi-intl": (("kimi-k2", 262_144),),
    # Anthropic's model list is fetched dynamically, so this mapping is the
    # only metadata available before the first successful discovery call.
    "anthropic": (
        ("claude-instant", 100_000),
        ("claude-2", 100_000),
        ("claude", 200_000),
    ),
    "gemini": (
        ("gemini-1.5-pro", 2_097_152),
        ("gemini", 1_048_576),
    ),
}

# A bare model ID must not accidentally claim a longer private variant (for
# example, ``gpt-5`` must not match Potato's ``gpt-5.6``). Keep exact entries
# separate from family patterns so the intended matching rule stays obvious.
_EXACT_CONTEXT_WINDOWS_BY_PROVIDER: dict[str, dict[str, int]] = {
    "openai": {"gpt-5": 272_000},
    "openai-response": {"gpt-5": 272_000},
}

_PATTERNS_BY_PROVIDER: dict[str, tuple[tuple[str, int], ...]] = {
    provider_id: tuple(
        sorted(patterns, key=lambda kv: len(kv[0]), reverse=True),
    )
    for provider_id, patterns in _CONTEXT_WINDOWS_BY_PROVIDER.items()
}


def _matches_at_boundary(model_id: str, pattern: str) -> bool:
    """True if ``pattern`` occurs in ``model_id`` at a word boundary.

    Boundary = start/end of string or a non-alphanumeric character on each
    side, so ``o3`` matches ``o3-mini`` and ``openai/o3`` but not ``o3x`` or
    ``gpt-4o3x``.
    """
    i = model_id.find(pattern)
    while i != -1:
        end = i + len(pattern)
        left_boundary = i == 0 or not model_id[i - 1].isalnum()
        right_boundary = end == len(model_id) or not model_id[end].isalnum()
        if left_boundary and right_boundary:
            return True
        i = model_id.find(pattern, i + 1)
    return False


def known_context_size(
    model_id: str,
    *,
    provider_id: str | None = None,
) -> int | None:
    """The cataloged input-context window for a provider/model pair, or None.

    A provider id is required intentionally. Without it, model names are not
    treated as globally portable and the caller falls back to
    :data:`DEFAULT_CONTEXT_WINDOW`.
    """
    normalized = (model_id or "").lower()
    normalized_provider = (provider_id or "").lower()
    patterns = _PATTERNS_BY_PROVIDER.get(normalized_provider, ())
    if not normalized:
        return None
    exact = _EXACT_CONTEXT_WINDOWS_BY_PROVIDER.get(normalized_provider, {})
    if normalized in exact:
        return exact[normalized]
    if not patterns:
        return None
    for pattern, tokens in patterns:
        if _matches_at_boundary(normalized, pattern):
            return tokens
    return None


def resolve_context_window(
    model_id: str,
    *,
    provider_id: str | None = None,
    configured: int | None = None,
    configured_is_explicit: bool = False,
    use_catalog: bool = True,
) -> int:
    """Resolve a model's input-context window. The single entry point.

    ``configured`` is the model's ``max_input_length`` from user/provider
    config. A value marked by ``configured_is_explicit`` wins outright, even
    when it is exactly :data:`DEFAULT_CONTEXT_WINDOW`. For compatibility with
    existing provider data, any non-default configured value also wins. The
    provider-scoped catalog answers otherwise, unless ``use_catalog`` is False.
    Everything else falls back to the default.
    """
    if configured is not None and (
        configured_is_explicit or configured != DEFAULT_CONTEXT_WINDOW
    ):
        return configured
    if use_catalog:
        known = known_context_size(model_id, provider_id=provider_id)
        if known is not None:
            return known
    return DEFAULT_CONTEXT_WINDOW
