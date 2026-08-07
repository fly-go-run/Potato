# -*- coding: utf-8 -*-
"""Unit tests for Doubao / OpenSpeech flash ASR helper."""
# pylint: disable=protected-access

from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from qwenpaw.agents.utils import doubao_asr as mod


class TestResolveCredentials:
    def test_missing(self, monkeypatch):
        for key in (
            "VOLCENGINE_SPEECH_API_KEY",
            "QWENPAW_SPEECH_API_KEY",
            "apikey",
            "APIKEY",
            "keyid",
            "KEYID",
            "VOLCENGINE_SPEECH_APP_ID",
            "QWENPAW_SPEECH_APP_ID",
        ):
            monkeypatch.delenv(key, raising=False)
        assert mod.resolve_speech_credentials() is None
        assert mod.has_doubao_credentials() is False

    def test_single_key(self, monkeypatch):
        monkeypatch.setenv("apikey", "sk-test")
        monkeypatch.delenv("keyid", raising=False)
        assert mod.resolve_speech_credentials() == ("sk-test", "")
        assert mod.has_doubao_credentials() is True

    def test_legacy_pair(self, monkeypatch):
        monkeypatch.setenv("apikey", "token")
        monkeypatch.setenv("keyid", "app-1")
        assert mod.resolve_speech_credentials() == ("token", "app-1")


class TestBuildAuthHeaders:
    def test_new_console_key(self):
        headers = mod.build_auth_headers(
            "sk-x",
            "",
            resource_id="volc.bigasr.auc_turbo",
            request_id="rid-1",
        )
        assert headers["X-Api-Key"] == "sk-x"
        assert "X-Api-App-Key" not in headers
        assert headers["X-Api-Resource-Id"] == "volc.bigasr.auc_turbo"
        assert headers["X-Api-Request-Id"] == "rid-1"

    def test_legacy_pair(self):
        headers = mod.build_auth_headers(
            "token",
            "app-1",
            request_id="rid-2",
        )
        assert headers["X-Api-App-Key"] == "app-1"
        assert headers["X-Api-Access-Key"] == "token"
        assert "X-Api-Key" not in headers


class TestExtractText:
    def test_nested_result(self):
        assert mod._extract_text({"result": {"text": "  你好  "}}) == "你好"

    def test_top_level(self):
        assert mod._extract_text({"text": "hi"}) == "hi"

    def test_empty(self):
        assert mod._extract_text({}) == ""
        assert mod._extract_text({"result": {}}) == ""


class TestTranscribeFlash:
    @pytest.mark.asyncio
    async def test_missing_credentials(self, monkeypatch):
        monkeypatch.delenv("apikey", raising=False)
        monkeypatch.delenv("VOLCENGINE_SPEECH_API_KEY", raising=False)
        monkeypatch.delenv("QWENPAW_SPEECH_API_KEY", raising=False)
        result = await mod.transcribe_doubao_flash("/tmp/a.wav")
        assert result is None

    @pytest.mark.asyncio
    async def test_success(self, monkeypatch, tmp_path):
        monkeypatch.setenv("apikey", "sk-test")
        monkeypatch.delenv("keyid", raising=False)
        audio = tmp_path / "a.wav"
        audio.write_bytes(b"RIFF" + b"\x00" * 32)

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.headers = {
            "X-Api-Status-Code": "20000000",
            "X-Api-Message": "OK",
            "X-Tt-Logid": "log-1",
        }
        mock_response.json.return_value = {
            "result": {"text": "今天天气不错"},
        }

        mock_client = AsyncMock()
        mock_client.__aenter__.return_value = mock_client
        mock_client.__aexit__.return_value = None
        mock_client.post = AsyncMock(return_value=mock_response)

        with patch.object(mod.httpx, "AsyncClient", return_value=mock_client):
            text = await mod.transcribe_doubao_flash(str(audio))

        assert text == "今天天气不错"
        mock_client.post.assert_awaited_once()
        call_kwargs = mock_client.post.await_args
        headers = call_kwargs.kwargs["headers"]
        assert headers["X-Api-Key"] == "sk-test"
        body = call_kwargs.kwargs["json"]
        assert "data" in body["audio"]
        assert body["audio"]["format"] == "wav"


