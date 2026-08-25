# -*- coding: utf-8 -*-
"""Read provider API keys from the process environment.

``~/.potato/.env`` is loaded at startup. Users can put gateway keys
there instead of the encrypted provider store.
"""
from __future__ import annotations

import os
import re

_WELL_KNOWN: dict[str, tuple[str, ...]] = {
    "openai": ("OPENAI_API_KEY",),
    "anthropic": ("ANTHROPIC_API_KEY",),
    "dashscope": ("DASHSCOPE_API_KEY",),
    "gemini": ("GEMINI_API_KEY", "GOOGLE_API_KEY"),
    "openrouter": ("OPENROUTER_API_KEY",),
    "deepseek": ("DEEPSEEK_API_KEY",),
    "kimi": ("KIMI_API_KEY", "MOONSHOT_API_KEY"),
}


def provider_env_ident(provider_id: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "_", provider_id).strip("_").upper()


def _model_family(model_id: str | None) -> str | None:
    name = (model_id or "").lower()
    if any(
        token in name
        for token in ("claude", "anthropic", "sonnet", "haiku", "opus")
    ):
        return "CLAUDE"
    if any(
        token in name for token in ("gpt", "o1", "o3", "o4", "openai")
    ):
        return "OPENAI"
    return None


def resolve_provider_api_key(
    provider_id: str,
    stored: str = "",
    model_id: str | None = None,
) -> str:
    """Env wins over the encrypted store. Never log the value."""
    ident = provider_env_ident(provider_id)
    names = [
        f"{ident}_API_KEY",
        f"POTATO_{ident}_API_KEY",
        *list(_WELL_KNOWN.get(provider_id.lower(), ())),
    ]
    family = _model_family(model_id)
    family_names = [f"{ident}_CLAUDE", f"{ident}_OPENAI"]
    if family == "OPENAI":
        family_names = [f"{ident}_OPENAI", f"{ident}_CLAUDE"]
    names.extend(family_names)

    for name in names:
        value = (os.environ.get(name) or "").strip()
        if value:
            return value
    return (stored or "").strip()
