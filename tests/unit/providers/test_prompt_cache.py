# -*- coding: utf-8 -*-
from __future__ import annotations

from potato.providers.prompt_cache import apply_prompt_cache_key


def test_apply_sets_session_key():
    kwargs: dict = {}
    apply_prompt_cache_key(kwargs, session_id="sess-1")
    assert kwargs == {"prompt_cache_key": "sess-1"}


def test_apply_skips_blank_session():
    kwargs: dict = {}
    apply_prompt_cache_key(kwargs, session_id="  ")
    assert kwargs == {}


def test_apply_does_not_override_explicit_key():
    kwargs = {"prompt_cache_key": "manual"}
    apply_prompt_cache_key(kwargs, session_id="sess-1")
    assert kwargs["prompt_cache_key"] == "manual"
