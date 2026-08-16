# -*- coding: utf-8 -*-
"""Doubao / Volcengine streaming ASR (WebSocket bigmodel_async).

Bidirectional optimized endpoint:

  wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async

Resource ids (2.0 hourly is the one we opened):

  volc.seedasr.sauc.duration      豆包流式 2.0 · 小时版
  volc.seedasr.sauc.concurrent    豆包流式 2.0 · 并发版
  volc.bigasr.sauc.duration       豆包流式 1.0 · 小时版

Auth matches flash: new-console ``X-Api-Key``, or legacy
``X-Api-App-Key`` + ``X-Api-Access-Key``.
"""
from __future__ import annotations

import gzip
import json
import logging
import os
import struct
import uuid
from typing import Any, Optional, Tuple

logger = logging.getLogger(__name__)

STREAM_URL = (
    "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
)
DEFAULT_STREAM_RESOURCE_ID = "volc.seedasr.sauc.duration"

# header: version=1, size=4 bytes
_HEADER_BASE = 0x11
MSG_FULL_CLIENT = 0x01
MSG_AUDIO_ONLY = 0x02
MSG_FULL_SERVER = 0x09
MSG_ERROR = 0x0F
FLAG_LAST = 0x02
SERIAL_NONE = 0x00
SERIAL_JSON = 0x01
COMPRESS_GZIP = 0x01


def resolve_stream_resource_id() -> str:
    """Resource id for streaming recognition."""
    configured = ""
    try:
        from ...config import load_config

        configured = (
            load_config().agents.transcription_doubao_stream_resource_id
            or ""
        ).strip()
    except Exception:  # noqa: BLE001
        configured = ""
    if configured:
        return configured
    return (
        os.environ.get("POTATO_SPEECH_STREAM_RESOURCE_ID")
        or os.environ.get("VOLCENGINE_SPEECH_STREAM_RESOURCE_ID")
        or DEFAULT_STREAM_RESOURCE_ID
    ).strip() or DEFAULT_STREAM_RESOURCE_ID


def build_stream_headers(
    api_key: str,
    app_id: str = "",
    *,
    resource_id: Optional[str] = None,
    connect_id: Optional[str] = None,
) -> dict:
    """Headers for the OpenSpeech streaming handshake."""
    headers = {
        "X-Api-Resource-Id": resource_id or resolve_stream_resource_id(),
        "X-Api-Connect-Id": connect_id or str(uuid.uuid4()),
    }
    if app_id:
        headers["X-Api-App-Key"] = app_id
        headers["X-Api-Access-Key"] = api_key
    else:
        headers["X-Api-Key"] = api_key
    return headers


def encode_frame(
    msg_type: int,
    flags: int,
    serialization: int,
    compression: int,
    payload: bytes,
) -> bytes:
    header = bytes(
        [
            _HEADER_BASE,
            ((msg_type & 0x0F) << 4) | (flags & 0x0F),
            ((serialization & 0x0F) << 4) | (compression & 0x0F),
            0x00,
        ]
    )
    return header + struct.pack(">I", len(payload)) + payload


def encode_full_client_request() -> bytes:
    body = {
        "user": {"uid": "potato-composer"},
        "audio": {
            "format": "pcm",
            "codec": "raw",
            "rate": 16000,
            "bits": 16,
            "channel": 1,
        },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": True,
            "enable_punc": True,
            "show_utterances": True,
            "result_type": "full",
        },
    }
    payload = gzip.compress(
        json.dumps(body, ensure_ascii=False).encode("utf-8"),
    )
    return encode_frame(
        MSG_FULL_CLIENT,
        0x00,
        SERIAL_JSON,
        COMPRESS_GZIP,
        payload,
    )


def encode_audio(pcm: bytes, *, last: bool = False) -> bytes:
    flags = FLAG_LAST if last else 0x00
    return encode_frame(
        MSG_AUDIO_ONLY,
        flags,
        SERIAL_NONE,
        COMPRESS_GZIP,
        gzip.compress(pcm),
    )


def decode_frame(data: bytes) -> dict[str, Any]:
    """Parse one OpenSpeech binary frame."""
    if len(data) < 8:
        return {"kind": "error", "code": 0, "message": "short frame"}
    msg_type = (data[1] >> 4) & 0x0F
    flags = data[1] & 0x0F
    compression = data[2] & 0x0F
    offset = 4
    sequence: Optional[int] = None
    if flags & 0x01:
        if len(data) < offset + 4:
            return {"kind": "error", "code": 0, "message": "short seq"}
        sequence = struct.unpack(">i", data[offset:offset + 4])[0]
        offset += 4
    if msg_type == MSG_ERROR:
        if len(data) < offset + 8:
            return {"kind": "error", "code": 0, "message": "short err"}
        code = struct.unpack(">I", data[offset:offset + 4])[0]
        size = struct.unpack(">I", data[offset + 4:offset + 8])[0]
        raw = data[offset + 8:offset + 8 + size]
        if compression == COMPRESS_GZIP:
            try:
                raw = gzip.decompress(raw)
            except OSError:
                pass
        return {
            "kind": "error",
            "code": code,
            "message": raw.decode("utf-8", "replace"),
            "flags": flags,
            "sequence": sequence,
        }
    if len(data) < offset + 4:
        return {"kind": "error", "code": 0, "message": "short size"}
    size = struct.unpack(">I", data[offset:offset + 4])[0]
    raw = data[offset + 4:offset + 4 + size]
    if compression == COMPRESS_GZIP:
        try:
            raw = gzip.decompress(raw)
        except OSError:
            pass
    try:
        body: Any = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        body = {}
    return {
        "kind": "response",
        "flags": flags,
        "sequence": sequence,
        "final": bool(flags & FLAG_LAST),
        "body": body,
    }


def extract_stream_text(body: Any) -> Tuple[str, bool]:
    """Return ``(text, definite)`` from a server JSON body."""
    if not isinstance(body, dict):
        return "", False
    result = body.get("result")
    if isinstance(result, list):
        result = result[0] if result else {}
    if not isinstance(result, dict):
        return "", False
    text = str(result.get("text") or "").strip()
    definite = False
    utterances = result.get("utterances") or []
    if isinstance(utterances, list):
        definite = any(
            isinstance(item, dict) and item.get("definite")
            for item in utterances
        )
    return text, definite
