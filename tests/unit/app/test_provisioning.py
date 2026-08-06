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

from qwenpaw.app import provisioning


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
            "qwenpaw.envs.set_env_var",
            lambda key, value: seeded.update({key: value}),
        )
        _write(tmp_path, {"envs": {"apikey": "sk-voice", "keyid": "app-1"}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert seeded == {"apikey": "sk-voice", "keyid": "app-1"}
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
            "qwenpaw.envs.set_env_var",
            lambda key, value: seeded.update({key: value}),
        )
        _write(tmp_path, {"envs": {"  ": "x", "keyid": None, "ok": "1"}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert seeded == {"ok": "1"}


class TestTranscriptionSetting:
    async def test_enables_the_requested_backend(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        config = MagicMock()
        saved: list = []
        monkeypatch.setattr("qwenpaw.config.load_config", lambda: config)
        monkeypatch.setattr("qwenpaw.config.save_config", saved.append)
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
        monkeypatch.setattr("qwenpaw.config.save_config", saved.append)
        _write(tmp_path, {"active": {"provider_id": "", "model": ""}})

        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert not saved


class TestFailureHandling:
    async def test_keeps_the_file_for_retry_when_applying_fails(
        self,
        manager,
        monkeypatch,
        tmp_path,
    ):
        def boom(_key, _value):
            raise RuntimeError("secret store unavailable")

        monkeypatch.setattr("qwenpaw.envs.set_env_var", boom)
        _write(tmp_path, {"envs": {"apikey": "sk-voice"}})

        # 不能抛:预配置失败绝不能挡住应用启动。
        await provisioning.apply_provision_file(manager, "default", tmp_path)

        assert (tmp_path / provisioning.PROVISION_FILE).exists()
        assert not (tmp_path / provisioning.APPLIED_FILE).exists()
