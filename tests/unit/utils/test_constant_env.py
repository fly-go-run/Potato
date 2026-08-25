# -*- coding: utf-8 -*-
from __future__ import annotations

from pathlib import Path

from potato.constant import (
    is_working_data_home,
    resolve_secret_dir,
    resolve_working_dir,
    user_env_file_paths,
)
from potato.user_home import MIGRATION_MARKER


def test_user_env_canonical_path_is_potato_home():
    paths = user_env_file_paths()
    assert paths[0] == Path.home() / ".potato" / ".env"
    assert Path.home() / ".qwenpaw" / ".env" in paths


def test_env_only_potato_dir_is_not_a_data_home(tmp_path: Path):
    potato = tmp_path / ".potato"
    potato.mkdir()
    (potato / ".env").write_text("EXA_API_KEY=x\n", encoding="utf-8")
    assert is_working_data_home(potato) is False


def test_potato_dir_with_config_is_a_data_home(tmp_path: Path):
    potato = tmp_path / ".potato"
    potato.mkdir()
    (potato / "config.json").write_text("{}\n", encoding="utf-8")
    assert is_working_data_home(potato) is True


def test_potato_dir_with_workspaces_is_a_data_home(tmp_path: Path):
    potato = tmp_path / ".potato"
    (potato / "workspaces").mkdir(parents=True)
    assert is_working_data_home(potato) is True


def test_new_install_uses_only_potato_home(tmp_path: Path):
    home = tmp_path / "home"
    home.mkdir()
    assert resolve_working_dir(home=home) == (home / ".potato").resolve()
    assert resolve_secret_dir(home / ".potato") == (
        home / ".potato.secret"
    ).resolve()


def test_old_install_is_copied_then_new_path_wins(tmp_path: Path):
    home = tmp_path / "home"
    qwenpaw = home / ".qwenpaw"
    potato = home / ".potato"
    secret = home / ".qwenpaw.secret"
    (qwenpaw / "workspaces" / "default").mkdir(parents=True)
    (qwenpaw / "config.json").write_text(
        '{"language":"zh"}\n',
        encoding="utf-8",
    )
    (qwenpaw / "workspaces" / "default" / "chats.json").write_text(
        "[]\n",
        encoding="utf-8",
    )
    (qwenpaw / "venv" / "bin").mkdir(parents=True)
    (qwenpaw / "venv" / "bin" / "python").write_text("x", encoding="utf-8")
    (qwenpaw / "desktop.log").write_text("old log", encoding="utf-8")
    (secret / "providers").mkdir(parents=True)
    (secret / "envs.json").write_text("{}\n", encoding="utf-8")
    (secret / ".master_key").write_text("ab" * 32, encoding="utf-8")
    potato.mkdir()
    (potato / ".env").write_text("EXA_API_KEY=keep\n", encoding="utf-8")

    chosen = resolve_working_dir(home=home)
    assert chosen == potato.resolve()
    assert (potato / "config.json").read_text(encoding="utf-8") == (
        '{"language":"zh"}\n'
    )
    assert (potato / "workspaces" / "default" / "chats.json").is_file()
    env_text = (potato / ".env").read_text(encoding="utf-8")
    assert env_text == "EXA_API_KEY=keep\n"
    assert not (potato / "venv").exists()
    assert not (potato / "desktop.log").exists()
    assert (potato / MIGRATION_MARKER).is_file()
    assert (home / ".potato.secret" / "envs.json").is_file()
    assert (home / ".potato.secret" / ".master_key").read_text(
        encoding="utf-8",
    ) == "ab" * 32
    assert (qwenpaw / "config.json").is_file()
    assert resolve_secret_dir(chosen) == (home / ".potato.secret").resolve()

    again = resolve_working_dir(home=home)
    assert again == potato.resolve()


def test_migrate_failure_keeps_legacy_home(tmp_path: Path, monkeypatch):
    home = tmp_path / "home"
    qwenpaw = home / ".qwenpaw"
    (qwenpaw / "config.json").parent.mkdir(parents=True)
    (qwenpaw / "config.json").write_text("{}\n", encoding="utf-8")

    def _boom(*_args, **_kwargs):
        raise OSError("disk full")

    monkeypatch.setattr("potato.user_home._copy_tree", _boom)
    chosen = resolve_working_dir(home=home)
    assert chosen == qwenpaw.resolve()


def test_empty_leftover_qwenpaw_dir_does_not_trap_new_install(tmp_path: Path):
    home = tmp_path / "home"
    (home / ".qwenpaw").mkdir(parents=True)
    assert resolve_working_dir(home=home) == (home / ".potato").resolve()


def test_explicit_working_dir_wins(tmp_path: Path):
    home = tmp_path / "home"
    other = tmp_path / "elsewhere"
    other.mkdir()
    (home / ".qwenpaw" / "config.json").parent.mkdir(parents=True)
    (home / ".qwenpaw" / "config.json").write_text("{}\n", encoding="utf-8")
    chosen = resolve_working_dir(home=home, explicit=str(other))
    assert chosen == other.resolve()
