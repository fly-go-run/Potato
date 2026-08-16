# -*- coding: utf-8 -*-
"""Unit tests for the composer voice-input endpoints.

Covers the readiness report and the up-front "this audio needs ffmpeg and
there is none" rejection — the packaged desktop app does not inherit a
login shell PATH, so that used to surface as a generic transcription
failure nobody could diagnose.
"""
# pylint: disable=redefined-outer-name
from __future__ import annotations

import io

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

import potato.app.routers.workspace as workspace_router


@pytest.fixture
def client() -> TestClient:
    app = FastAPI()
    app.include_router(workspace_router.router, prefix="/api")
    return TestClient(app)


def _configure(monkeypatch, provider_type: str) -> None:
    """Point ``load_config`` at the requested transcription provider."""
    config = workspace_router.load_config()
    config.agents.transcription_provider_type = provider_type
    monkeypatch.setattr(workspace_router, "load_config", lambda: config)


class TestSpeechStatus:
    def test_reports_credentials_and_ffmpeg(self, client, monkeypatch):
        _configure(monkeypatch, "doubao_asr")
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.has_doubao_credentials",
            lambda: True,
        )
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.ffmpeg_available",
            lambda: False,
        )

        body = client.get("/api/workspace/speech-status").json()

        assert body["doubao_credentials_configured"] is True
        assert body["ffmpeg_available"] is False
        # 录音器直接产 wav,缺 ffmpeg 不影响语音输入可用。
        assert body["ready"] is True

    def test_not_ready_without_credentials(self, client, monkeypatch):
        _configure(monkeypatch, "doubao_asr")
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.has_doubao_credentials",
            lambda: False,
        )
        body = client.get("/api/workspace/speech-status").json()
        assert body["ready"] is False


class TestTranscribeConversionGuard:
    def _upload(self, client, filename: str):
        return client.post(
            "/api/workspace/transcribe",
            files={"file": (filename, io.BytesIO(b"\x00" * 64), "audio/wav")},
        )

    def test_rejects_webm_when_ffmpeg_is_missing(self, client, monkeypatch):
        _configure(monkeypatch, "doubao_asr")
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.has_doubao_credentials",
            lambda: True,
        )
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.ffmpeg_available",
            lambda: False,
        )

        resp = self._upload(client, "voice.webm")

        assert resp.status_code == 400
        assert resp.json()["detail"]["code"] == "AUDIO_CONVERSION_UNAVAILABLE"

    def test_wav_is_not_gated_on_ffmpeg(self, client, monkeypatch):
        _configure(monkeypatch, "doubao_asr")
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.has_doubao_credentials",
            lambda: True,
        )
        monkeypatch.setattr(
            "potato.agents.utils.doubao_asr.ffmpeg_available",
            lambda: False,
        )

        async def fake_transcribe(_path: str) -> str:
            return "你好"

        monkeypatch.setattr(
            "potato.agents.utils.audio_transcription.transcribe_audio",
            fake_transcribe,
        )

        resp = self._upload(client, "voice.wav")

        assert resp.status_code == 200
        assert resp.json()["text"] == "你好"
