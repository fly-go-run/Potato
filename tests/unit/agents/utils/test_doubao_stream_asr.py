# -*- coding: utf-8 -*-
"""Unit tests for Doubao streaming ASR framing helpers."""
# pylint: disable=protected-access

import gzip
import json

from potato.agents.utils import doubao_stream_asr as mod


def _empty_stream_config():
    class Agents:
        transcription_doubao_stream_resource_id = ""

    class Cfg:
        agents = Agents()

    return Cfg()


class TestResolveStreamResource:
    def test_default(self, monkeypatch):
        monkeypatch.delenv("POTATO_SPEECH_STREAM_RESOURCE_ID", raising=False)
        monkeypatch.delenv(
            "VOLCENGINE_SPEECH_STREAM_RESOURCE_ID",
            raising=False,
        )
        monkeypatch.setattr(
            "potato.config.load_config",
            _empty_stream_config,
        )
        assert (
            mod.resolve_stream_resource_id() == "volc.seedasr.sauc.duration"
        )

    def test_env_override(self, monkeypatch):
        monkeypatch.setenv(
            "POTATO_SPEECH_STREAM_RESOURCE_ID",
            "volc.seedasr.sauc.concurrent",
        )
        monkeypatch.setattr(
            "potato.config.load_config",
            lambda: type(
                "Cfg",
                (),
                {
                    "agents": type(
                        "A",
                        (),
                        {"transcription_doubao_stream_resource_id": ""},
                    )(),
                },
            )(),
        )
        assert (
            mod.resolve_stream_resource_id() == "volc.seedasr.sauc.concurrent"
        )


class TestBuildStreamHeaders:
    def test_new_console_key(self):
        headers = mod.build_stream_headers(
            "sk-x",
            "",
            resource_id="volc.seedasr.sauc.duration",
            connect_id="cid-1",
        )
        assert headers["X-Api-Key"] == "sk-x"
        assert "X-Api-App-Key" not in headers
        assert headers["X-Api-Resource-Id"] == "volc.seedasr.sauc.duration"
        assert headers["X-Api-Connect-Id"] == "cid-1"


class TestFraming:
    def test_full_client_roundtrip_shape(self):
        frame = mod.encode_full_client_request()
        assert frame[0] == 0x11
        assert (frame[1] >> 4) == mod.MSG_FULL_CLIENT
        size = int.from_bytes(frame[4:8], "big")
        payload = gzip.decompress(frame[8:8 + size])
        body = json.loads(payload.decode("utf-8"))
        assert body["audio"]["format"] == "pcm"
        assert body["request"]["model_name"] == "bigmodel"

    def test_audio_last_flag(self):
        frame = mod.encode_audio(b"\x00\x01", last=True)
        assert (frame[1] >> 4) == mod.MSG_AUDIO_ONLY
        assert (frame[1] & 0x0F) == mod.FLAG_LAST

    def test_decode_response_and_extract(self):
        body = {
            "result": {
                "text": "今天天气",
                "utterances": [{"text": "今天天气", "definite": False}],
            }
        }
        payload = gzip.compress(json.dumps(body).encode("utf-8"))
        header = bytes([0x11, 0x91, 0x11, 0x00])
        seq = (1).to_bytes(4, "big")
        size = len(payload).to_bytes(4, "big")
        parsed = mod.decode_frame(header + seq + size + payload)
        assert parsed["kind"] == "response"
        assert parsed["final"] is False
        text, definite = mod.extract_stream_text(parsed["body"])
        assert text == "今天天气"
        assert definite is False

    def test_extract_definite_utterance(self):
        text, definite = mod.extract_stream_text(
            {
                "result": {
                    "text": "好的。",
                    "utterances": [{"text": "好的。", "definite": True}],
                }
            }
        )
        assert text == "好的。"
        assert definite is True