class TestAudioFormatForPath:
    def test_wav_mp3_ogg(self):
        assert mod.audio_format_for_path("/tmp/a.wav") == "wav"
        assert mod.audio_format_for_path("/tmp/a.mp3") == "mp3"
        assert mod.audio_format_for_path("/tmp/a.ogg") == "ogg"

    def test_m4a_maps_to_mp4(self):
        assert mod.audio_format_for_path("/tmp/a.m4a") == "mp4"

    def test_unknown_defaults_wav(self):
        assert mod.audio_format_for_path("/tmp/a.bin") == "wav"

    @pytest.mark.asyncio
    async def test_business_error(self, monkeypatch, tmp_path):
        monkeypatch.setenv("apikey", "sk-test")
        audio = tmp_path / "a.wav"
        audio.write_bytes(b"\x00" * 16)

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.headers = {
            "X-Api-Status-Code": "45000000",
            "X-Api-Message": "auth failed",
            "X-Tt-Logid": "log-2",
        }
        mock_response.json.return_value = {}

        mock_client = AsyncMock()
        mock_client.__aenter__.return_value = mock_client
        mock_client.__aexit__.return_value = None
        mock_client.post = AsyncMock(return_value=mock_response)

        with patch.object(mod.httpx, "AsyncClient", return_value=mock_client):
            text = await mod.transcribe_doubao_flash(str(audio))
        assert text is None

    @pytest.mark.asyncio
    async def test_no_speech_is_empty_text_not_failure(
        self,
        monkeypatch,
        tmp_path,
    ):
        """录到静音 => 空转写,不是失败。

        用户没开口是正常结果。以前这里和「鉴权失败」一样返回 None,路由
        据此抛 500,界面就弹出「转写失败,请检查供应商配置和日志」——把一次
        没说话说成配置有问题,还要人去翻日志。
        """
        monkeypatch.setenv("apikey", "sk-test")
        audio = tmp_path / "a.wav"
        audio.write_bytes(b"\x00" * 16)

        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.headers = {
            "X-Api-Status-Code": mod.NO_SPEECH_STATUS,
            "X-Api-Message": "[Normal silence audio] no valid speech in audio",
            "X-Tt-Logid": "log-3",
        }
        mock_response.json.return_value = {}

        mock_client = AsyncMock()
        mock_client.__aenter__.return_value = mock_client
        mock_client.__aexit__.return_value = None
        mock_client.post = AsyncMock(return_value=mock_response)

        with patch.object(mod.httpx, "AsyncClient", return_value=mock_client):
            text = await mod.transcribe_doubao_flash(str(audio))
        assert text == ""


class TestNeedsConversion:
    """Browser recordings always need ffmpeg; the packaged app may lack it."""

    def test_native_formats_pass_through(self):
        assert mod.needs_conversion("/tmp/a.wav") is False
        assert mod.needs_conversion("/tmp/a.mp3") is False
        assert mod.needs_conversion("/tmp/a.ogg") is False

    def test_browser_recordings_need_ffmpeg(self):
        assert mod.needs_conversion("/tmp/a.webm") is True
        assert mod.needs_conversion("/tmp/a.mp4") is True
        assert mod.needs_conversion("/tmp/a.m4a") is True

    def test_accepts_a_bare_suffix(self):
        # 上传路由手里只有后缀,而 Path(".webm").suffix 是空串。
        assert mod.needs_conversion(".webm") is True
        assert mod.needs_conversion(".wav") is False


class TestPrepareAudio:
    def test_missing_ffmpeg_reports_failure(self, monkeypatch, tmp_path):
        audio = tmp_path / "a.webm"
        audio.write_bytes(b"\x00" * 16)
        monkeypatch.setattr(mod.shutil, "which", lambda _name: None)
        assert mod.ffmpeg_available() is False
        assert mod.prepare_audio_for_flash(str(audio)) is None

    def test_native_audio_is_not_copied(self, tmp_path):
        audio = tmp_path / "a.wav"
        audio.write_bytes(b"\x00" * 16)
        assert mod.prepare_audio_for_flash(str(audio)) == (str(audio), False)


class TestSizeGuard:
    @pytest.mark.asyncio
    async def test_oversized_audio_is_refused(self, monkeypatch, tmp_path):
        # 整个文件会被读进内存再 base64(×1.33),必须先卡住。
        monkeypatch.setenv("apikey", "sk-test")
        monkeypatch.setattr(mod, "MAX_AUDIO_BYTES", 8)
        audio = tmp_path / "a.wav"
        audio.write_bytes(b"\x00" * 64)

        with patch.object(mod.httpx, "AsyncClient") as client:
            assert await mod.transcribe_doubao_flash(str(audio)) is None
            client.assert_not_called()
