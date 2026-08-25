# -*- coding: utf-8 -*-
from __future__ import annotations

from potato.providers.env_api_key import resolve_provider_api_key


def test_env_key_wins_over_store(monkeypatch):
    monkeypatch.setenv("SUB2API_API_KEY", "from-env")
    assert (
        resolve_provider_api_key("sub2api", stored="from-store") == "from-env"
    )


def test_family_key_openai_preferred_for_gpt(monkeypatch):
    monkeypatch.delenv("SUB2API_API_KEY", raising=False)
    monkeypatch.setenv("SUB2API_OPENAI", "oa")
    monkeypatch.setenv("SUB2API_CLAUDE", "cl")
    assert (
        resolve_provider_api_key("sub2api", model_id="gpt-5.6-terra") == "oa"
    )


def test_family_key_claude_preferred_for_claude(monkeypatch):
    monkeypatch.delenv("SUB2API_API_KEY", raising=False)
    monkeypatch.setenv("SUB2API_OPENAI", "oa")
    monkeypatch.setenv("SUB2API_CLAUDE", "cl")
    assert (
        resolve_provider_api_key("sub2api", model_id="claude-sonnet-4")
        == "cl"
    )


def test_falls_back_to_store(monkeypatch):
    monkeypatch.delenv("SUB2API_API_KEY", raising=False)
    monkeypatch.delenv("SUB2API_OPENAI", raising=False)
    monkeypatch.delenv("SUB2API_CLAUDE", raising=False)
    assert resolve_provider_api_key("sub2api", stored="disk") == "disk"
