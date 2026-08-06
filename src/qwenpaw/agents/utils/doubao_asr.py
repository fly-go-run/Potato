# -*- coding: utf-8 -*-
"""Doubao / Volcengine OpenSpeech ASR (flash HTTP).

Uses the big-model flash recognition endpoint:

  POST https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash

Auth (either works):
  - New console: ``X-Api-Key`` from env ``apikey`` / ``VOLCENGINE_SPEECH_API_KEY``
  - Legacy: ``X-Api-App-Key`` + ``X-Api-Access-Key`` from ``keyid`` + ``apikey``
"""
from __future__ import annotations

import base64
import logging
import os
import shutil
import subprocess
import tempfile
import uuid
from pathlib import Path
from typing import Optional, Tuple

import httpx

logger = logging.getLogger(__name__)

FLASH_URL = (
    "https://openspeech.bytedance.com"
    "/api/v3/auc/bigmodel/recognize/flash"
)
DEFAULT_RESOURCE_ID = "volc.bigasr.auc_turbo"
SUCCESS_STATUS = "20000000"

# Formats the flash API documents as supported without conversion.
_NATIVE_EXTS = {".wav", ".mp3", ".ogg"}
_CONVERT_EXTS = {".webm", ".mp4", ".m4a", ".flac", ".opus", ".aac"}


def resolve_speech_credentials() -> Optional[Tuple[str, str]]:
    """Return ``(api_key, app_id)`` for OpenSpeech auth.

    ``api_key`` is always required (new-console single key, or legacy access
    token). ``app_id`` is optional; when set, legacy dual-header auth is used.
    """
    # Ensure project-root .env is loaded (keyid / apikey).
    try:
        import qwenpaw.constant  # noqa: F401
    except Exception:  # noqa: BLE001
        pass

    api_key = (
        os.environ.get("VOLCENGINE_SPEECH_API_KEY")
        or os.environ.get("QWENPAW_SPEECH_API_KEY")
        or os.environ.get("apikey")
        or os.environ.get("APIKEY")
        or ""
    ).strip()
    app_id = (
        os.environ.get("VOLCENGINE_SPEECH_APP_ID")
        or os.environ.get("QWENPAW_SPEECH_APP_ID")
        or os.environ.get("keyid")
        or os.environ.get("KEYID")
        or ""
    ).strip()
    if not api_key:
        return None
    return api_key, app_id


def resolve_resource_id() -> str:
    """Resource id for flash recognition (overridable via env/config)."""
    configured = ""
    try:
        from ...config import load_config

        configured = (
            load_config().agents.transcription_doubao_resource_id or ""
        ).strip()
    except Exception:  # noqa: BLE001 — config may be unavailable in tests
        configured = ""
    if configured:
        return configured
    return (
        os.environ.get("QWENPAW_SPEECH_RESOURCE_ID")
        or os.environ.get("VOLCENGINE_SPEECH_RESOURCE_ID")
        or DEFAULT_RESOURCE_ID
    ).strip() or DEFAULT_RESOURCE_ID


def has_doubao_credentials() -> bool:
    return resolve_speech_credentials() is not None


def build_auth_headers(
    api_key: str,
    app_id: str = "",
    *,
    resource_id: str = DEFAULT_RESOURCE_ID,
    request_id: Optional[str] = None,
) -> dict:
    """Build OpenSpeech request headers for flash recognition."""
    rid = request_id or str(uuid.uuid4())
    headers = {
        "Content-Type": "application/json",
        "X-Api-Resource-Id": resource_id,
        "X-Api-Request-Id": rid,
        "X-Api-Sequence": "-1",
    }
    if app_id:
        headers["X-Api-App-Key"] = app_id
        headers["X-Api-Access-Key"] = api_key
    else:
        headers["X-Api-Key"] = api_key
    return headers


def _convert_to_wav(src_path: str) -> Optional[str]:
    """Convert non-native formats to 16k mono wav via ffmpeg."""
    if not shutil.which("ffmpeg"):
        logger.warning(
            "ffmpeg not found; cannot convert %s for Doubao ASR",
            src_path,
        )
        return None

    fd, dst_path = tempfile.mkstemp(suffix=".wav")
    os.close(fd)
    try:
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-loglevel",
                "error",
                "-i",
                src_path,
                "-acodec",
                "pcm_s16le",
                "-ar",
                "16000",
                "-ac",
                "1",
                dst_path,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=60,
            check=True,
        )
        return dst_path
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        stderr = getattr(exc, "stderr", b"") or b""
        logger.warning(
            "ffmpeg conversion failed for Doubao ASR: %s\n%s",
            exc,
            stderr.decode(errors="replace"),
        )
        try:
            os.unlink(dst_path)
        except OSError:
            pass
        return None


