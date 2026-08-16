# -*- coding: utf-8 -*-
"""Responses-API prompt-cache request fields.

``prompt_cache_key`` is a Responses API field. Official OpenAI, Azure,
and third-party relays that terminate on OpenAI all accept or forward
it. Chat Completions hosts (DeepSeek, DashScope, local vLLM, …) do not
use this helper — they never see the field.
"""

from __future__ import annotations


def apply_prompt_cache_key(
    kwargs: dict,
    *,
    session_id: str | None,
) -> None:
    """Set ``prompt_cache_key`` from the current session when unset.

    A caller-supplied key is left untouched. Empty session ids are
    skipped so we never send a blank routing key.
    """
    if "prompt_cache_key" in kwargs:
        return
    key = (session_id or "").strip()
    if key:
        kwargs["prompt_cache_key"] = key
