# -*- coding: utf-8 -*-
"""Guard tests for the composer streaming transcription socket."""
# pylint: disable=redefined-outer-name
from __future__ import annotations

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

import potato.app.routers.transcribe_stream as stream_router
import potato.app.routers.workspace as workspace_router


@pytest.fixture
def client() -> TestClient:
    app = FastAPI()
    app.include_router(stream_router.router, prefix="/api")
    return TestClient(app)


def _configure(monkeypatch, provider_type: str) -> None:
    config = workspace_router.load_config()
    config.agents.transcription_provider_type = provider_type
    monkeypatch.setattr(stream_router, "load_config", lambda: config)


class TestTranscribeStreamGuards:
    def test_disabled(self, client, monkeypatch):
        _configure(monkeypatch, "disabled")
        path = "/api/workspace/transcribe-stream"
        with client.websocket_connect(path) as ws:
            body = ws.receive_json()
        assert body["type"] == "error"
        assert body["code"] == "TRANSCRIPTION_DISABLED"

    def test_missing_key(self, client, monkeypatch):
        _configure(monkeypatch, "doubao_asr")
        monkeypatch.setattr(
            stream_router,
            "has_doubao_credentials",
            lambda: False,
        )
        path = "/api/workspace/transcribe-stream"
        with client.websocket_connect(path) as ws:
            body = ws.receive_json()
        assert body["type"] == "error"
        assert body["code"] == "SPEECH_API_KEY_MISSING"
