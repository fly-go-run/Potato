# -*- coding: utf-8 -*-
"""Tests for startup configuration migrations."""

from types import SimpleNamespace
from unittest.mock import Mock

from qwenpaw.app import migration


def _profile(workspace_dir: str, enabled: bool = True):
    return SimpleNamespace(workspace_dir=workspace_dir, enabled=enabled)


def _config(profiles: dict[str, object], active_agent: str):
    return SimpleNamespace(
        agents=SimpleNamespace(
            profiles=profiles,
            active_agent=active_agent,
        ),
    )


def test_remove_builtin_qa_agent_profiles_removes_profiles_and_falls_back(
    monkeypatch,
    tmp_path,
) -> None:
    qa_workspace = tmp_path / "qa"
    legacy_workspace = tmp_path / "legacy-qa"
    qa_workspace.mkdir()
    legacy_workspace.mkdir()
    current_qa_id = "QwenPaw_" + "QA_Agent_0.2"
    legacy_qa_id = "CoPaw_" + "QA_Agent_0.1beta1"
    config = _config(
        {
            "default": _profile(str(tmp_path / "default")),
            current_qa_id: _profile(str(qa_workspace)),
            legacy_qa_id: _profile(str(legacy_workspace)),
            "custom": _profile(str(tmp_path / "custom")),
        },
        current_qa_id,
    )
    save_config = Mock()
    monkeypatch.setattr(migration, "load_config", lambda: config)
    monkeypatch.setattr(migration, "save_config", save_config)

    migration.remove_builtin_qa_agent_profiles()

    assert set(config.agents.profiles) == {"default", "custom"}
    assert config.agents.active_agent == "default"
    assert qa_workspace.is_dir()
    assert legacy_workspace.is_dir()
    save_config.assert_called_once_with(config)


def test_remove_builtin_qa_agent_profiles_is_idempotent(
    monkeypatch,
    tmp_path,
) -> None:
    config = _config(
        {"default": _profile(str(tmp_path / "default"))},
        "default",
    )
    save_config = Mock()
    monkeypatch.setattr(migration, "load_config", lambda: config)
    monkeypatch.setattr(migration, "save_config", save_config)

    migration.remove_builtin_qa_agent_profiles()

    assert config.agents.profiles == {
        "default": config.agents.profiles["default"],
    }
    assert config.agents.active_agent == "default"
    save_config.assert_not_called()
