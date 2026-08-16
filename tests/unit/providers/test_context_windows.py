# -*- coding: utf-8 -*-
# pylint: disable=protected-access,missing-function-docstring
# pylint: disable=too-few-public-methods,unused-argument
# pylint: disable=unsubscriptable-object
"""The provider-scoped context-window catalog and its wiring into providers.

The compaction trigger is ``trigger_ratio * model.context_size``; before the
catalog every model inherited the 128k ``max_input_length`` default, so a
1M-context model compacted exactly like a 128k one.
"""

from types import SimpleNamespace

import pytest

from potato.providers.context_windows import (
    DEFAULT_CONTEXT_WINDOW,
    known_context_size,
    resolve_context_window,
)
from potato.providers.provider import ModelInfo, Provider


@pytest.mark.parametrize(
    ("provider_id", "model_id", "expected"),
    [
        ("sub2api", "gpt-5.6", 1_050_000),
        ("sub2api", "gpt-5.6-luna", 1_050_000),
        ("deepseek", "deepseek-v4-flash", 1_000_000),
        ("deepseek", "deepseek-v4-pro", 1_000_000),
        ("deepseek", "deepseek-chat", 1_000_000),
        ("deepseek", "deepseek-reasoner", 1_000_000),
        ("deepseek", "deepseek-v3.2", 131_072),
    ],
)
def test_known_windows(provider_id: str, model_id: str, expected: int):
    assert known_context_size(model_id, provider_id=provider_id) == expected


def test_known_windows_are_not_global():
    assert known_context_size("gpt-5.6-luna") is None
    assert known_context_size("gpt-5.6-luna", provider_id="openai") is None
    assert known_context_size("deepseek-v4-pro", provider_id="sub2api") is None


def test_model_family_matching_requires_a_right_boundary():
    assert known_context_size("o3-mini", provider_id="openai") == 200_000
    assert known_context_size("o3x", provider_id="openai") is None
    assert known_context_size("openai/o3x", provider_id="openai") is None


@pytest.mark.parametrize(
    ("provider_id", "model_id", "expected"),
    [
        ("dashscope", "qwen3.7-max", 1_000_000),
        ("aliyun-codingplan", "qwen3-coder-plus", 1_000_000),
        ("zhipu-intl", "glm-5.2", 1_000_000),
        ("openai", "gpt-5", 272_000),
        ("openai", "gpt-4.1-mini", 1_047_576),
        ("openai-response", "o3", 200_000),
        ("deepseek", "deepseek-v4-flash", 1_000_000),
        ("deepseek-response", "deepseek-v4-flash", 1_000_000),
        ("azure-openai", "gpt-5-chat", 272_000),
        ("kimi-cn", "kimi-k2-thinking", 262_144),
        ("anthropic", "claude-sonnet-4-5", 200_000),
        ("gemini", "gemini-2.5-pro", 1_048_576),
    ],
)
def test_official_builtin_windows(
    provider_id: str,
    model_id: str,
    expected: int,
):
    assert known_context_size(model_id, provider_id=provider_id) == expected


# -- resolve_context_window: the single resolution entry point ---------------


def test_resolve_explicit_config_wins():
    assert (
        resolve_context_window(
            "gpt-5.6-luna",
            provider_id="sub2api",
            configured=1_000_000,
        )
        == 1_000_000
    )


def test_resolve_default_valued_config_falls_to_catalog():
    assert (
        resolve_context_window(
            "gpt-5.6-luna",
            provider_id="sub2api",
            configured=DEFAULT_CONTEXT_WINDOW,
        )
        == 1_050_000
    )


def test_resolve_explicit_default_valued_config_wins():
    assert (
        resolve_context_window(
            "gpt-5.6-luna",
            provider_id="sub2api",
            configured=DEFAULT_CONTEXT_WINDOW,
            configured_is_explicit=True,
        )
        == DEFAULT_CONTEXT_WINDOW
    )


