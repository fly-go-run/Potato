# -*- coding: utf-8 -*-
"""Compatibility helpers for the CoPaw -> QwenPaw -> Potato rename."""

from __future__ import annotations

import os


def install_legacy_env_aliases() -> None:
    """Expose legacy environment variables through the Potato namespace.

    Potato variables always win. QwenPaw is the immediate predecessor and
    CoPaw is the oldest supported prefix. The reverse QwenPaw alias keeps
    third-party plugins that still read ``QWENPAW_*`` directly working during
    the compatibility window.
    """

    snapshot = tuple(os.environ.items())
    for key, value in snapshot:
        if key.startswith("QWENPAW_"):
            os.environ.setdefault("POTATO_" + key[len("QWENPAW_") :], value)
        elif key.startswith("COPAW_"):
            os.environ.setdefault("POTATO_" + key[len("COPAW_") :], value)

    for key, value in tuple(os.environ.items()):
        if key.startswith("POTATO_"):
            os.environ.setdefault("QWENPAW_" + key[len("POTATO_") :], value)