def prepare_audio_for_flash(file_path: str) -> Optional[Tuple[str, bool]]:
    """Return ``(path, is_temp)`` ready for flash upload.

    Native wav/mp3/ogg are used as-is. Other formats are converted to wav.
    """
    ext = Path(file_path).suffix.lower() or ".wav"
    if ext in _NATIVE_EXTS:
        return file_path, False
    if ext in _CONVERT_EXTS or ext:
        converted = _convert_to_wav(file_path)
        if converted:
            return converted, True
        return None
    return file_path, False


def audio_format_for_path(file_path: str) -> str:
    """Map a local path to OpenSpeech ``audio.format`` (e.g. wav/mp3/ogg)."""
    ext = (Path(file_path).suffix.lower() or ".wav").lstrip(".")
    # Flash docs use short container names; opus-in-ogg stays "ogg".
    if ext in {"wav", "mp3", "ogg", "mp4", "m4a", "flac", "aac", "webm"}:
        if ext == "m4a":
            return "mp4"
        return ext
    return "wav"


def _extract_text(payload: dict) -> str:
    """Pull transcript text from flash JSON body."""
    if not isinstance(payload, dict):
        return ""
    result = payload.get("result")
    if isinstance(result, dict):
        text = result.get("text")
        if isinstance(text, str) and text.strip():
            return text.strip()
    text = payload.get("text")
    if isinstance(text, str) and text.strip():
        return text.strip()
    return ""


async def transcribe_doubao_flash(file_path: str) -> Optional[str]:
    """Transcribe *file_path* with Doubao flash ASR.

    Returns transcribed text, or ``None`` on failure / missing credentials.
    """
    creds = resolve_speech_credentials()
    if creds is None:
        logger.warning(
            "Doubao ASR credentials missing "
            "(set apikey/keyid in .env or VOLCENGINE_SPEECH_API_KEY)",
        )
        return None

    api_key, app_id = creds
    prepared = prepare_audio_for_flash(file_path)
    if prepared is None:
        logger.warning(
            "Doubao ASR: unsupported or unconvertible audio: %s",
            file_path,
        )
        return None

    audio_path, is_temp = prepared
    resource_id = resolve_resource_id()
    headers = build_auth_headers(
        api_key,
        app_id,
        resource_id=resource_id,
    )

    try:
        raw = Path(audio_path).read_bytes()
        if not raw:
            logger.warning("Doubao ASR: empty audio file %s", audio_path)
            return None
        audio_b64 = base64.b64encode(raw).decode("ascii")
        # OpenSpeech flash requires audio.format; must match the bytes we send
        # (post-conversion .wav when browser webm/mp4 was transcoded).
        audio_format = audio_format_for_path(audio_path)
        payload = {
            "user": {"uid": "qwenpaw-user"},
            "audio": {
                "data": audio_b64,
                "format": audio_format,
            },
            "request": {
                "model_name": "bigmodel",
                "enable_itn": True,
                "enable_punc": True,
                "show_utterances": True,
            },
        }

        async with httpx.AsyncClient(timeout=httpx.Timeout(180.0)) as client:
            response = await client.post(
                FLASH_URL,
                headers=headers,
                json=payload,
            )

        status_code = response.headers.get("X-Api-Status-Code", "")
        status_message = response.headers.get("X-Api-Message", "")
        log_id = response.headers.get("X-Tt-Logid", "")

        if response.status_code >= 400:
            logger.warning(
                "Doubao ASR HTTP %s: %s (log_id=%s)",
                response.status_code,
                (response.text or "")[:300],
                log_id,
            )
            return None

        if status_code and status_code != SUCCESS_STATUS:
            logger.warning(
                "Doubao ASR business failure: code=%s message=%s log_id=%s",
                status_code,
                status_message,
                log_id,
            )
            return None

        try:
            body = response.json()
        except Exception:  # noqa: BLE001
            logger.warning(
                "Doubao ASR: non-JSON response (log_id=%s)",
                log_id,
            )
            return None

        text = _extract_text(body)
        if not text:
            logger.warning(
                "Doubao ASR returned empty text (status=%s log_id=%s)",
                status_code or "ok",
                log_id,
            )
            return None

        logger.debug("Doubao ASR transcribed %s: %s", file_path, text[:80])
        return text
    except httpx.TimeoutException:
        logger.warning("Doubao ASR request timed out for %s", file_path)
        return None
    except Exception:  # noqa: BLE001
        logger.warning(
            "Doubao ASR failed for %s",
            file_path,
            exc_info=True,
        )
        return None
    finally:
        if is_temp:
            try:
                os.unlink(audio_path)
            except OSError:
                pass