def test_resolve_without_catalog_uses_default():
    # Local-serving providers opt out: family windows don't apply.
    assert (
        resolve_context_window("qwen3-coder:30b", use_catalog=False)
        == DEFAULT_CONTEXT_WINDOW
    )
    # But an explicit config still wins.
    assert (
        resolve_context_window(
            "qwen3-coder:30b",
            configured=32_768,
            use_catalog=False,
        )
        == 32_768
    )


def test_resolve_unknown_model_uses_default():
    assert (
        resolve_context_window("totally-unknown-model")
        == DEFAULT_CONTEXT_WINDOW
    )


def test_unknown_model_returns_none():
    assert (
        known_context_size("totally-unknown-model", provider_id="deepseek")
        is None
    )
    assert known_context_size("") is None


def test_unconfigured_provider_uses_default_even_for_known_model_name():
    assert (
        resolve_context_window(
            "gpt-5.6-luna",
            provider_id="openai",
        )
        == DEFAULT_CONTEXT_WINDOW
    )


def test_custom_provider_does_not_inherit_official_windows():
    assert (
        resolve_context_window(
            "gpt-4.1-mini",
            provider_id="my-openai-gateway",
        )
        == DEFAULT_CONTEXT_WINDOW
    )


def test_builtin_provider_resolution_uses_its_scoped_catalog():
    """The static provider catalogs must reach the common resolver path."""
    from potato.providers.provider_manager import (
        PROVIDER_DASHSCOPE,
        PROVIDER_GEMINI,
        PROVIDER_OPENAI,
    )

    assert PROVIDER_DASHSCOPE.get_context_size("qwen3.7-plus") == 1_000_000
    assert PROVIDER_OPENAI.get_context_size("gpt-4.1-mini") == 1_047_576
    assert PROVIDER_GEMINI.get_context_size("gemini-2.5-flash") == 1_048_576


class _CatalogProvider:
    """Minimal stand-in exposing what get_context_size touches.

    Binds the real ``Provider`` methods without instantiating the abstract
    ``Provider`` class.
    """

    _info: ModelInfo | None = None
    id = "sub2api"

    def get_model_info(self, model_id):
        return self._info

    get_context_size = Provider.get_context_size
    _get_context_size = Provider._get_context_size
    _context_catalog_enabled = Provider._context_catalog_enabled


class _MutableCatalogProvider(_CatalogProvider):
    models: list[ModelInfo]
    extra_models: list[ModelInfo]

    update_model_config = Provider.update_model_config


def test_context_size_prefers_explicit_user_config():
    p = _CatalogProvider()
    p._info = ModelInfo(
        id="gpt-5.6-luna",
        name="x",
        max_input_length=1_000_000,
    )
    assert p.get_context_size("gpt-5.6-luna") == 1_000_000


def test_context_size_falls_back_to_catalog_when_default():
    p = _CatalogProvider()
    p._info = ModelInfo(id="gpt-5.6-luna", name="x")  # default 128k
    assert p.get_context_size("gpt-5.6-luna") == 1_050_000


def test_context_size_honors_explicit_128k_user_config():
    p = _CatalogProvider()
    p._info = ModelInfo(
        id="gpt-5.6-luna",
        name="x",
        max_input_length=DEFAULT_CONTEXT_WINDOW,
        max_input_length_configured=True,
    )
    assert p.get_context_size("gpt-5.6-luna") == DEFAULT_CONTEXT_WINDOW


def test_model_config_update_marks_128k_as_explicit():
    p = _MutableCatalogProvider()
    model = ModelInfo(id="gpt-5.6-luna", name="x")
    p.models = [model]
    p.extra_models = []

    assert p.update_model_config(
        model.id,
        {"max_input_length": DEFAULT_CONTEXT_WINDOW},
    )
    assert model.max_input_length_configured is True
    p._info = model
    assert p.get_context_size(model.id) == DEFAULT_CONTEXT_WINDOW


