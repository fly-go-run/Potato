# -*- coding: utf-8 -*-
# pylint: disable=protected-access
"""Tests for lazy channel registry loading."""
import sys

import pytest

from qwenpaw.app.channels import registry
from qwenpaw.app.channels.base import BaseChannel


@pytest.fixture(autouse=True)
def _fresh_cache():
    registry.clear_builtin_channel_cache()
    yield
    registry.clear_builtin_channel_cache()


class TestChannelKeys:
    def test_keys_cover_all_builtin_specs(self):
        keys = registry.get_channel_keys()
        assert set(registry._BUILTIN_SPECS) <= set(keys)

    def test_keys_do_not_import_sdk_modules(self):
        """Key listing must never pay channel SDK import cost."""
        for mod in list(sys.modules):
            if mod.startswith("lark_oapi"):
                del sys.modules[mod]
        registry.get_channel_keys()
        assert not any(m.startswith("lark_oapi") for m in sys.modules)


class TestChannelClass:
    def test_resolves_console_channel(self):
        cls = registry.get_channel_class("console")
        assert isinstance(cls, type)
        assert issubclass(cls, BaseChannel)

    def test_unknown_key_returns_none(self):
        assert registry.get_channel_class("no-such-channel") is None

    def test_import_failure_cached_as_none(self, monkeypatch):
        registry._BUILTIN_SPECS["broken"] = (".broken", "BrokenChannel")
        try:
            assert registry.get_channel_class("broken") is None
            # Cached: a second resolve must not re-import.
            assert registry._BUILTIN_CLASS_CACHE["broken"] is None
            assert registry.get_channel_class("broken") is None
        finally:
            registry._BUILTIN_SPECS.pop("broken", None)

    def test_registry_and_lazy_resolution_agree(self):
        eager = registry.get_channel_registry()
        for key, cls in eager.items():
            assert registry.get_channel_class(key) is cls


class TestLoadableChannels:
    def test_loadable_is_subset_of_available(self):
        from qwenpaw.config.utils import (
            get_available_channels,
            get_loadable_channels,
        )

        available = get_available_channels()
        loadable = get_loadable_channels()
        assert set(loadable) <= set(available)
        assert "console" in loadable
