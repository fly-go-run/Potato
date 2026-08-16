# -*- coding: utf-8 -*-
"""Regression coverage for the shared pytest filesystem isolation."""

from pathlib import Path


def test_potato_default_storage_is_not_the_user_home() -> None:
    """Business-module imports during unit tests must use a tmp runtime."""
    from potato.constant import SECRET_DIR, WORKING_DIR

    assert WORKING_DIR != Path.home() / ".potato"
    assert SECRET_DIR != Path.home() / ".potato.secret"