def test_unrelated_model_config_update_keeps_catalog_window():
    p = _MutableCatalogProvider()
    model = ModelInfo(id="gpt-5.6-luna", name="x")
    p.models = [model]
    p.extra_models = []

    assert p.update_model_config(model.id, {"max_tokens": 4096})
    assert model.max_input_length_configured is False
    p._info = model
    assert p.get_context_size(model.id) == 1_050_000


def test_context_size_default_when_unknown_everywhere():
    p = _CatalogProvider()
    p._info = None
    assert (
        p.get_context_size("totally-unknown-model") == DEFAULT_CONTEXT_WINDOW
    )


def test_private_alias_still_works():
    # Providers call self._get_context_size internally; it must stay wired.
    p = _CatalogProvider()
    p._info = ModelInfo(id="gpt-5.6-luna", name="x")
    assert p._get_context_size("gpt-5.6-luna") == 1_050_000


# -- Ollama: local serving opts out of the cloud catalog ----------------------


def _make_ollama(**kw):
    from potato.providers.ollama_provider import OllamaProvider

    return OllamaProvider(
        id="ollama",
        name="Ollama",
        base_url="http://localhost:11434",
        api_key="EMPTY",
        chat_model="OpenAIChatModel",
        **kw,
    )


def test_ollama_skips_catalog():
    """A local qwen3-coder:30b must NOT get the family's cloud 262k — the
    local serve truncates at num_ctx, so assuming a huge window would
    disable compression while the server drops the prompt head."""
    provider = _make_ollama()
    assert (
        provider.get_context_size("qwen3-coder:30b") == DEFAULT_CONTEXT_WINDOW
    )


def test_ollama_explicit_config_still_wins():
    provider = _make_ollama(
        models=[
            ModelInfo(
                id="qwen3-coder:30b",
                name="qwen3-coder",
                max_input_length=32_768,
            ),
        ],
    )
    assert provider.get_context_size("qwen3-coder:30b") == 32_768


# -- OpenRouter: the API's context_length is authoritative --------------------


def _openrouter_payload(*rows):
    return SimpleNamespace(data=list(rows))


def test_openrouter_reads_context_length():
    from potato.providers.openrouter_provider import OpenRouterProvider

    payload = _openrouter_payload(
        SimpleNamespace(
            id="anthropic/claude-sonnet-4.5",
            name="Claude Sonnet 4.5",
            pricing=None,
            context_length=1_000_000,
        ),
        SimpleNamespace(  # absent → field default → catalog resolves
            id="mistralai/mistral-large",
            name="Mistral Large",
            pricing=None,
        ),
        SimpleNamespace(  # invalid → ignored
            id="foo/bar",
            name="Bar",
            pricing=None,
            context_length="not-a-number",
        ),
    )
    models = {
        m.id: m for m in OpenRouterProvider._normalize_models_payload(payload)
    }
    assert models["anthropic/claude-sonnet-4.5"].max_input_length == 1_000_000
    assert (
        models["mistralai/mistral-large"].max_input_length
        == DEFAULT_CONTEXT_WINDOW
    )
    assert models["foo/bar"].max_input_length == DEFAULT_CONTEXT_WINDOW


# -- config display path resolves through the SAME provider method -----------


def test_get_model_max_input_length_uses_provider_resolution(monkeypatch):
    """/history, usage%%, and daemon status must report the same window the
    compaction trigger uses — the display path delegates to
    Provider.get_context_size instead of reading the raw field."""
    from potato.config import config as config_mod

    class _Provider:
        def get_context_size(self, model_id):
            assert model_id == "claude-sonnet-4-5"
            return 200_000

    class _Manager:
        def get_provider(self, provider_id):
            return _Provider()

    monkeypatch.setattr(
        "potato.providers.ProviderManager.get_instance",
        staticmethod(_Manager),
    )
    agent_config = SimpleNamespace(
        id="agent-1",
        active_model=SimpleNamespace(
            provider_id="anthropic",
            model="claude-sonnet-4-5",
        ),
    )
    assert config_mod.get_model_max_input_length(agent_config) == 200_000
