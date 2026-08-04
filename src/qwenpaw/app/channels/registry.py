# -*- coding: utf-8 -*-
"""Channel registry: built-in + plugin-registered channels."""

from __future__ import annotations

import importlib
import logging
import threading
from typing import TYPE_CHECKING

from .base import BaseChannel

if TYPE_CHECKING:
    pass

logger = logging.getLogger(__name__)

_BUILTIN_SPECS: dict[str, tuple[str, str]] = {
    "imessage": (".imessage", "IMessageChannel"),
    "discord": (".discord_", "DiscordChannel"),
    "dingtalk": (".dingtalk", "DingTalkChannel"),
    "feishu": (".feishu", "FeishuChannel"),
    "qq": (".qq", "QQChannel"),
    "telegram": (".telegram", "TelegramChannel"),
    "mattermost": (".mattermost", "MattermostChannel"),
    "mqtt": (".mqtt", "MQTTChannel"),
    "console": (".console", "ConsoleChannel"),
    "matrix": (".matrix", "MatrixChannel"),
    "slack": (".slack", "SlackChannel"),
    "voice": (".voice", "VoiceChannel"),
    "sip": (".sip", "SIPChannel"),
    "wecom": (".wecom", "WecomChannel"),
    "xiaoyi": (".xiaoyi", "XiaoYiChannel"),
    "yuanbao": (".yuanbao", "YuanbaoChannel"),
    "wechat": (".wechat", "WeChatChannel"),
    "onebot": (".onebot", "OneBotChannel"),
}

# Required channels must load; failures are raised, not skipped.
_REQUIRED_CHANNEL_KEYS: frozenset[str] = frozenset({"console"})

# Per-key cache: value is the loaded class, or None when the module failed
# to import (missing optional dependency). Populated lazily so that desktop
# startup never pays the import cost of channel SDKs nobody enabled
# (lark_oapi alone is thousands of generated modules).
_BUILTIN_CLASS_CACHE: dict[str, type[BaseChannel] | None] = {}
# RLock: the lock is held across module imports, and an imported channel
# module (or something it pulls in) may itself resolve a channel class.
_BUILTIN_CHANNEL_CACHE_LOCK = threading.RLock()


def _load_builtin_channel(key: str) -> type[BaseChannel] | None:
    """Import a single built-in channel class.

    A single optional dependency failure should not break CLI startup.
    """
    module_name, class_name = _BUILTIN_SPECS[key]
    try:
        mod = importlib.import_module(module_name, package=__package__)
        cls = getattr(mod, class_name)
        if not (
            isinstance(cls, type)
            and issubclass(cls, BaseChannel)
            and cls is not BaseChannel
        ):
            raise TypeError(
                f"{module_name}.{class_name} is not a BaseChannel subtype",
            )
    except Exception:
        if key in _REQUIRED_CHANNEL_KEYS:
            logger.error(
                'failed to load required built-in channel "%s"',
                key,
                exc_info=True,
            )
            raise
        logger.debug(
            "built-in channel unavailable: %s",
            key,
            exc_info=True,
        )
        return None
    return cls


def _get_builtin_channel_class(key: str) -> type[BaseChannel] | None:
    """Return one cached built-in channel class, importing on first use."""
    if key not in _BUILTIN_SPECS:
        return None
    with _BUILTIN_CHANNEL_CACHE_LOCK:
        if key not in _BUILTIN_CLASS_CACHE:
            _BUILTIN_CLASS_CACHE[key] = _load_builtin_channel(key)
        return _BUILTIN_CLASS_CACHE[key]


def _get_cached_builtin_channels() -> dict[str, type[BaseChannel]]:
    """Return all built-in channels, importing any not yet loaded."""
    out: dict[str, type[BaseChannel]] = {}
    for key in _BUILTIN_SPECS:
        cls = _get_builtin_channel_class(key)
        if cls is not None:
            out[key] = cls
    return out


def clear_builtin_channel_cache() -> None:
    """Reset built-in channel cache. Primarily for tests."""
    with _BUILTIN_CHANNEL_CACHE_LOCK:
        _BUILTIN_CLASS_CACHE.clear()


BUILTIN_CHANNEL_KEYS = frozenset(_BUILTIN_SPECS.keys())


def _get_plugin_channels() -> dict[str, type[BaseChannel]]:
    """Return channel classes registered via the plugin system."""
    try:
        from ...plugins.registry import PluginRegistry

        registry = PluginRegistry()
        return {
            key: reg.channel_class
            for key, reg in registry.get_registered_channels().items()
        }
    except ImportError:
        logger.debug("plugin channel discovery skipped (not installed)")
        return {}
    except Exception:
        logger.warning(
            "plugin channel discovery failed",
            exc_info=True,
        )
        return {}


def get_channel_registry() -> dict[str, type[BaseChannel]]:
    """Built-in + plugin-registered channels.

    Eagerly imports every built-in channel module. Prefer
    ``get_channel_keys()`` + ``get_channel_class()`` on startup paths so
    unconfigured channel SDKs are never imported.
    """
    out = _get_cached_builtin_channels()
    for key, ch_cls in _get_plugin_channels().items():
        if key in out:
            logger.warning(
                "Plugin channel '%s' skipped: key already exists in "
                "built-in channels",
                key,
            )
            continue
        out[key] = ch_cls
    return out


def get_channel_keys() -> tuple[str, ...]:
    """All channel keys (built-in + plugin) without importing SDK modules."""
    keys = list(_BUILTIN_SPECS)
    for key in _get_plugin_channels():
        if key not in _BUILTIN_SPECS:
            keys.append(key)
    return tuple(keys)


def get_channel_class(key: str) -> type[BaseChannel] | None:
    """Resolve one channel class, importing its module on first use.

    Returns ``None`` when the key is unknown or the channel's optional
    dependencies are unavailable.
    """
    cls = _get_builtin_channel_class(key)
    if cls is not None:
        return cls
    if key in _BUILTIN_SPECS:
        # Built-in key whose module failed to import; a plugin must not
        # shadow it.
        return None
    return _get_plugin_channels().get(key)
