# -*- coding: utf-8 -*-
"""Potato Pet Desktop runtime."""

import os

# The desktop package can run independently of Potato, so install the
# QwenPaw-era environment aliases here as well as in potato.__init__.
for _key, _value in tuple(os.environ.items()):
    if _key.startswith("QWENPAW_"):
        os.environ.setdefault(
            "POTATO_" + _key[len("QWENPAW_") :],
            _value,
        )

__version__ = "0.1.0"
