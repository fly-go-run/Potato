# -*- coding: utf-8 -*-
"""Unit tests for src/potato/plugins/download_catalog.py."""

from __future__ import annotations

from potato.plugins.download_catalog import _is_entry_compatible


def test_entry_with_potato_version_compatible() -> None:
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": {"min": "1.1.6", "max": "2.1.0"},
    }
    assert _is_entry_compatible(entry) is True


def test_entry_with_potato_version_incompatible() -> None:
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": {"min": "0.1.0", "max": "1.1.0"},
    }
    assert _is_entry_compatible(entry) is False


def test_entry_with_only_min_compatible() -> None:
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": {"min": "2.0.0"},
    }
    assert _is_entry_compatible(entry) is True


def test_entry_with_only_min_incompatible() -> None:
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": {"min": "3.0.0"},
    }
    assert _is_entry_compatible(entry) is False


def test_entry_without_version_constraints_is_compatible() -> None:
    entry = {
        "id": "demo",
        "version": "1.0.0",
    }
    assert _is_entry_compatible(entry) is True


def test_entry_with_malformed_potato_version_falls_to_legacy() -> None:
    """Non-dict potato_version falls back to min_version/max_version."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": "not-a-dict",
        "min_version": "1.0.0",
        "max_version": "2.1.0",
    }
    assert _is_entry_compatible(entry) is True


def test_entry_with_malformed_potato_version_no_legacy() -> None:
    """Non-dict potato_version with no legacy fields is compatible."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": "not-a-dict",
    }
    assert _is_entry_compatible(entry) is True


def test_legacy_min_version_compatible() -> None:
    """Legacy min_version within range is compatible."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "min_version": "2.0.0",
    }
    assert _is_entry_compatible(entry) is True


def test_legacy_min_version_incompatible() -> None:
    """Legacy min_version above current Potato is incompatible."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "min_version": "3.0.0",
    }
    assert _is_entry_compatible(entry) is False


def test_legacy_min_max_version_compatible() -> None:
    """Legacy min+max range covering current Potato is compatible."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "min_version": "1.0.0",
        "max_version": "2.1.0",
    }
    assert _is_entry_compatible(entry) is True


def test_legacy_min_max_version_incompatible() -> None:
    """Legacy min+max range not covering current Potato."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "min_version": "0.1.0",
        "max_version": "1.0.0",
    }
    assert _is_entry_compatible(entry) is False


def test_entry_with_empty_dict_potato_version() -> None:
    """Empty dict potato_version triggers compat check with defaults."""
    entry = {
        "id": "demo",
        "version": "1.0.0",
        "potato_version": {},
    }
    # Empty dict is isinstance(dict) but has no min/max, should
    # still pass through PluginManifest validation or fallback gracefully
    result = _is_entry_compatible(entry)
    assert isinstance(result, bool)
