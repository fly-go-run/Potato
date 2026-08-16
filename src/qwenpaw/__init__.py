# -*- coding: utf-8 -*-
"""Compatibility import namespace for the former QwenPaw package.

New code should import :mod:`potato`. Keeping the old package path lets
installed plugins transition without forcing an atomic ecosystem upgrade.
"""

from __future__ import annotations

import potato as _potato

__path__ = _potato.__path__
__version__ = getattr(_potato, "__version__", None)


def __getattr__(name: str):
    return getattr(_potato, name)
