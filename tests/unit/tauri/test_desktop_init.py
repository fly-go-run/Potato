# -*- coding: utf-8 -*-
from __future__ import annotations

from potato.tauri.desktop_init import ensure_minimal_desktop_config


def test_ensure_minimal_desktop_config_writes_once(tmp_path):
    created = ensure_minimal_desktop_config(tmp_path)
    config_path = tmp_path / "config.json"

    assert created is True
    assert config_path.read_text(encoding="utf-8") == "{}\n"
    assert ensure_minimal_desktop_config(tmp_path) is False
    assert config_path.read_text(encoding="utf-8") == "{}\n"


def test_ensure_minimal_desktop_config_creates_parent(tmp_path):
    target = tmp_path / "nested" / "working"
    assert ensure_minimal_desktop_config(target) is True
    assert (target / "config.json").is_file()
