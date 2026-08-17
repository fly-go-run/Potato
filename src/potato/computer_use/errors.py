# -*- coding: utf-8 -*-
"""Computer-use errors that tools turn into model-visible JSON."""

from __future__ import annotations


class ComputerUseError(Exception):
    """User-facing computer-use failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
