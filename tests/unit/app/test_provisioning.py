# -*- coding: utf-8 -*-
"""Unit tests for first-run provisioning.

Focus on the parts a preconfigured installer needs for voice input: the
speech credentials are only ever read from the environment, and a packaged
build has no ``.env`` the recipient can reach nor an env editor in
settings — so provisioning is the only delivery path.
"""
# pylint: disable=redefined-outer-name
from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import MagicMock

import pytest

from potato.app import provisioning


@pytest.fixture
def manager() -> MagicMock:
    """A ProviderManager stub: nothing in this suite touches providers."""
    stub = MagicMock(name="ProviderManager")
    stub.get_provider.return_value = None
    return stub


def _write(tmp_path: Path, payload: dict) -> Path:
    path = tmp_path / provisioning.PROVISION_FILE
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


class TestEnvSeeding:
    async def test_seeds_speech_credentials(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        seeded: dict[str, str] = {}
        monkeypatch.setattr(
            "potato.envs.set_env_var",
            lambda key, value: seeded.update({key: value}),
        )
        _write(tmp_path, {"envs": {"apikey": "sk-voice", "keyid": "app-1"}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert seeded == {
            "VOLCENGINE_SPEECH_API_KEY": "sk-voice",
            "VOLCENGINE_SPEECH_APP_ID": "app-1",
        }
        # 应用成功后归档,重启不会重复种。
        assert (tmp_path / provisioning.APPLIED_FILE).exists()
        assert not (tmp_path / provisioning.PROVISION_FILE).exists()

    async def test_skips_blank_names_and_null_values(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        seeded: dict[str, str] = {}
        monkeypatch.setattr(
            "potato.envs.set_env_var",
            lambda key, value: seeded.update({key: value}),
        )
        _write(tmp_path, {"envs": {"  ": "x", "keyid": None, "ok": "1"}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert seeded == {"ok": "1"}

    async def test_overwrites_old_dotenv_values(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        dest = tmp_path / ".potato" / ".env"
        dest.parent.mkdir()
        dest.write_text(
            "VOLCENGINE_SPEECH_API_KEY=old\nUNRELATED=keep\n",
            encoding="utf-8",
        )
        monkeypatch.setattr(
            "potato.envs.set_env_var",
            lambda _key, _value: None,
        )
        monkeypatch.setattr(
            "potato.envs.dotenv_file.potato_dotenv_path",
            lambda: dest,
        )
        monkeypatch.setattr(
            "potato.envs.dotenv_file.should_touch_user_dotenv",
            lambda: True,
        )
        _write(tmp_path, {"envs": {"apikey": "new"}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        from potato.envs.dotenv_file import parse_dotenv

        assert parse_dotenv(dest) == {
            "UNRELATED": "keep",
            "VOLCENGINE_SPEECH_API_KEY": "new",
        }


class TestTranscriptionSetting:
    async def test_enables_the_requested_backend(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        config = MagicMock()
        saved: list = []
        monkeypatch.setattr("potato.config.load_config", lambda: config)
        monkeypatch.setattr("potato.config.save_config", saved.append)
        _write(tmp_path, {"transcription_provider_type": "doubao_asr"})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        # 语音默认是 disabled,不在这里打开的话按钮永远不会出现。
        assert config.agents.transcription_provider_type == "doubao_asr"
        assert saved == [config]

    async def test_leaves_the_setting_alone_when_absent(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        saved: list = []
        monkeypatch.setattr("potato.config.save_config", saved.append)
        _write(tmp_path, {"active": {"provider_id": "", "model": ""}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert not saved


class TestProviderOverwrite:
    async def test_existing_provider_key_wins_over_old_dotenv(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        manager.get_provider.return_value = object()
        dest = tmp_path / ".potato" / ".env"
        dest.parent.mkdir()
        dest.write_text("SUB2API_API_KEY=old\n", encoding="utf-8")
        monkeypatch.setattr(
            "potato.envs.dotenv_file.potato_dotenv_path",
            lambda: dest,
        )
        monkeypatch.setattr(
            "potato.envs.dotenv_file.should_touch_user_dotenv",
            lambda: True,
        )
        monkeypatch.setenv("SUB2API_API_KEY", "old")
        _write(
            tmp_path,
            {
                "custom_providers": [
                    {
                        "id": "sub2api",
                        "api_key": "new-provider-key",
                    },
                ],
            },
        )

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        manager.update_provider.assert_called_once_with(
            "sub2api",
            {"api_key": "new-provider-key"},
        )
        from potato.envs.dotenv_file import parse_dotenv

        assert parse_dotenv(dest)["SUB2API_API_KEY"] == "new-provider-key"
        assert __import__("os").environ["SUB2API_API_KEY"] == (
            "new-provider-key"
        )


class TestFailureHandling:
    async def test_keeps_the_file_for_retry_when_applying_fails(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        def boom(_key, _value):
            raise RuntimeError("secret store unavailable")

        monkeypatch.setattr("potato.envs.set_env_var", boom)
        _write(tmp_path, {"envs": {"apikey": "sk-voice"}})

        # 不能抛:预配置失败绝不能挡住应用启动。
        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert (tmp_path / provisioning.PROVISION_FILE).exists()
        assert not (tmp_path / provisioning.APPLIED_FILE).exists()
